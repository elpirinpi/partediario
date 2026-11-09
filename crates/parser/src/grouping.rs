//! Agrupación de mensajes del chat en "novedades" (una fila del reporte = un
//! incidente/evento policial completo).
//!
//! Heurísticas:
//! - Un bloque de texto con estructura de "nota policial" (encabezados
//!   institucionales en mayúsculas o con **negritas**, inicio típico
//!   "Siendo las..." / "Respetuosamente informo que...") abre una novedad.
//! - Las fotos consecutivas sin texto intermedio se agrupan bajo la nota más
//!   cercana (anterior o posterior): las fotos pueden llegar ANTES o DESPUÉS
//!   del texto según cómo las envió el agente.
//! - Las fotos que no pueden asociarse quedan como "huérfanas" para
//!   reasignación manual asistida en la GUI.
//! - La fecha del reporte se infiere del cuerpo de cada novedad (NO del
//!   timestamp de WhatsApp); las novedades con fecha atípica se señalan.

use chrono::NaiveDate;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use crate::parser::{AttachmentKind, ParsedChat};

#[derive(Debug, Clone)]
pub struct Novedad {
    pub id: usize,
    pub headers: Vec<String>,
    pub body_raw: String,
    pub closing: Option<String>,
    pub photos: Vec<String>,
    pub videos_skipped: Vec<String>,
    pub fecha_mencionada: Option<NaiveDate>,
    pub include: bool,
    pub is_orphan: bool,
    pub body_formal: String,
    pub sender: String,
}

impl Novedad {
    fn new(id: usize, sender: String) -> Self {
        Self {
            id,
            headers: Vec::new(),
            body_raw: String::new(),
            closing: None,
            photos: Vec::new(),
            videos_skipped: Vec::new(),
            fecha_mencionada: None,
            include: true,
            is_orphan: false,
            body_formal: String::new(),
            sender,
        }
    }
}

fn re_note_start() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)^\s*(siendo las|respetuosamente (le )?informo|por medio de la presente|cumplo con informar(le)?|me permito informar(le)?|por este medio (le )?informo)",
        )
        .unwrap()
    })
}

fn re_closing() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)(es lo que informo( para( su conocimiento( y fines? (de lugar|correspondientes)?)?)?)?|para su conocimiento( y fines? (de lugar|correspondientes)?)?|para los fines (de lugar|correspondientes)|lo que comunico para (su )?conocimiento)",
        )
        .unwrap()
    })
}

fn re_fecha_num() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(\d{1,2})[/\-.](\d{1,2})[/\-.](\d{2,4})").unwrap())
}

fn re_fecha_txt() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)(\d{1,2})\s+de\s+(enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre)\s*(?:del?\s+|,?\s*)(\d{4})",
        )
        .unwrap()
    })
}

fn mes_num(nombre: &str) -> Option<u32> {
    match nombre.to_lowercase().as_str() {
        "enero" => Some(1),
        "febrero" => Some(2),
        "marzo" => Some(3),
        "abril" => Some(4),
        "mayo" => Some(5),
        "junio" => Some(6),
        "julio" => Some(7),
        "agosto" => Some(8),
        "septiembre" | "setiembre" => Some(9),
        "octubre" => Some(10),
        "noviembre" => Some(11),
        "diciembre" => Some(12),
        _ => None,
    }
}

/// ¿La línea parece un encabezado institucional?
fn is_header_line(line: &str) -> bool {
    let t = line.trim().trim_matches('*').trim();
    if t.chars().count() < 4 {
        return false;
    }
    // **negritas** de WhatsApp
    if line.trim().starts_with("**") && t.len() > 4 {
        return true;
    }
    let letters: Vec<char> = t.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 10 {
        let up = letters.iter().filter(|c| c.is_uppercase()).count();
        if up as f32 / letters.len() as f32 > 0.8 {
            let tl = t.to_lowercase();
            const KW: &[&str] = &[
                "proc",
                "operativo",
                "incaut",
                "seguimiento",
                "visita",
                "denuncia",
                "contaminac",
                "sónic",
                "sonica",
                "ruido",
                "anti",
                "p.n",
                "comandancia",
                "destacamento",
                "departamento",
                "garantía de paz",
                "fuerza de tarea",
                "supervisión",
                "patrulla",
            ];
            if KW.iter().any(|k| tl.contains(k)) {
                return true;
            }
            // Línea corta toda en mayúsculas también suele ser encabezado.
            if letters.len() <= 45 {
                return true;
            }
        }
    }
    false
}

