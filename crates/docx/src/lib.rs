//! Edición quirúrgica de la plantilla .docx (que es un .zip con XML OOXML).
//!
//! Estrategia: NO se reconstruye el documento desde cero (eso perdería el
//! membrete). Se lee el contenedor con el crate `zip`, se edita
//! `word/document.xml` por reemplazo de texto dirigido (con fusión de runs
//! para el caso en que el texto esté dividido en varios `<w:t>`), se
//! ELIMINAN las tablas de novedades antiguas de la plantilla (la plantilla
//! solo aporta el formato) y se inserta una tabla por novedad —con el mismo
//! formato de la plantilla— usando ÚNICAMENTE las novedades del .zip de
//! WhatsApp. Las fotos se incrustan como `<w:drawing>` en la celda izquierda,
//! registrando la relación en `word/_rels/document.xml.rels` y la
//! declaración en `[Content_Types].xml`.
//!
//! Detalles críticos:
//! - Cada `<w:p>` nuevo lleva un `w14:paraId` único de 31 bits (< 0x80000000),
//!   en hexadecimal de 8 dígitos (UUID truncado).
//! - Dimensiones de imagen en EMU (914400 EMU = 1 pulgada), preservando el
//!   aspect ratio real (crate `image`), ancho fijo ~2.36" y alto máximo ~3.15".

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDate};
use regex::Regex;
use uuid::Uuid;

use parte_diario_ai::strip_asterisks;
use parte_diario_parser::grouping::Novedad;

/// Plantilla DOCX embebida por defecto (se usa si el usuario no carga una externa).
/// Contiene un membrete genérico y una tabla vacía de novedades para anclar el contenido.
const DEFAULT_TEMPLATE_DOCX: &[u8] = include_bytes!("../../../testdata/plantilla_minima.docx");

const EMU_PER_INCH: f64 = 914400.0;
pub const DEFAULT_PHOTO_WIDTH_IN: f64 = 2.36;
pub const DEFAULT_PHOTO_MAX_HEIGHT_IN: f64 = 3.15;

