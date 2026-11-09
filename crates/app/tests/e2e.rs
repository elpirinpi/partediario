//! Prueba end-to-end: parseo del chat -> agrupación en novedades ->
//! generación del .docx -> verificación del OOXML resultante.
//!
//! Usa los fixtures en `../../testdata/` (se ejecutan con cwd = raíz del crate).

use parte_diario_docx as docx_editor;
use parte_diario_parser::grouping;
use parte_diario_parser::parser;
use std::collections::{HashMap, HashSet};
use std::io::Read;

#[test]
fn e2e_chat_a_docx() {
    // 1) Parseo.
    let txt =
        std::fs::read_to_string("../../testdata/chat.txt").expect("falta ../../testdata/chat.txt");
    let chat = parser::parse_chat_txt(&txt);
    assert_eq!(chat.format, parser::ChatFormat::Bracket);
    // 5 líneas con timestamp - 1 mensaje de sistema = 4 mensajes útiles.
    assert_eq!(chat.messages.len(), 4);

    // 2) Agrupación: foto antes + foto después + video => 1 novedad.
    let mut media_names = HashSet::new();
    media_names.insert("foto1.png".to_string());
    media_names.insert("foto2.png".to_string());
    media_names.insert("video1.mp4".to_string());
    let novs = grouping::group_novedades(&chat, &media_names);
    assert_eq!(novs.len(), 1, "debe agruparse en una sola novedad");
    let n = &novs[0];
    assert_eq!(n.photos.len(), 2, "las dos fotos van a la misma novedad");
    assert_eq!(n.videos_skipped.len(), 1, "el video se marca como omitido");
    assert_eq!(n.headers.len(), 1);
    assert!(n.headers[0].contains("ANTI-RUIDO"));
    assert!(n.closing.is_some(), "debe detectar el cierre");
    assert_eq!(
        n.fecha_mencionada,
        chrono::NaiveDate::from_ymd_opt(2026, 10, 10),
        "la fecha se infiere del cuerpo, no del timestamp de envío"
    );
    // Datos factuales intactos en el texto crudo.
    assert!(n.body_raw.contains("A123456"));
    assert!(n.body_raw.contains("Charles de Gaulle"));

    // 3) Generación del .docx.
    let mut media: HashMap<String, Vec<u8>> = HashMap::new();
    for f in ["foto1.png", "foto2.png"] {
        media.insert(
            f.to_string(),
            std::fs::read(format!("../../testdata/{f}")).unwrap(),
        );
    }
    let desde = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
    let hasta = chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
    let out = std::env::temp_dir().join("parte-diario-e2e.docx");
    let res = docx_editor::build_report(
        Some(std::path::Path::new("../../testdata/plantilla_minima.docx")),
        &out,
        &novs,
        desde,
        hasta,
        &media,
    )
    .expect("build_report falló");
    assert_eq!(res.novedades_incluidas, 1);
    assert_eq!(res.images_embedded, 2);

    // 4) Verificación del .docx resultante.
    let file = std::fs::File::open(&out).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut names = Vec::new();
    for i in 0..zip.len() {
        names.push(zip.by_index(i).unwrap().name().unwrap().into_owned());
    }
    assert!(names.contains(&"word/media/img1.png".to_string()));
    assert!(names.contains(&"word/media/img2.png".to_string()));

    let mut doc = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut doc)
        .unwrap();
    // Fecha del encabezado reemplazada.
    assert!(doc.contains("11 de octubre, 2026."), "fecha del encabezado");
    assert!(
        !doc.contains("10 de octubre, 2026."),
        "fecha vieja eliminada"
    );
    // Rango del Asunto reemplazado.
    assert!(
        doc.contains(
            "desde las 08:00 horas del día 11-10-2026 hasta las 08:00 horas del día 12-10-2026."
        ),
        "rango del asunto"
    );
    // Encabezado de la novedad y narrativa presentes.
    assert!(doc.contains("ANTI-RUIDO"));
    assert!(doc.contains("A123456"));
    // Relaciones de imagen referenciadas.
    assert!(doc.contains("r:embed=\"rIdFoto1\""));
    assert!(doc.contains("r:embed=\"rIdFoto2\""));
    // paraIds válidos: 8 hex, únicos, < 0x80000000.
    let re = regex::Regex::new(r#"w14:paraId="([0-9A-Fa-f]{8})""#).unwrap();
    let mut seen = HashSet::new();
    let mut count = 0;
    for c in re.captures_iter(&doc) {
        let v = u32::from_str_radix(&c[1], 16).unwrap();
        assert!(v < 0x8000_0000, "paraId fuera de 31 bits");
        assert!(seen.insert(v), "paraId duplicado");
        count += 1;
    }
    assert!(count >= 4, "debe haber varios párrafos nuevos con paraId");

    let mut rels = String::new();
    zip.by_name("word/_rels/document.xml.rels")
        .unwrap()
        .read_to_string(&mut rels)
        .unwrap();
    assert!(rels.contains("Target=\"media/img1.png\""));
    assert!(rels.contains("Target=\"media/img2.png\""));

    let mut ct = String::new();
    zip.by_name("[Content_Types].xml")
        .unwrap()
        .read_to_string(&mut ct)
        .unwrap();
    assert!(ct.contains("PartName=\"/word/media/img1.png\""));
}