/// ¿El bloque de texto inicia una novedad nueva?
fn is_note_start_block(text: &str) -> bool {
    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .peekable();
    let mut saw_header = false;
    while let Some(&l) = lines.peek() {
        if is_header_line(l) {
            saw_header = true;
            lines.next();
        } else {
            break;
        }
    }
    if saw_header {
        return true;
    }
    match lines.peek() {
        Some(l) => re_note_start().is_match(l),
        None => false,
    }
}

/// Separa encabezados institucionales (inicio del bloque) del cuerpo narrativo.
fn split_headers_body(text: &str) -> (Vec<String>, String) {
    let mut headers = Vec::new();
    let mut body_lines: Vec<&str> = Vec::new();
    let mut in_headers = true;
    for line in text.lines() {
        let t = line.trim();
        if in_headers && !t.is_empty() && is_header_line(t) {
            headers.push(t.trim_matches('*').trim().to_string());
        } else {
            in_headers = false;
            body_lines.push(line);
        }
    }
    (headers, body_lines.join("\n").trim().to_string())
}

/// Extrae la última oración de cierre ("Es lo que informo para...").
fn extract_closing(text: &str) -> Option<String> {
    let re = re_closing();
    let m = re.find_iter(text).last()?;
    // Expandir a la oración completa.
    let bytes = text.as_bytes();
    let mut start = m.start();
    while start > 0 && bytes[start - 1] != b'.' && bytes[start - 1] != b'\n' {
        start -= 1;
    }
    let mut end = m.end();
    while end < bytes.len() && bytes[end] != b'.' && bytes[end] != b'\n' {
        end += 1;
    }
    if end < bytes.len() && bytes[end] == b'.' {
        end += 1;
    }
    let s = text[start..end].trim();
    if s.len() >= 10 {
        Some(s.to_string())
    } else {
        None
    }
}

/// Fecha más mencionada dentro del texto (numérica o "10 de octubre de 2026").
///
/// Es **determinística**: ante empate de frecuencias se elige la fecha **más
/// reciente**. El orden de iteración de `HashMap` es no determinista en Rust
/// (se randomiza el hash por proceso), por lo que un simple `max_by_key` sobre
/// un `HashMap` podría devolver fechas distintas en ejecuciones distintas con
/// el mismo texto. Elegir siempre la mayor entre los empatados garantiza
/// reproducibilidad del reporte.
pub fn most_frequent_date(text: &str) -> Option<NaiveDate> {
    let mut counts: HashMap<NaiveDate, usize> = HashMap::new();
    for c in re_fecha_num().captures_iter(text) {
        let (d, m) = (c[1].parse::<u32>().ok()?, c[2].parse::<u32>().ok()?);
        let y_raw = &c[3];
        let y: i32 = if y_raw.len() == 2 {
            let v: i32 = y_raw.parse().ok()?;
            if v < 70 {
                2000 + v
            } else {
                1900 + v
            }
        } else {
            y_raw.parse().ok()?
        };
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            *counts.entry(date).or_insert(0) += 1;
        }
    }
    for c in re_fecha_txt().captures_iter(text) {
        let d: u32 = c[1].parse().ok()?;
        let m = mes_num(&c[2])?;
        let y: i32 = c[3].parse().ok()?;
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            *counts.entry(date).or_insert(0) += 1;
        }
    }
    // max_by_key con comparador determinístico: prioridad a frecuencia, luego
    // a la fecha más reciente (NaiveDate ordena cronológicamente).
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(d, _)| d)
}

