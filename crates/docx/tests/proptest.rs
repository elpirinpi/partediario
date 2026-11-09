//! Property-based tests para el generador de .docx OOXML (`build_report`).
//!
//! Verifican que:
//! - `build_report` nunca hace panic con entradas arbitrarias.
//! - Con entradas válidas produce un .docx que es un ZIP correcto, con
//!   `word/document.xml` bien formado, `w14:paraId` válidos y únicos, el
//!   número exacto de tablas de novedad, de imágenes incrustadas y de
//!   relaciones/overrides coherentes.

use std::collections::{HashMap, HashSet};
use std::io::Read;

use chrono::NaiveDate;
use proptest::prelude::*;

use parte_diario_docx::build_report;
use parte_diario_parser::grouping::Novedad;

// ---------------------------------------------------------------------------
// Estrategias
// ---------------------------------------------------------------------------

fn arb_date() -> impl Strategy<Value = NaiveDate> {
    (2000i32..=2035, 1u32..=12, 1u32..=28)
        .prop_map(|(y, m, d)| NaiveDate::from_ymd_opt(y, m, d).unwrap())
}

/// Texto construido con vocabulario realista (incluye caracteres que deben
/// escaparse en XML y marcas invisibles de WhatsApp).
fn arb_words() -> impl Strategy<Value = String> {
    let words: Vec<String> = [
        "operativo",
        "bocina",
        "vehículo",
        "A123456",
        "Charles de Gaulle",
        "<xml>&\"'",
        "ñandú",
        "música",
        "P.N.",
        "08:00",
        "10/10/2026",
        "**negrita**",
        "\u{200E}",
        "  ",
        "\t",
        "\n",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    prop::collection::vec(prop::sample::select(words), 0..=24).prop_map(|words| words.join(" "))
}

fn arb_raw_text() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"(?s).{0,200}").unwrap()
}

fn arb_photo_name() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"foto\d{1,3}\.png").unwrap()
}

/// Novedad con texto estructurado (para validar OOXML e imágenes).
fn arb_novedad() -> impl Strategy<Value = Novedad> {
    (
        any::<usize>(),
        prop::collection::vec(arb_words(), 0..=3),
        arb_words(),
        prop::option::of(arb_words()),
        prop::collection::vec(arb_photo_name(), 0..=4),
        prop::collection::vec(
            prop::string::string_regex(r"video\d{1,3}\.mp4").unwrap(),
            0..=3,
        ),
        prop::option::of(arb_date()),
        any::<bool>(),
        any::<bool>(),
        arb_words(),
        prop::sample::select(vec!["Agente Pérez".to_string(), "DIG-N3".to_string()]),
    )
        .prop_map(
            |(
                id,
                headers,
                body_raw,
                closing,
                photos,
                videos_skipped,
                fecha_mencionada,
                include,
                is_orphan,
                body_formal,
                sender,
            )| Novedad {
                id,
                headers,
                body_raw,
                closing,
                photos,
                videos_skipped,
                fecha_mencionada,
                include,
                is_orphan,
                body_formal,
                sender,
            },
        )
}

/// Novedad con texto arbitrario (para el test de "nunca hace panic").
fn arb_novedad_fuzz() -> impl Strategy<Value = Novedad> {
    (
        prop::collection::vec(arb_raw_text(), 0..=3),
        arb_raw_text(),
        prop::option::of(arb_raw_text()),
        prop::collection::vec(arb_photo_name(), 0..=4),
        prop::collection::vec(
            prop::string::string_regex(r"[A-Za-z0-9_.-]{1,20}").unwrap(),
            0..=3,
        ),
        prop::option::of(arb_date()),
        any::<bool>(),
        any::<bool>(),
        arb_raw_text(),
    )
        .prop_map(
            |(
                headers,
                body_raw,
                closing,
                photos,
                videos_skipped,
                fecha_mencionada,
                include,
                is_orphan,
                body_formal,
            )| Novedad {
                id: 1,
                headers,
                body_raw,
                closing,
                photos,
                videos_skipped,
                fecha_mencionada,
                include,
                is_orphan,
                body_formal,
                sender: "X".to_string(),
            },
        )
}