/// Con la plantilla REAL del usuario: las tablas de novedades antiguas de la
/// plantilla deben eliminarse; solo quedan las novedades del .zip, con el
/// formato de la plantilla (Tahoma 13, columnas 4082/5273).
#[test]
fn e2e_plantilla_real_solo_novedades_del_zip() {
    let txt =
        std::fs::read_to_string("../../testdata/chat.txt").expect("falta ../../testdata/chat.txt");
    let chat = parser::parse_chat_txt(&txt);
    let mut media_names = HashSet::new();
    media_names.insert("foto1.png".to_string());
    media_names.insert("foto2.png".to_string());
    let novs = grouping::group_novedades(&chat, &media_names);
    assert_eq!(novs.len(), 1);

    let mut media: HashMap<String, Vec<u8>> = HashMap::new();
    for f in ["foto1.png", "foto2.png"] {
        media.insert(
            f.to_string(),
            std::fs::read(format!("../../testdata/{f}")).unwrap(),
        );
    }
    let desde = chrono::NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();
    let hasta = chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap();
    let out = std::env::temp_dir().join("parte-diario-e2e-real.docx");
    let res = docx_editor::build_report(
        Some(std::path::Path::new("../../testdata/plantilla_real.docx")),
        &out,
        &novs,
        desde,
        hasta,
        &media,
    )
    .expect("build_report falló con la plantilla real");
    assert_eq!(res.novedades_incluidas, 1);
    assert!(
        res.warnings
            .iter()
            .any(|w| w.contains("novedades antiguas")),
        "debe avisar que eliminó las tablas viejas"
    );

    let file = std::fs::File::open(&out).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let names: Vec<String> = (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().unwrap().into_owned())
        .collect();
    assert!(
        names.contains(&"word/media/image1.png".to_string()),
        "el logo del membrete debe conservarse"
    );
    assert!(
        !names.contains(&"word/media/image2.jpg".to_string()),
        "las fotos de novedades antiguas no deben quedar en el .docx"
    );
    let mut doc = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut doc)
        .unwrap();

    // Membrete preservado.
    assert!(doc.contains("REPUBLICA DOMINICANA"));
    assert!(doc.contains("Directora de"));
    // Novedad nueva presente, con formato de la plantilla.
    assert!(doc.contains("A123456"));
    assert!(doc.contains("Charles de Gaulle"));
    assert!(doc.contains("Tahoma"), "fuente de la plantilla");
    assert!(
        doc.contains("<w:gridCol w:w=\"4082\""),
        "ancho de columna de la plantilla"
    );
    // Contenido viejo de la plantilla: ELIMINADO.
    assert!(
        !doc.contains("Sandy Veloz"),
        "las novedades antiguas de la plantilla no deben quedar"
    );
    assert!(
        !doc.contains("dando inicio al operativo en la Direcci"),
        "texto viejo de novedad eliminado"
    );
    // Fecha del encabezado actualizada.
    assert!(doc.contains("11 de octubre, 2026."));
}