/// Recalcula los datos derivados del texto (fecha mencionada y cierre).
/// Se usa al cargar el chat y también cuando el usuario edita el texto crudo.
pub fn refresh_metadata(n: &mut Novedad) {
    let joined = format!("{}\n{}", n.headers.join("\n"), n.body_raw);
    n.fecha_mencionada = most_frequent_date(&joined);
    n.closing = extract_closing(&n.body_raw);
}

fn finalize_novedad(n: &mut Novedad) {
    refresh_metadata(n);
    n.body_formal = n.body_raw.clone(); // se reemplaza al formalizar
}

/// Agrupa los mensajes del chat en novedades.
/// `media_names`: nombres de archivo de imagen/video realmente presentes en el .zip.
///
/// La comparación de nombres es insensible a mayúsculas/minúsculas: los .zip
/// creados en Windows a veces difieren en el caso del nombre respecto a lo
/// que dice el .txt, y antes eso hacía que la foto se descartara en silencio.
pub fn group_novedades(chat: &ParsedChat, media_names: &HashSet<String>) -> Vec<Novedad> {
    let media_lower: HashSet<String> = media_names.iter().map(|s| s.to_lowercase()).collect();
    let mut novedades: Vec<Novedad> = Vec::new();
    let mut current: Option<Novedad> = None;
    let mut pending_photos: Vec<String> = Vec::new();
    let mut pending_videos: Vec<String> = Vec::new();
    let mut since_text: usize = 0; // mensajes de texto desde el último texto de la novedad actual
    let mut idc: usize = 0;

    // Vacía la novedad en curso hacia la lista.
    macro_rules! flush {
        () => {
            if let Some(mut n) = current.take() {
                finalize_novedad(&mut n);
                novedades.push(n);
            }
        };
    }

    for msg in &chat.messages {
        let text = msg.text.trim();
        let has_text = !text.is_empty();

        // 1) Adjunto (puede venir con o sin texto acompañante).
        if let Some(att) = &msg.attachment {
            let present = media_lower.contains(&att.filename.to_lowercase());
            match att.kind {
                AttachmentKind::Image => {
                    if let Some(c) = current.as_mut() {
                        if since_text <= 5 && present {
                            c.photos.push(att.filename.clone());
                        }
                    } else if present {
                        pending_photos.push(att.filename.clone());
                    }
                }
                AttachmentKind::Video => {
                    // El video se descarta del documento, pero la novedad se conserva.
                    if let Some(c) = current.as_mut() {
                        c.videos_skipped.push(att.filename.clone());
                    } else {
                        pending_videos.push(att.filename.clone());
                    }
                }
                AttachmentKind::Other => {}
            }
            if !has_text {
                continue;
            }
        }
        if !has_text {
            continue;
        }

        // 2) Texto: ¿abre una novedad nueva?
        if is_note_start_block(text) {
            flush!();
            idc += 1;
            let mut n = Novedad::new(idc, msg.sender.clone());
            let (headers, body) = split_headers_body(text);
            n.headers = headers;
            n.body_raw = body;
            n.photos.append(&mut pending_photos);
            n.videos_skipped.append(&mut pending_videos);
            current = Some(n);
            since_text = 0;
        } else if current.is_none() {
            // Texto sin encabezado formal: igual se crea la novedad.
            idc += 1;
            let mut n = Novedad::new(idc, msg.sender.clone());
            n.body_raw = text.to_string();
            n.photos.append(&mut pending_photos);
            n.videos_skipped.append(&mut pending_videos);
            current = Some(n);
            since_text = 0;
        } else {
            // Continuación de la novedad actual.
            if let Some(c) = current.as_mut() {
                if !c.body_raw.is_empty() {
                    c.body_raw.push_str("\n\n");
                }
                c.body_raw.push_str(text);
            }
            since_text += 1;
        }
    }
    flush!();

    // 3) Fotos pendientes sin nota asociada -> novedades "huérfanas" (una por
    // racha) para reasignación manual asistida.
    if !pending_photos.is_empty() {
        idc += 1;
        let mut n = Novedad::new(idc, String::from("(sin remitente)"));
        n.is_orphan = true;
        n.photos = pending_photos;
        n.videos_skipped = pending_videos;
        n.include = true;
        novedades.push(n);
    }

    novedades
}