// ---------------------------------------------------------------------------
// Utilidades
// ---------------------------------------------------------------------------

/// PNG válido (dimensiones y color derivados del nombre) para incrustar.
fn make_png(name: &str) -> Vec<u8> {
    let seed = name.bytes().fold(0u32, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(u32::from(b))
    });
    let (w, h) = (1 + seed % 40, 1 + (seed / 7) % 40);
    let img = image::RgbImage::from_pixel(
        w,
        h,
        image::Rgb([
            (seed & 0xFF) as u8,
            ((seed >> 8) & 0xFF) as u8,
            ((seed >> 16) & 0xFF) as u8,
        ]),
    );
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn well_formed(xml: &str) -> Result<(), String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    reader.config_mut().check_end_names = true;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(e) => return Err(format!("XML mal formado: {e}")),
        }
        buf.clear();
    }
    Ok(())
}

/// Filtro idéntico al de `build_report` para saber qué novedades se incluyen.
fn is_included(n: &Novedad) -> bool {
    n.include && !(n.is_orphan && n.body_raw.trim().is_empty() && n.headers.is_empty())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// `build_report` nunca hace panic, ni con texto ni con bytes arbitrarios.
    #[test]
    fn build_report_never_panics(
        novs in prop::collection::vec(arb_novedad_fuzz(), 0..=4),
        media in prop::collection::hash_map(
            prop::string::string_regex(r"[A-Za-z0-9_.-]{1,20}").unwrap(),
            prop::collection::vec(any::<u8>(), 0..=128),
            0..=4,
        ),
        desde in arb_date(),
        hasta in arb_date(),
    ) {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out.docx");
        let _ = build_report(None, &out, &novs, desde, hasta, &media);
    }

    /// Con entradas válidas se produce un OOXML coherente y bien formado.
    #[test]
    fn build_report_produces_valid_ooxml(
        novs in prop::collection::vec(arb_novedad(), 0..=5),
        desde in arb_date(),
        hasta in arb_date(),
    ) {
        // Media: un PNG válido por cada nombre de foto referenciado.
        let mut media: HashMap<String, Vec<u8>> = HashMap::new();
        for n in &novs {
            for ph in &n.photos {
                media
                    .entry(ph.to_lowercase())
                    .or_insert_with(|| make_png(ph));
            }
        }

        let incluidas: Vec<&Novedad> = novs.iter().filter(|n| is_included(n)).collect();
        let expected_included = incluidas.len();
        let expected_photos: usize = incluidas.iter().map(|n| n.photos.len()).sum();

        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("parte-diario.docx");

        match build_report(None, &out, &novs, desde, hasta, &media) {
            Ok(res) => {
                prop_assert_eq!(res.novedades_incluidas, expected_included);
                prop_assert_eq!(
                    res.images_embedded,
                    expected_photos,
                    "imágenes incrustadas != fotos de novedades incluidas"
                );

                let file = std::fs::File::open(&out).expect("el .docx debe existir");
                let mut zip = zip::ZipArchive::new(file).expect("el .docx debe ser un ZIP");

                for part in [
                    "[Content_Types].xml",
                    "word/document.xml",
                    "word/_rels/document.xml.rels",
                ] {
                    prop_assert!(zip.by_name(part).is_ok(), "falta la parte {}", part);
                }

                let mut doc = String::new();
                zip.by_name("word/document.xml")
                    .unwrap()
                    .read_to_string(&mut doc)
                    .unwrap();
                if let Err(e) = well_formed(&doc) {
                    prop_assert!(false, "{}", e);
                }

                // Una tabla de membrete + una tabla por novedad incluida.
                let table_count = doc.matches("<w:tbl>").count() + doc.matches("<w:tbl ").count();
                prop_assert_eq!(
                    table_count,
                    1 + expected_included,
                    "número de tablas incorrecto"
                );

                // w14:paraId: válidos (31 bits) y únicos.
                let re_pid = regex::Regex::new(r#"w14:paraId="([0-9A-Fa-f]{1,8})""#).unwrap();
                let mut seen = HashSet::new();
                for c in re_pid.captures_iter(&doc) {
                    let v = u32::from_str_radix(&c[1], 16).unwrap();
                    prop_assert!(v < 0x8000_0000, "paraId fuera de 31 bits: {}", &c[1]);
                    prop_assert!(seen.insert(v), "paraId duplicado: {}", &c[1]);
                }

                // Relaciones e overrides de imagen coherentes con lo incrustado.
                let mut rels = String::new();
                zip.by_name("word/_rels/document.xml.rels")
                    .unwrap()
                    .read_to_string(&mut rels)
                    .unwrap();
                prop_assert_eq!(
                    rels.matches("relationships/image").count(),
                    expected_photos,
                    "relaciones de imagen != fotos incrustadas"
                );

                let mut ctype = String::new();
                zip.by_name("[Content_Types].xml")
                    .unwrap()
                    .read_to_string(&mut ctype)
                    .unwrap();
                prop_assert_eq!(
                    ctype.matches("PartName=\"/word/media/").count(),
                    expected_photos,
                    "overrides de media != fotos incrustadas"
                );
            }
            Err(e) => {
                prop_assert!(false, "build_report devolvió error: {}", e);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests deterministas complementarios
// ---------------------------------------------------------------------------

/// Las novedades con `include = false` no deben aparecer en el documento.
#[test]
fn excluded_novedades_do_not_appear() {
    let novs = vec![
        Novedad {
            id: 1,
            headers: vec!["OPERATIVO UNO".to_string()],
            body_raw: "Texto de la novedad uno.".to_string(),
            closing: None,
            photos: Vec::new(),
            videos_skipped: Vec::new(),
            fecha_mencionada: None,
            include: true,
            is_orphan: false,
            body_formal: String::new(),
            sender: "A".to_string(),
        },
        Novedad {
            id: 2,
            headers: vec!["OPERATIVO DOS".to_string()],
            body_raw: "Texto de la novedad dos.".to_string(),
            closing: None,
            photos: Vec::new(),
            videos_skipped: Vec::new(),
            fecha_mencionada: None,
            include: false,
            is_orphan: false,
            body_formal: String::new(),
            sender: "A".to_string(),
        },
    ];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("excluida.docx");
    let res = build_report(
        None,
        &out,
        &novs,
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 11).unwrap(),
        &HashMap::new(),
    )
    .expect("build_report debe funcionar");

    assert_eq!(res.novedades_incluidas, 1);

    let file = std::fs::File::open(&out).unwrap();
    let mut zip = zip::ZipArchive::new(file).unwrap();
    let mut doc = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut doc)
        .unwrap();

    assert!(doc.contains("OPERATIVO UNO"));
    assert!(!doc.contains("OPERATIVO DOS"));
}

/// Una foto referenciada que no está en el mapa de media debe omitirse con
/// un aviso, sin romper la generación.
#[test]
fn missing_photo_is_warned_not_fatal() {
    let novs = vec![Novedad {
        id: 1,
        headers: vec!["OPERATIVO".to_string()],
        body_raw: "Texto de la novedad.".to_string(),
        closing: None,
        photos: vec!["no-existe.png".to_string()],
        videos_skipped: Vec::new(),
        fecha_mencionada: None,
        include: true,
        is_orphan: false,
        body_formal: String::new(),
        sender: "A".to_string(),
    }];

    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("falta-foto.docx");
    let res = build_report(
        None,
        &out,
        &novs,
        NaiveDate::from_ymd_opt(2026, 10, 10).unwrap(),
        NaiveDate::from_ymd_opt(2026, 10, 11).unwrap(),
        &HashMap::new(),
    )
    .expect("build_report debe funcionar aunque falte la foto");

    assert_eq!(res.novedades_incluidas, 1);
    assert_eq!(res.images_embedded, 0);
    assert!(
        res.warnings.iter().any(|w| w.contains("no-existe.png")),
        "debe avisar de la foto ausente: {:?}",
        res.warnings
    );
}