/// Opciones de formato del documento. Los valores por defecto reproducen
/// exactamente la plantilla oficial: Tahoma 13 pt y fotos de 2.36" de ancho.
#[derive(Debug, Clone)]
pub struct BuildOptions {
    pub font_family: String,
    pub font_size_pt: u8,
    pub photo_width_in: f64,
    pub photo_max_height_in: f64,
    pub app_version: String,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            font_family: "Tahoma".to_string(),
            font_size_pt: 13,
            photo_width_in: DEFAULT_PHOTO_WIDTH_IN,
            photo_max_height_in: DEFAULT_PHOTO_MAX_HEIGHT_IN,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

pub struct BuildResult {
    pub warnings: Vec<String>,
    pub images_embedded: usize,
    pub novedades_incluidas: usize,
}

fn esc(s: &str) -> String {
    // Caracteres prohibidos en XML 1.0 (p. ej. controles de WhatsApp):
    // Word los rechaza con "archivo dañado", así que se eliminan.
    let s: String = s
        .chars()
        .filter(|c| {
            matches!(c,
                '\u{9}' | '\u{A}' | '\u{D}'
                | '\u{20}'..='\u{D7FF}'
                | '\u{E000}'..='\u{FFFD}'
                | '\u{10000}'..='\u{10FFFF}'
            )
        })
        .collect();
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// Generador de w14:paraId únicos de 31 bits.
struct ParaIdGen {
    used: HashSet<u32>,
}

impl ParaIdGen {
    fn new(existing: &HashSet<u32>) -> Self {
        Self {
            used: existing.clone(),
        }
    }
    fn next(&mut self) -> String {
        for _ in 0..10_000 {
            let v = (Uuid::new_v4().as_u128() & 0x7FFF_FFFF) as u32;
            // 0 se evita; fuera de 31 bits es inválido para Word.
            if v != 0 && self.used.insert(v) {
                return format!("{v:08X}");
            }
        }
        // Fallback determinista (prácticamente inalcanzable).
        let mut v: u32 = 0x1000_0001;
        while !self.used.insert(v) {
            v = (v.wrapping_add(1)) & 0x7FFF_FFFF;
            if v == 0 {
                v = 1;
            }
        }
        format!("{v:08X}")
    }
}

struct EmbeddedImage {
    rid: String,
    part: String, // "word/media/img1.jpg"
    bytes: Vec<u8>,
    content_type: String,
}

/// ¿Declara `tag` el atributo EXACTO `attr` (p. ej. `xmlns:a`)?
///
/// No basta con `tag.contains("xmlns:a")`: esa subcadena también está
/// presente en `xmlns:aink` y `xmlns:am3d` (namespaces de Word 2016+),
/// lo que hacía saltarse la declaración y Word rechazaba el .docx.
fn tag_has_attr(tag: &str, attr: &str) -> bool {
    let mut rest = tag;
    while let Some(pos) = rest.find(attr) {
        let before_ok = pos == 0
            || rest[..pos]
                .chars()
                .last()
                .map(|c| c.is_whitespace() || c == '<')
                .unwrap_or(false);
        if before_ok && rest[pos + attr.len()..].starts_with('=') {
            return true;
        }
        rest = &rest[pos + attr.len()..];
    }
    false
}

/// Asegura que el elemento raíz declare los namespaces que usará el XML nuevo.
fn ensure_namespaces(doc: &mut String) {
    let needed = [
        (
            "xmlns:w14",
            "http://schemas.microsoft.com/office/word/2010/wordml",
        ),
        (
            "xmlns:wp",
            "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing",
        ),
        (
            "xmlns:a",
            "http://schemas.openxmlformats.org/drawingml/2006/main",
        ),
        (
            "xmlns:pic",
            "http://schemas.openxmlformats.org/drawingml/2006/picture",
        ),
        (
            "xmlns:r",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships",
        ),
    ];
    if let Some(start) = doc.find("<w:document") {
        if let Some(rel_end) = doc[start..].find('>') {
            let end = start + rel_end;
            let tag = doc[start..end].to_string();
            let mut add = String::new();
            for (k, v) in needed {
                if !tag_has_attr(&tag, k) {
                    let _ = write!(add, " {k}=\"{v}\"");
                }
            }
            if !add.is_empty() {
                doc.insert_str(end, &add);
            }
        }
    }
}

/// Todas las etiquetas y atributos con prefijo deben tener su `xmlns`
/// declarado. `quick-xml` no valida namespaces: sin esta comprobación, un
/// `xmlns:a` faltante pasaba la validación interna y Word abría el archivo
/// como dañado ("Word detectó un error al intentar abrir el archivo").
fn validate_namespaces(doc: &str) -> Result<(), String> {
    let decl_re = Regex::new(r"xmlns:([A-Za-z_][\w.-]*)=").unwrap();
    let mut declared: HashSet<String> = HashSet::new();
    for c in decl_re.captures_iter(doc) {
        declared.insert(c[1].to_string());
    }
    let check = |p: &str| -> Result<(), String> {
        if p == "xml" || p == "xmlns" || declared.contains(p) {
            Ok(())
        } else {
            Err(format!(
                "el prefijo '{p}' se usa pero no está declarado con xmlns (Word rechazará el archivo)"
            ))
        }
    };
    // Prefijos de elementos (<w:p>, </a:graphic>…): seguro en todo el texto,
    // porque '<' literal no puede aparecer dentro del contenido (va escapado).
    let elem_re = Regex::new(r"</?([A-Za-z_][\w.-]*):").unwrap();
    for c in elem_re.captures_iter(doc) {
        check(&c[1])?;
    }
    // Prefijos de atributos (w14:paraId, r:embed…): solo dentro de etiquetas,
    // para no confundirlos con dos puntos del texto ("Placa:A123").
    let tag_re = Regex::new(r"<[^>]*>").unwrap();
    let attr_re = Regex::new(r"[\s<]([A-Za-z_][\w.-]*):[A-Za-z_]").unwrap();
    for t in tag_re.find_iter(doc) {
        for c in attr_re.captures_iter(t.as_str()) {
            check(&c[1])?;
        }
    }
    Ok(())
}

/// Reemplaza `re` por `replacement` dentro de los primeros `max_paras`
/// párrafos. Si el texto del párrafo está dividido en varios `<w:t>`, se
/// fusiona y se reconstruye como un único run conservando el `<w:rPr>` del
/// primer run (así el formato del membrete no se pierde).
fn replace_in_paragraphs(
    doc: &str,
    re: &Regex,
    replacement: &str,
    max_paras: usize,
) -> (String, usize) {
    // NOTA: la primera alternativa captura las etiquetas autocerradas
    // (`<w:p w:rsidR="..." />`, que Word usa para párrafos vacíos) y las deja
    // intactas. Sin esto, el regex las trataba como aperturas, "tragaba" el
    // párrafo siguiente y al reconstruir dejaba un `</w:p>` huérfano que
    // invalidaba todo el documento ("expected `</w:body>`, but `</w:p>`").
    // (El crate `regex` no soporta look-around, de ahí la alternancia.)
    let para_re = Regex::new(r"(?s)<w:p(\s[^>]*)?/>|<w:p(\s[^>]*)?>(.*?)</w:p>").unwrap();
    let t_re = Regex::new(r"(?s)<w:t(\s[^>]*)?/>|<w:t(\s[^>]*)?>(.*?)</w:t>").unwrap();
    let rpr_re = Regex::new(r"(?s)<w:rPr>.*?</w:rPr>").unwrap();

    let mut out = String::with_capacity(doc.len());
    let mut last = 0;
    let mut n_paras = 0;
    let mut count = 0;

    for caps in para_re.captures_iter(doc) {
        let m = caps.get(0).unwrap();
        out.push_str(&doc[last..m.start()]);
        if caps.get(1).is_some() {
            // Etiqueta autocerrada: no es un párrafo procesable, se deja intacta.
            out.push_str(m.as_str());
            last = m.end();
            continue;
        }
        n_paras += 1;
        if n_paras > max_paras {
            out.push_str(m.as_str());
        } else {
            let attrs = caps.get(2).map(|g| g.as_str()).unwrap_or("");
            let inner = caps.get(3).map(|g| g.as_str()).unwrap_or("");
            let mut full = String::new();
            for tc in t_re.captures_iter(inner) {
                // Saltar <w:t ... /> autocerrados (grupo 1).
                if tc.get(1).is_none() {
                    if let Some(t) = tc.get(3) {
                        full.push_str(&xml_unescape(t.as_str()));
                    }
                }
            }
            if re.is_match(&full) {
                count += 1;
                let new_text = re
                    .replace_all(&full, regex::NoExpand(replacement))
                    .to_string();
                let rpr = rpr_re.find(inner).map(|x| x.as_str()).unwrap_or("");
                let _ = write!(
                    out,
                    "<w:p{attrs}><w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
                    esc(&new_text)
                );
            } else {
                out.push_str(m.as_str());
            }
        }
        last = m.end();
    }
    out.push_str(&doc[last..]);
    (out, count)
}

fn insert_before(haystack: &str, marker: &str, insertion: &str) -> String {
    match haystack.find(marker) {
        Some(pos) => {
            let mut s = String::with_capacity(haystack.len() + insertion.len());
            s.push_str(&haystack[..pos]);
            s.push_str(insertion);
            s.push_str(&haystack[pos..]);
            s
        }
        None => haystack.to_string(),
    }
}

/// Rangos (inicio, fin) de los <w:tbl> de nivel superior en document.xml.
/// El conteo de anidamiento protege contra tablas anidadas y se aceptan
/// atributos en la etiqueta (`<w:tbl w:rsidR="...">`), que Word suele añadir.
fn top_level_tables(doc: &str) -> Vec<(usize, usize)> {
    let open_pat = "<w:tbl";
    let close_pat = "</w:tbl>";
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < doc.len() {
        let open = doc[i..].find(open_pat).map(|p| i + p);
        let close = doc[i..].find(close_pat).map(|p| i + p);
        match (open, close) {
            (Some(o), Some(c)) if o < c => {
                // Debe ser exactamente <w:tbl o <w:tbl ...>, no <w:tblPr>.
                let after = doc[o + open_pat.len()..].chars().next();
                let is_tbl_tag = after
                    .map(|c| c == '>' || c.is_whitespace())
                    .unwrap_or(false);
                if !is_tbl_tag {
                    i = o + open_pat.len();
                    continue;
                }
                let tag_end = match doc[o..].find('>') {
                    Some(p) => o + p + 1,
                    None => break,
                };
                if doc[o..tag_end].trim_end().ends_with("/>") {
                    // Tabla autocerrada: no es válida en OOXML, pero no rompemos.
                    out.push((o, tag_end));
                    i = tag_end;
                    continue;
                }
                let mut depth = 1usize;
                let mut j = tag_end;
                let mut end = None;
                while j < doc.len() {
                    let next_open = doc[j..].find(open_pat).map(|p| j + p);
                    let next_close = doc[j..].find(close_pat).map(|p| j + p);
                    match (next_open, next_close) {
                        (Some(no), Some(nc)) if no < nc => {
                            let after = doc[no + open_pat.len()..].chars().next();
                            let is_tbl_tag = after
                                .map(|c| c == '>' || c.is_whitespace())
                                .unwrap_or(false);
                            if is_tbl_tag {
                                depth += 1;
                            }
                            j = no + open_pat.len();
                        }
                        (_, Some(nc)) => {
                            depth -= 1;
                            j = nc + close_pat.len();
                            if depth == 0 {
                                end = Some(j);
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                match end {
                    Some(e) => {
                        out.push((o, e));
                        i = e;
                    }
                    None => break,
                }
            }
            (_, Some(c)) => i = c + close_pat.len(),
            (_, None) => break,
        }
    }
    out
}

/// Estilo de la tabla de novedad: se copia de la plantilla cuando esta trae
/// tablas de novedades antiguas; si no, se usan los valores oficiales.
struct NovedadTableStyle {
    tbl_pr_inner: String,
    grid: [String; 2],
}

impl NovedadTableStyle {
    fn from_template(novedad_table_xml: Option<&str>) -> Self {
        if let Some(t) = novedad_table_xml {
            let pr = Regex::new(r"(?s)<w:tblPr>(.*?)</w:tblPr>")
                .ok()
                .and_then(|re| re.captures(t))
                .map(|c| c[1].to_string());
            let grid: Vec<String> = Regex::new(r#"<w:gridCol w:w="(\d+)""#)
                .ok()
                .map(|re| re.captures_iter(t).map(|c| c[1].to_string()).collect())
                .unwrap_or_default();
            if let (Some(pr), [a, b]) = (pr, grid.as_slice()) {
                return Self {
                    tbl_pr_inner: pr,
                    grid: [a.clone(), b.clone()],
                };
            }
        }
        Self::official()
    }

    /// Valores de la plantilla oficial (Anti-Ruidos): sin bordes visibles,
    /// centrada, columnas 4082/5273 dxa.
    fn official() -> Self {
        Self {
            tbl_pr_inner:
                "<w:tblStyle w:val=\"Tablaconcuadrcula\"/><w:tblW w:w=\"0\" w:type=\"auto\"/>\
                <w:jc w:val=\"center\"/><w:tblLook w:val=\"04A0\" w:firstRow=\"1\" w:lastRow=\"0\" \
                w:firstColumn=\"1\" w:lastColumn=\"0\" w:noHBand=\"0\" w:noVBand=\"1\"/>"
                    .to_string(),
            grid: ["4082".to_string(), "5273".to_string()],
        }
    }
}

/// Párrafo con el estilo de la plantilla oficial: Tahoma 13pt.
/// `center` = centrado (encabezados y fotos); si no, justificado (cuerpo).
fn tahoma_para(gen: &mut ParaIdGen, text: &str, center: bool, opts: &BuildOptions) -> String {
    let pid = gen.next();
    let jc = if center { "center" } else { "both" };
    let font = esc(&opts.font_family);
    let half_points = u32::from(opts.font_size_pt).saturating_mul(2);
    format!(
        "<w:p w14:paraId=\"{pid}\"><w:pPr><w:jc w:val=\"{jc}\"/></w:pPr>\
         <w:r><w:rPr><w:rFonts w:ascii=\"{font}\" w:hAnsi=\"{font}\"/><w:sz w:val=\"{half_points}\"/><w:szCs w:val=\"{half_points}\"/></w:rPr>\
         <w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
        esc(text)
    )
}

/// Párrafo vacío (separador entre cuerpo y cierre), como en la plantilla.
fn empty_para(gen: &mut ParaIdGen) -> String {
    let pid = gen.next();
    format!("<w:p w14:paraId=\"{pid}\"><w:pPr><w:jc w:val=\"center\"/></w:pPr></w:p>")
}

struct PreparedImage {
    bytes: Vec<u8>,
    ext: &'static str,
    content_type: &'static str,
    cx: i64,
    cy: i64,
}

/// Dimensiones en EMU preservando aspect ratio (ancho fijo, alto máximo).
fn emu_size(img: &image::DynamicImage, opts: &BuildOptions) -> Option<(i64, i64)> {
    let (w, h) = (f64::from(img.width()), f64::from(img.height()));
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let mut cx = opts.photo_width_in.max(0.1) * EMU_PER_INCH;
    let mut cy = cx * h / w;
    let max_cy = opts.photo_max_height_in.max(0.1) * EMU_PER_INCH;
    if cy > max_cy {
        cy = max_cy;
        cx = cy * w / h;
    }
    Some((cx as i64, cy as i64))
}

/// Prepara la imagen para Word. JPEG/PNG se conservan tal cual; GIF, BMP y
/// WebP se convierten a PNG (con transparencia) para no etiquetar bytes de un
/// formato con la extensión/MIME de otro, lo que Word mostraba como imagen rota.
fn prepare_image(bytes: &[u8], opts: &BuildOptions) -> Option<PreparedImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let (cx, cy) = emu_size(&img, opts)?;
    let format = image::guess_format(bytes).ok();
    let (bytes, ext, content_type) = match format {
        Some(image::ImageFormat::Jpeg) => (bytes.to_vec(), "jpg", "image/jpeg"),
        Some(image::ImageFormat::Png) => (bytes.to_vec(), "png", "image/png"),
        _ => {
            let mut out = Cursor::new(Vec::new());
            img.write_to(&mut out, image::ImageFormat::Png).ok()?;
            (out.into_inner(), "png", "image/png")
        }
    };
    Some(PreparedImage {
        bytes,
        ext,
        content_type,
        cx,
        cy,
    })
}

fn drawing_xml(rid: &str, name: &str, cx: i64, cy: i64, doc_id: usize) -> String {
    format!(
        "<w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
         <wp:extent cx=\"{cx}\" cy=\"{cy}\"/>\
         <wp:docPr id=\"{doc_id}\" name=\"{name}\"/>\
         <wp:cNvGraphicFramePr><a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr>\
         <a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
         <pic:pic><pic:nvPicPr><pic:cNvPr id=\"{doc_id}\" name=\"{name}\"/><pic:cNvPicPr/></pic:nvPicPr>\
         <pic:blipFill><a:blip r:embed=\"{rid}\"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill>\
         <pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm>\
         <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr>\
         </pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing>"
    )
}

/// Una tabla por novedad, con el formato de la plantilla oficial:
/// celda izquierda = encabezados centrados + fotos; celda derecha = cuerpo
/// justificado + cierre. Las fotos van en la celda izquierda, como en el modelo.
#[allow(clippy::too_many_arguments)]
fn novedad_table(
    n: &Novedad,
    gen: &mut ParaIdGen,
    images: &mut Vec<EmbeddedImage>,
    warnings: &mut Vec<String>,
    media: &HashMap<String, &[u8]>,
    doc_id: &mut usize,
    rels_so_far: &str,
    used_parts: &mut HashSet<String>,
    style: &NovedadTableStyle,
    opts: &BuildOptions,
) -> String {
    // Celda izquierda: encabezados + fotos.
    let mut left = String::new();
    if n.headers.is_empty() {
        left.push_str(&tahoma_para(gen, &format!("Novedad #{}", n.id), true, opts));
    } else {
        for h in &n.headers {
            // Los encabezados del chat traen "**negrita**": nunca al .docx.
            left.push_str(&tahoma_para(gen, &strip_asterisks(h), true, opts));
        }
    }
    for ph in &n.photos {
        let key = ph.to_lowercase();
        match media.get(&key) {
            Some(bytes) => match prepare_image(bytes, opts) {
                Some(prep) => {
                    // rId y parte únicos frente a la plantilla y entre sí.
                    let mut idx = images.len() + 1;
                    let (rid, part) = loop {
                        let rid = format!("rIdFoto{idx}");
                        let part = format!("word/media/img{}.{}", idx, prep.ext);
                        if !rels_so_far.contains(&format!("Id=\"{rid}\""))
                            && !used_parts.contains(&part)
                        {
                            break (rid, part);
                        }
                        idx += 1;
                    };
                    used_parts.insert(part.clone());
                    let cx = prep.cx;
                    let cy = prep.cy;
                    images.push(EmbeddedImage {
                        rid: rid.clone(),
                        part,
                        bytes: prep.bytes,
                        content_type: prep.content_type.to_string(),
                    });
                    let dxml = drawing_xml(&rid, &format!("Foto {idx}"), cx, cy, *doc_id);
                    *doc_id += 1;
                    let pid = gen.next();
                    let _ = write!(
                        left,
                        "<w:p w14:paraId=\"{pid}\"><w:pPr><w:jc w:val=\"center\"/></w:pPr>\
                         <w:r><w:rPr><w:noProof/></w:rPr>{dxml}</w:r></w:p>"
                    );
                }
                None => warnings.push(format!(
                    "No se pudo leer la foto '{}' (novedad #{}); se omitió la imagen pero se conserva el texto.",
                    ph, n.id
                )),
            },
            None => warnings.push(format!(
                "La foto '{}' (novedad #{}) no está dentro del .zip; se omitió.",
                ph, n.id
            )),
        }
    }

    // Celda derecha: cuerpo justificado + cierre (como la plantilla).
    let mut right = String::new();
    let formal = n.body_formal.trim();
    let raw = n.body_raw.trim();
    // Red de seguridad: si el texto formal solo contiene espacios, usar el texto crudo.
    let formal_trimmed = formal.trim();
    let mut body = strip_asterisks(if formal_trimmed.is_empty() {
        raw
    } else {
        formal
    });
    // Evitar duplicar la oración de cierre (va como párrafo final).
    if let Some(cl) = &n.closing {
        let clt = strip_asterisks(cl.trim());
        if !clt.is_empty() {
            if let Some(pos) = body.find(&clt) {
                body.replace_range(pos..pos + clt.len(), "");
            }
        }
    }
    let re_par = Regex::new(r"\n\s*\n").unwrap();
    let mut any = false;
    for par in re_par.split(&body) {
        let t: String = par.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() {
            right.push_str(&tahoma_para(gen, &t, false, opts));
            any = true;
        }
    }
    if let Some(cl) = &n.closing {
        let clt = strip_asterisks(cl.trim());
        if !clt.is_empty() {
            if any {
                right.push_str(&empty_para(gen));
            }
            right.push_str(&tahoma_para(gen, &clt, false, opts));
            any = true;
        }
    }
    if !any {
        right.push_str(&tahoma_para(gen, "(sin texto)", false, opts));
    }

    format!(
        "<w:tbl><w:tblPr>{}</w:tblPr>\
         <w:tblGrid><w:gridCol w:w=\"{}\"/><w:gridCol w:w=\"{}\"/></w:tblGrid>\
         <w:tr><w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc>\
         <w:tc><w:tcPr><w:tcW w:w=\"{}\" w:type=\"dxa\"/></w:tcPr>{}</w:tc></w:tr></w:tbl>",
        style.tbl_pr_inner, style.grid[0], style.grid[1], style.grid[0], left, style.grid[1], right
    )
}

const MESES_ES: &[&str] = &[
    "enero",
    "febrero",
    "marzo",
    "abril",
    "mayo",
    "junio",
    "julio",
    "agosto",
    "septiembre",
    "octubre",
    "noviembre",
    "diciembre",
];

fn fecha_larga_es(d: NaiveDate) -> String {
    format!(
        "{} de {}, {}.",
        d.day(),
        MESES_ES[d.month0() as usize],
        d.year()
    )
}

fn fecha_ddmmyyyy(d: NaiveDate) -> String {
    format!("{:02}-{:02}-{}", d.day(), d.month(), d.year())
}

// ---------- validación interna ----------

fn validate_wellformed(name: &str, xml: &str) -> Result<(), String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(format!("{name}: XML mal formado: {e}")),
        }
        buf.clear();
    }
    Ok(())
}

fn validate_relationships(doc: &str, rels: &str) -> Result<(), String> {
    let re_embed = Regex::new(r#"r:embed="(rId[^"]+)""#).unwrap();
    let re_id = Regex::new(r#"Id="(rId[^"]+)""#).unwrap();
    let defined: HashSet<String> = re_id
        .captures_iter(rels)
        .map(|c| c[1].to_string())
        .collect();
    for c in re_embed.captures_iter(doc) {
        if !defined.contains(&c[1]) {
            return Err(format!(
                "La imagen con relación '{}' no está registrada en document.xml.rels",
                &c[1]
            ));
        }
    }
    Ok(())
}

/// Escribe la versión de la aplicación en las propiedades del documento para
/// que el archivo generado sea trazable, sin tocar el contenido ni el formato.
fn apply_version_properties(
    entries: &mut HashMap<String, Vec<u8>>,
    ctype: &mut String,
    version: &str,
) {
    let description = format!("Generado por Parte Diario v{version}");
    let core_key = "docProps/core.xml";
    let existing = entries
        .get(core_key)
        .and_then(|b| std::str::from_utf8(b).ok())
        .map(str::to_string);

    let core = match existing {
        Some(xml) => {
            let re = Regex::new(r"(?s)<dc:description>.*?</dc:description>").unwrap();
            if re.is_match(&xml) {
                re.replace(&xml, format!("<dc:description>{}</dc:description>", esc(&description)))
                    .into_owned()
            } else {
                insert_before(
                    &xml,
                    "</cp:coreProperties>",
                    &format!("<dc:description>{}</dc:description>", esc(&description)),
                )
            }
        }
        None => format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
             <cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" \
             xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" \
             xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
             <dc:description>{}</dc:description></cp:coreProperties>",
            esc(&description)
        ),
    };
    entries.insert(core_key.to_string(), core.into_bytes());

    let core_part = "PartName=\"/docProps/core.xml\"";
    if !ctype.contains(core_part) {
        *ctype = insert_before(
            ctype,
            "</Types>",
            "<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>",
        );
    }

    if let Some(pkg_rels) = entries
        .get("_rels/.rels")
        .and_then(|b| std::str::from_utf8(b).ok())
        .map(str::to_string)
    {
        if !pkg_rels.contains("docProps/core.xml") {
            let updated = insert_before(
                &pkg_rels,
                "</Relationships>",
                "<Relationship Id=\"rIdCoreProps\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>",
            );
            entries.insert("_rels/.rels".to_string(), updated.into_bytes());
        }
    }
}

fn validate_paraids(doc: &str) -> Result<(), String> {
    let re = Regex::new(r#"w14:paraId="([0-9A-Fa-f]{1,8})""#).unwrap();
    let mut seen = HashSet::new();
    for c in re.captures_iter(doc) {
        let v = u32::from_str_radix(&c[1], 16)
            .map_err(|_| format!("w14:paraId inválido: {}", &c[1]))?;
        if v >= 0x8000_0000 {
            return Err(format!("w14:paraId fuera de rango de 31 bits: {}", &c[1]));
        }
        if !seen.insert(v) {
            return Err(format!("w14:paraId duplicado: {}", &c[1]));
        }
    }
    Ok(())
}

/// Construye el Parte Diario a partir de la plantilla.
///
/// `novedades`: lista completa (se usan las marcadas con `include = true`).
/// `media`: nombre de archivo -> bytes, para las fotos a incrustar.
/// `template`: si es `None`, se usa una plantilla DOCX embebida por defecto
///   (con membrete genérico y tabla de novedades vacía), así el programa
///   funciona sin necesidad de que el usuario cargue una plantilla externa.
pub fn build_report(
    template: Option<&Path>,
    out: &Path,
    novedades: &[Novedad],
    fecha_desde: NaiveDate,
    fecha_hasta: NaiveDate,
    media: &HashMap<String, Vec<u8>>,
) -> Result<BuildResult, String> {
    build_report_with_options(
        template,
        out,
        novedades,
        fecha_desde,
        fecha_hasta,
        media,
        &BuildOptions::default(),
    )
}

/// Igual que [`build_report`], pero permite personalizar fuente, tamaño de
/// letra y dimensiones de las fotos sin alterar el formato por defecto.
#[allow(clippy::too_many_arguments)]
pub fn build_report_with_options(
    template: Option<&Path>,
    out: &Path,
    novedades: &[Novedad],
    fecha_desde: NaiveDate,
    fecha_hasta: NaiveDate,
    media: &HashMap<String, Vec<u8>>,
    opts: &BuildOptions,
) -> Result<BuildResult, String> {
    let mut warnings: Vec<String> = Vec::new();

    // 1) Cargar la plantilla: externa o embebida por defecto.
    let template_bytes: Vec<u8> = match template {
        Some(p) => std::fs::read(p).map_err(|e| format!("No se pudo abrir la plantilla: {e}"))?,
        None => DEFAULT_TEMPLATE_DOCX.to_vec(),
    };
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&template_bytes))
        .map_err(|e| format!("La plantilla no es un .docx válido: {e}"))?;
    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();
    for i in 0..zip.len() {
        let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let Ok(name) = f.name().map(std::borrow::Cow::into_owned) else {
            continue;
        };
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
        entries.insert(name, buf);
    }
    let mut used_parts: HashSet<String> = entries.keys().cloned().collect();

    let mut doc = String::from_utf8(
        entries
            .remove("word/document.xml")
            .ok_or("La plantilla no contiene word/document.xml")?,
    )
    .map_err(|_| "word/document.xml no es UTF-8 válido".to_string())?;

    // 0) La plantilla misma debe estar bien formada; si no, ábrala en Word
    //    y guárdela de nuevo antes de usarla.
    validate_wellformed("word/document.xml (plantilla)", &doc).map_err(|e| {
        format!(
            "La plantilla .docx parece estar dañada ({e}). Ábrala en Word, guárdela de nuevo e inténtelo otra vez."
        )
    })?;

    ensure_namespaces(&mut doc);
    validate_namespaces(&doc).map_err(|e| {
        format!(
            "La plantilla .docx usa namespaces que no declara ({e}). Ábrala en Word, guárdela de nuevo e inténtelo otra vez."
        )
    })?;

    // paraIds ya existentes (para no colisionar).
    let mut existing = HashSet::new();
    let re_pid = Regex::new(r#"w14:paraId="([0-9A-Fa-f]{1,8})""#).unwrap();
    for c in re_pid.captures_iter(&doc) {
        if let Ok(v) = u32::from_str_radix(&c[1], 16) {
            existing.insert(v);
        }
    }
    let mut gen = ParaIdGen::new(&existing);

    // 2) Fecha del encabezado ("10 de octubre, 2026.").
    let re_fecha = Regex::new(
        r"(?i)\d{1,2}\s+de\s+(enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre)\s*,?\s*\d{4}\.?",
    )
    .unwrap();
    let (d2, c1) = replace_in_paragraphs(&doc, &re_fecha, &fecha_larga_es(fecha_desde), 30);
    doc = d2;
    if c1 == 0 {
        warnings.push(
            "No se encontró la fecha del encabezado en la plantilla; revísela manualmente en el .docx generado."
                .to_string(),
        );
    }

    // 3) Rango del "Asunto".
    let rango = format!(
        "desde las 08:00 horas del día {} hasta las 08:00 horas del día {}.",
        fecha_ddmmyyyy(fecha_desde),
        fecha_ddmmyyyy(fecha_hasta)
    );
    let re_rango =
        Regex::new(r"(?i)desde las 08:00[^<>\n]*?hasta las 08:00[^<>\n]*?\d{4}[^<>\n.]*\.?")
            .unwrap();
    let (d3, c2) = replace_in_paragraphs(&doc, &re_rango, &rango, usize::MAX);
    doc = d3;
    if c2 == 0 {
        warnings.push(
            "No se encontró el rango del \"Asunto\" (08:00...hasta las 08:00) en la plantilla; revíselo manualmente."
                .to_string(),
        );
    }

    // 4) Novedades: SOLO las del .zip. La plantilla aporta el membrete y el
    //    formato; sus tablas de novedades antiguas se eliminan para que no
    //    se mezclen con las nuevas.
    let rels_original = String::from_utf8(
        entries
            .remove("word/_rels/document.xml.rels")
            .ok_or("La plantilla no contiene word/_rels/document.xml.rels")?,
    )
    .map_err(|_| "document.xml.rels no es UTF-8 válido".to_string())?;

    let incluidas: Vec<&Novedad> = novedades
        .iter()
        .filter(|n| {
            n.include && !(n.is_orphan && n.body_raw.trim().is_empty() && n.headers.is_empty())
        })
        .collect();

    let tables = top_level_tables(&doc);
    if tables.is_empty() {
        return Err(
            "La plantilla no tiene tabla (membrete) donde anclar las novedades.".to_string(),
        );
    }
    let first_end = tables[0].1;
    let removed = tables.len() - 1;
    if removed > 0 {
        warnings.push(format!(
            "La plantilla traía {removed} tabla(s) con novedades antiguas: se eliminaron. El reporte solo incluye las novedades del .zip de WhatsApp."
        ));
    }
    // Estilo de tabla copiado de la primera tabla de novedad de la plantilla
    // (si la plantilla no trae ninguna, se usan los valores oficiales).
    let style = NovedadTableStyle::from_template(tables.get(1).map(|(s, e)| &doc[*s..*e]));

    let media_lower: HashMap<String, &[u8]> = media
        .iter()
        .map(|(k, v)| (k.to_lowercase(), v.as_slice()))
        .collect();

    let mut images: Vec<EmbeddedImage> = Vec::new();
    // Los docPr de las imágenes que se conservan (p. ej. el logo del membrete)
    // ya usan ids; empezar en 1 provocaba ids duplicados en el documento.
    let re_docpr = Regex::new(r#"<wp:docPr\b[^>]*\bid="(\d+)""#).unwrap();
    let mut doc_id = re_docpr
        .captures_iter(&doc)
        .filter_map(|c| c[1].parse::<usize>().ok())
        .max()
        .unwrap_or(0)
        + 1;
    let mut tables_xml = String::new();
    for n in &incluidas {
        tables_xml.push_str(&novedad_table(
            n,
            &mut gen,
            &mut images,
            &mut warnings,
            &media_lower,
            &mut doc_id,
            &rels_original,
            &mut used_parts,
            &style,
            opts,
        ));
    }
    // Se reemplaza SOLO la región de tablas antiguas: se conserva el membrete
    // (primera tabla) y todo el contenido posterior a la última tabla (firmas,
    // anexos o párrafos institucionales). Antes se borraba todo hasta el
    // <w:sectPr>, lo que podía eliminar una firma fuera de las tablas.
    let resume_at = if tables.len() > 1 {
        tables.last().map(|(_, e)| *e).unwrap_or(first_end)
    } else {
        first_end
    };
    doc = format!("{}{}{}", &doc[..first_end], tables_xml, &doc[resume_at..]);

    // 5) Registrar imágenes: entradas word/media/*, rels y Content_Types.
    let mut rels = rels_original;
    let mut ctype = String::from_utf8(
        entries
            .remove("[Content_Types].xml")
            .ok_or("La plantilla no contiene [Content_Types].xml")?,
    )
    .map_err(|_| "[Content_Types].xml no es UTF-8 válido".to_string())?;

    for img in &images {
        entries.insert(img.part.clone(), img.bytes.clone());
        let target = img.part.strip_prefix("word/").unwrap_or(&img.part);
        rels = insert_before(
            &rels,
            "</Relationships>",
            &format!(
                "<Relationship Id=\"{}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/image\" Target=\"{}\"/>",
                img.rid, target
            ),
        );
        let ov = format!("PartName=\"/{}\"", img.part);
        if !ctype.contains(&ov) {
            ctype = insert_before(
                &ctype,
                "</Types>",
                &format!(
                    "<Override PartName=\"/{}\" ContentType=\"{}\"/>",
                    img.part, img.content_type
                ),
            );
        }
    }

    // 5b) Purgar las fotos de las novedades antiguas. Al eliminar sus tablas,
    // sus relaciones e imágenes quedaban dentro del .docx (peso y posible
    // fuga de información del reporte anterior). Se conservan las imágenes
    // que sigan referenciadas por el membrete, encabezados o pies.
    rels = remove_unused_image_rels(&rels, &doc);
    let referenced_media = referenced_media_parts(&rels, &entries);
    let before_media = entries.len();
    entries.retain(|name, _| !name.starts_with("word/media/") || referenced_media.contains(name));
    let removed_media = before_media.saturating_sub(entries.len());
    if removed_media > 0 {
        warnings.push(format!(
            "Se eliminaron {removed_media} imagen(es) de la plantilla que ya no estaban referenciadas por el documento."
        ));
    }

    apply_version_properties(&mut entries, &mut ctype, &opts.app_version);

    entries.insert(
        "word/_rels/document.xml.rels".to_string(),
        rels.clone().into_bytes(),
    );
    entries.insert(
        "[Content_Types].xml".to_string(),
        ctype.clone().into_bytes(),
    );
    entries.insert("word/document.xml".to_string(), doc.clone().into_bytes());

    // 6) Validación interna estricta antes de escribir.
    validate_wellformed("word/document.xml", &doc)?;
    validate_namespaces(&doc)?;
    validate_wellformed("word/_rels/document.xml.rels", &rels)?;
    validate_wellformed("[Content_Types].xml", &ctype)?;
    validate_relationships(&doc, &rels)?;
    validate_paraids(&doc)?;

    // 7) Escribir el .docx de salida de forma atómica: primero un temporal en
    // la misma carpeta y luego renombrar. Si algo falla, el reporte anterior
    // no queda truncado.
    let tmp = temp_output_path(out);
    let write_result: Result<(), String> = (|| {
        let out_file = File::create(&tmp)
            .map_err(|e| format!("No se pudo crear el archivo de salida: {e}"))?;
        let mut writer = zip::ZipWriter::new(out_file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut names: Vec<&String> = entries.keys().collect();
        names.sort();
        for name in names {
            writer
                .start_file(name, options)
                .map_err(|e| e.to_string())?;
            writer
                .write_all(&entries[name])
                .map_err(|e| e.to_string())?;
        }
        let file = writer.finish().map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    match fs::rename(&tmp, out) {
        Ok(()) => {}
        Err(_) if out.exists() => {
            // Windows no reemplaza el destino con rename.
            fs::remove_file(out).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("No se pudo reemplazar el archivo de salida: {e}")
            })?;
            fs::rename(&tmp, out).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                format!("No se pudo finalizar el archivo de salida: {e}")
            })?;
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            return Err(format!("No se pudo finalizar el archivo de salida: {e}"));
        }
    }

    Ok(BuildResult {
        warnings,
        images_embedded: images.len(),
        novedades_incluidas: incluidas.len(),
    })
}

fn temp_output_path(out: &Path) -> PathBuf {
    let name = out
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("parte-diario.docx");
    out.with_file_name(format!(".{}.{}.tmp", name, std::process::id()))
}

struct RelInfo {
    start: usize,
    end: usize,
    id: String,
    rel_type: String,
    target: String,
    external: bool,
}

fn parse_relationships(rels: &str) -> Vec<RelInfo> {
    let tag_re = Regex::new(r"(?s)<Relationship\b[^>]*>").unwrap();
    let attr_re = Regex::new(r#"([A-Za-z_:][\w:.-]*)\s*=\s*"([^"]*)""#).unwrap();
    let mut out = Vec::new();
    for m in tag_re.find_iter(rels) {
        let mut id = String::new();
        let mut rel_type = String::new();
        let mut target = String::new();
        let mut external = false;
        for c in attr_re.captures_iter(m.as_str()) {
            match &c[1] {
                "Id" => id = c[2].to_string(),
                "Type" => rel_type = c[2].to_string(),
                "Target" => target = c[2].to_string(),
                "TargetMode" => external = c[2].eq_ignore_ascii_case("External"),
                _ => {}
            }
        }
        if !id.is_empty() {
            out.push(RelInfo {
                start: m.start(),
                end: m.end(),
                id,
                rel_type,
                target,
                external,
            });
        }
    }
    out
}

/// Quita de `document.xml.rels` las relaciones de imagen que ya no aparecen
/// en el documento final (las de las tablas de novedades antiguas).
fn remove_unused_image_rels(rels: &str, doc: &str) -> String {
    let used_re = Regex::new(r#"r:(?:embed|id|link)="([^"]+)""#).unwrap();
    let used: HashSet<String> = used_re
        .captures_iter(doc)
        .map(|c| c[1].to_string())
        .collect();
    let mut out = String::with_capacity(rels.len());
    let mut last = 0usize;
    for r in parse_relationships(rels) {
        if r.rel_type.ends_with("/image") && !r.external && !used.contains(&r.id) {
            out.push_str(&rels[last..r.start]);
            last = r.end;
        }
    }
    out.push_str(&rels[last..]);
    out
}

fn normalize_media_target(target: &str) -> Option<String> {
    let t = target.trim().trim_start_matches('/');
    let t = t.strip_prefix("word/").unwrap_or(t);
    let t = t.strip_prefix("../").unwrap_or(t);
    if t.starts_with("media/") {
        Some(format!("word/{t}"))
    } else {
        None
    }
}

fn collect_media_targets(rels: &str, out: &mut HashSet<String>) {
    for r in parse_relationships(rels) {
        if r.external {
            continue;
        }
        if let Some(p) = normalize_media_target(&r.target) {
            out.insert(p);
        }
    }
}

/// Partes `word/media/*` que siguen referenciadas por el documento o por
/// cualquier `.rels` del paquete (encabezados, pies, etc.).
fn referenced_media_parts(rels: &str, entries: &HashMap<String, Vec<u8>>) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_media_targets(rels, &mut out);
    for (name, bytes) in entries {
        if name.starts_with("word/_rels/")
            && Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("rels"))
        {
            if let Ok(s) = std::str::from_utf8(bytes) {
                collect_media_targets(s, &mut out);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regresión: Word escribe los párrafos vacíos como etiquetas autocerradas
    /// (`<w:p w:rsidR="..." />`). Si uno queda justo antes del párrafo de la
    /// fecha, el reemplazo no debe romper el XML (antes generaba un `</w:p>`
    /// huérfano: "expected `</w:body>`, but `</w:p>` was found").
    #[test]
    fn replace_ignores_self_closing_paragraphs() {
        let doc = concat!(
            r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">"#,
            r#"<w:body>"#,
            r#"<w:p w:rsidR="00A1B2C3" />"#,
            r#"<w:p><w:r><w:t>Santo Domingo Norte, 10 de octubre, 2026.</w:t></w:r></w:p>"#,
            r#"<w:p><w:r><w:t>Otro texto.</w:t></w:r></w:p>"#,
            r#"</w:body></w:document>"#,
        );
        let re_fecha = Regex::new(
            r"(?i)\d{1,2}\s+de\s+(enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre)\s*,?\s*\d{4}\.?",
        )
        .unwrap();
        let (out, count) = replace_in_paragraphs(doc, &re_fecha, "11 de octubre, 2026.", 30);
        assert_eq!(count, 1, "la fecha debe reemplazarse una sola vez");
        assert!(out.contains("11 de octubre, 2026."));
        assert!(!out.contains("10 de octubre, 2026."));
        // El párrafo autocerrado debe quedar intacto y el XML bien formado.
        assert!(out.contains(r#"<w:p w:rsidR="00A1B2C3" />"#));
        validate_wellformed("test", &out).expect("el XML resultante debe estar bien formado");
    }

    #[test]
    fn template_itself_must_be_wellformed() {
        // Una plantilla dañada debe reportarse como tal, no como error genérico.
        let bad = "<w:document><w:body><w:p></w:body></w:document>";
        assert!(validate_wellformed("plantilla", bad).is_err());
    }

    /// Regresión (plantilla real de Kelly): la raíz trae `xmlns:aink` y
    /// `xmlns:am3d`, cuyas subcadenas contienen "xmlns:a". La detección debe
    /// ser por atributo EXACTO: si no se declara `xmlns:a`/`xmlns:pic`,
    /// Word rechaza el .docx generado como dañado.
    #[test]
    fn ensure_namespaces_no_colisiona_con_subcadenas() {
        let mut doc = concat!(
            r#"<w:document xmlns:w="urn:w" xmlns:aink="urn:aink" xmlns:am3d="urn:am3d" "#,
            r#"xmlns:wp14="urn:wp14" xmlns:w14="urn:w14" xmlns:r="urn:r">"#,
            r#"<w:body><w:p/></w:body></w:document>"#
        )
        .to_string();
        ensure_namespaces(&mut doc);
        assert!(
            doc.contains("xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\""),
            "falta declarar xmlns:a exacto: {doc}"
        );
        assert!(
            doc.contains("xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\"")
        );
        assert!(doc.contains(
            "xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\""
        ));
        // Los ya declarados exactamente no se duplican.
        assert_eq!(doc.matches("xmlns:w14=").count(), 1);
        assert_eq!(doc.matches("xmlns:wp14=").count(), 1);
        assert_eq!(doc.matches("xmlns:r=").count(), 1);
        validate_namespaces(&doc).expect("el XML con namespaces completos debe validar");
    }

    #[test]
    fn validate_namespaces_detecta_prefijo_faltante() {
        let bad = r#"<w:document xmlns:w="urn:w"><w:body><a:graphic/></w:body></w:document>"#;
        assert!(validate_namespaces(bad).is_err());
        let good = r#"<w:document xmlns:w="urn:w" xmlns:a="urn:a"><w:body><a:graphic/></w:body></w:document>"#;
        assert!(validate_namespaces(good).is_ok());
        // Dos puntos en texto plano no son un prefijo.
        let texto = r#"<w:document xmlns:w="urn:w"><w:body><w:p><w:t>Placa:A123</w:t></w:p></w:body></w:document>"#;
        assert!(validate_namespaces(texto).is_ok());
    }

    #[test]
    fn top_level_tables_accepts_attributes_and_ignores_nested() {
        let doc = concat!(
            r#"<w:tbl w:rsidR="00A1"><w:tr><w:tc>"#,
            r#"<w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl>"#,
            r#"</w:tc></w:tr></w:tbl>"#,
            r#"<w:tbl><w:tr/></w:tbl>"#,
        );
        let tables = top_level_tables(doc);
        assert_eq!(tables.len(), 2, "la tabla anidada no debe contarse aparte");
    }

    #[test]
    fn prepare_image_converts_bmp_to_png() {
        let img = image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0]));
        let mut bmp = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut bmp, image::ImageFormat::Bmp)
            .unwrap();
        let prep = prepare_image(bmp.get_ref(), &BuildOptions::default()).expect("BMP válido");
        assert_eq!(prep.ext, "png");
        assert_eq!(prep.content_type, "image/png");
        assert!(prep.bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn remove_unused_image_rels_keeps_used_and_non_image() {
        let rels = concat!(
            r#"<Relationships>"#,
            r#"<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image1.png"/>"#,
            r#"<Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/image2.jpg"/>"#,
            r#"<Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>"#,
            r#"</Relationships>"#,
        );
        let doc = r#"<a:blip r:embed="rId1"/>"#;
        let out = remove_unused_image_rels(rels, doc);
        assert!(out.contains("rId1"));
        assert!(!out.contains("rId2"));
        assert!(out.contains("rId3"));
    }
}