/// Infiera la fecha del reporte (la más mencionada) y devuelve los ids de las
/// novedades con fecha atípica (posible error de dedo del agente).
///
/// Determinístico: ante empate de frecuencias se elige la fecha más reciente,
/// igual que `most_frequent_date`.
pub fn infer_report_range(novedades: &[Novedad]) -> (Option<NaiveDate>, Vec<usize>) {
    let mut counts: HashMap<NaiveDate, usize> = HashMap::new();
    for n in novedades {
        if let Some(f) = n.fecha_mencionada {
            *counts.entry(f).or_insert(0) += 1;
        }
    }
    let majority = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
        .map(|(d, _)| d);
    let outliers = match majority {
        Some(m) => novedades
            .iter()
            .filter(|n| n.fecha_mencionada.map(|f| f != m).unwrap_or(false))
            .map(|n| n.id)
            .collect(),
        None => Vec::new(),
    };
    (majority, outliers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_chat_txt;

    fn media(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn groups_note_with_photos_before_and_after() {
        let txt = "[10/10/26, 11:00:00 p. m.] Agente: ‎<adjunto: IMG-1.jpg>\n\
                   [10/10/26, 11:01:00 p. m.] Agente: OPERATIVO ANTI-RUIDO GARANTIA DE PAZ\nSiendo las 20:00 horas del día 10/10/2026, se realizó operativo...\nEs lo que informo para su conocimiento.\n\
                   [10/10/26, 11:02:00 p. m.] Agente: ‎<adjunto: IMG-2.jpg>\n\
                   [10/10/26, 11:03:00 p. m.] Agente: ‎<adjunto: IMG-3.jpg>";
        let chat = parse_chat_txt(txt);
        let novs = group_novedades(&chat, &media(&["IMG-1.jpg", "IMG-2.jpg", "IMG-3.jpg"]));
        assert_eq!(novs.len(), 1);
        assert_eq!(novs[0].photos.len(), 3);
        assert_eq!(novs[0].headers.len(), 1);
        assert!(novs[0].closing.is_some());
        assert_eq!(
            novs[0].fecha_mencionada,
            NaiveDate::from_ymd_opt(2026, 10, 10)
        );
    }

    #[test]
    fn orphan_photos_become_review_item() {
        let txt = "[10/10/26, 11:00:00 p. m.] Agente: ‎<adjunto: IMG-9.jpg>";
        let chat = parse_chat_txt(txt);
        let novs = group_novedades(&chat, &media(&["IMG-9.jpg"]));
        assert_eq!(novs.len(), 1);
        assert!(novs[0].is_orphan);
    }

    #[test]
    fn photo_matching_is_case_insensitive() {
        // El .txt dice IMG-1.JPG pero el .zip trae img-1.jpg: debe asociarse igual.
        let txt = "[10/10/26, 11:00:00 p. m.] A: Siendo las 20:00 del día 10/10/2026, nota.\n\
                   [10/10/26, 11:01:00 p. m.] A: ‎<adjunto: IMG-1.JPG>";
        let chat = parse_chat_txt(txt);
        let novs = group_novedades(&chat, &media(&["img-1.jpg"]));
        assert_eq!(novs.len(), 1);
        assert_eq!(novs[0].photos.len(), 1);
    }

    #[test]
    fn outlier_date_detected() {
        let txt = "[10/10/26, 11:00:00 p. m.] A: Siendo las 20:00 del día 10/10/2026, nota uno.\n\
                   [10/10/26, 11:01:00 p. m.] A: Siendo las 21:00 del día 10/10/2026, nota dos.\n\
                   [10/10/26, 11:02:00 p. m.] A: Siendo las 22:00 del día 11/10/2026, nota tres.";
        let chat = parse_chat_txt(txt);
        let novs = group_novedades(&chat, &media(&[]));
        let (majority, outliers) = infer_report_range(&novs);
        assert_eq!(majority, NaiveDate::from_ymd_opt(2026, 10, 10));
        assert_eq!(outliers, vec![novs[2].id]);
    }
}
