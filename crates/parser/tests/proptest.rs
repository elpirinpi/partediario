//! Property-based tests para el parser de WhatsApp.
//!
//! Estos tests generan entradas aleatorias para verificar que el parser:
//! - Nunca hace panic (robustez)
//! - Mantiene invariantes clave (no pierde mensajes, fechas válidas, etc.)
//! - Maneja correctamente casos edge

use chrono::{Datelike, NaiveDate, Timelike};
use proptest::prelude::*;

use parte_diario_parser::{grouping, parser};

// ---------------------------------------------------------------------------
// Estrategias (generadores de entrada)
// ---------------------------------------------------------------------------

/// Timestamps válidos en formato corchete: `[DD/MM/YY, HH:MM:SS p. m.]`.
fn arb_bracket_timestamp() -> impl Strategy<Value = String> {
    (
        1u32..=31, // día
        1u32..=12, // mes
        20..=99,   // año (2 dígitos) -> se normaliza al parsear
        1u32..=12, // hora en 12 h
        0u32..=59, // minutos
        0u32..=59, // segundos
        prop::sample::select(vec!["a. m.", "p. m.", "a.m.", "p.m.", "AM", "PM"]),
    )
        .prop_map(|(d, m, y, h, min, s, ampm)| {
            format!("[{d:02}/{m:02}/{y:02}, {h:02}:{min:02}:{s:02} {ampm}]")
        })
}

/// Timestamps válidos en formato guion (iOS): `DD/MM/YY, HH:MM -`.
fn arb_dash_timestamp() -> impl Strategy<Value = String> {
    (
        1u32..=31,
        1u32..=12,
        20..=99,
        0u32..=23,                                                      // hora 24 h
        0u32..=59,                                                      // minutos
        prop::option::of(0u32..=59),                                    // segundos opcional
        prop::option::of(prop::sample::select(vec!["a. m.", "p. m."])), // am/pm opcional
    )
        .prop_map(|(d, m, y, h, min, sec, ampm)| {
            let sec_part = sec.map(|s| format!(":{s:02}")).unwrap_or_default();
            let ampm_part = ampm.map(|a| format!(" {a}")).unwrap_or_default();
            format!("{d:02}/{m:02}/{y:02}, {h:02}:{min:02}{sec_part}{ampm_part} -")
        })
}

/// Remitente realista (incluye el caso vacío del mensaje de sistema).
fn arb_sender() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "Digitador N3".to_string(),
        "Agente Pérez".to_string(),
        "DIG-N3".to_string(),
        "Oficial García".to_string(),
        "Sargento López".to_string(),
        "Cabo Rodríguez".to_string(),
        String::new(),
    ])
}

/// Remitente garantizado no vacío (para los tests de "una línea = un mensaje").
fn arb_clean_sender() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "Digitador N3".to_string(),
        "Agente Pérez".to_string(),
        "DIG-N3".to_string(),
        "Oficial García".to_string(),
    ])
}

/// Texto neutro construido con vocabulario policial que nunca coincide con las
/// frases de sistema de WhatsApp (así cada línea produce exactamente un mensaje).
fn arb_clean_text() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "nota",
            "operativo",
            "bocina",
            "vehiculo",
            "via",
            "hora",
            "dia",
            "informe",
            "patrulla",
            "incautacion",
            "musica",
            "ruido",
            "calle",
        ]),
        0..=10,
    )
    .prop_map(|words| words.join(" "))
}

/// Línea "limpia" en formato corchete: siempre parseable como un mensaje.
fn arb_clean_line_bracket() -> impl Strategy<Value = String> {
    (
        arb_bracket_timestamp(),
        arb_clean_sender(),
        arb_clean_text(),
    )
        .prop_map(|(ts, sender, text)| format!("{} {}: {}", ts, sender.trim(), text))
}

/// Línea "limpia" en formato guion: siempre parseable como un mensaje.
fn arb_clean_line_dash() -> impl Strategy<Value = String> {
    (arb_dash_timestamp(), arb_clean_sender(), arb_clean_text())
        .prop_map(|(ts, sender, text)| format!("{} {}: {}", ts, sender.trim(), text))
}

/// Línea de continuación: empieza por `/` para que nunca pueda confundirse
/// con un timestamp (que empieza por `[` o por un dígito).
fn arb_continuation_line() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "texto",
            "mas",
            "linea",
            "continuacion",
            "ejemplo",
            "detalle",
            "nota",
        ]),
        1..=8,
    )
    .prop_map(|words| format!("// {}", words.join(" ")))
}

// ---------------------------------------------------------------------------
// Tests del parser
// ---------------------------------------------------------------------------

proptest! {
    /// El parser NUNCA debe hacer panic, sin importar la entrada.
    #[test]
    fn parse_chat_txt_never_panics(input in "(?s).*") {
        let _ = parser::parse_chat_txt(&input);
    }

    /// Formato corchete: cada línea limpia produce exactamente un mensaje.
    #[test]
    fn bracket_format_parses_each_timestamped_line(
        lines in prop::collection::vec(arb_clean_line_bracket(), 1..=50)
    ) {
        let input = lines.join("\n");
        let chat = parser::parse_chat_txt(&input);

        prop_assert_eq!(chat.format, parser::ChatFormat::Bracket);
        prop_assert_eq!(
            chat.messages.len(),
            lines.len(),
            "se perdieron mensajes en formato corchete: {:?}",
            chat
        );
    }

    /// Formato guion: cada línea limpia produce exactamente un mensaje.
    #[test]
    fn dash_format_parses_each_timestamped_line(
        lines in prop::collection::vec(arb_clean_line_dash(), 1..=50)
    ) {
        let input = lines.join("\n");
        let chat = parser::parse_chat_txt(&input);

        prop_assert_eq!(chat.format, parser::ChatFormat::Dash);
        prop_assert_eq!(
            chat.messages.len(),
            lines.len(),
            "se perdieron mensajes en formato guion: {:?}",
            chat
        );
    }

    /// Las líneas de continuación se adjuntan al mensaje anterior.
    #[test]
    fn continuation_lines_attach_to_previous_message(
        first in arb_clean_line_bracket(),
        continuations in prop::collection::vec(arb_continuation_line(), 0..=5),
        second in arb_clean_line_bracket(),
    ) {
        let mut lines = vec![first];
        lines.extend(continuations.clone());
        lines.push(second);
        let input = lines.join("\n");

        let chat = parser::parse_chat_txt(&input);

        // Exactamente dos mensajes: las continuaciones se fusionan en el primero.
        prop_assert_eq!(chat.messages.len(), 2, "input: {:?}", input);

        for cont in &continuations {
            prop_assert!(
                chat.messages[0].text.contains(cont),
                "falta la continuación {:?} en {:?}",
                cont,
                chat.messages[0].text
            );
        }
    }

    /// Las fechas parseadas son válidas (día 1-31, mes 1-12, hora 0-23).
    #[test]
    fn parsed_dates_are_valid(lines in prop::collection::vec(arb_clean_line_bracket(), 1..=30)) {
        let input = lines.join("\n");
        let chat = parser::parse_chat_txt(&input);

        for msg in &chat.messages {
            if let Some(ts) = msg.ts {
                prop_assert!((1..=31).contains(&ts.day()), "día inválido: {}", ts.day());
                prop_assert!((1..=12).contains(&ts.month()), "mes inválido: {}", ts.month());
                prop_assert!(ts.hour() <= 23, "hora inválida: {}", ts.hour());
                prop_assert!(ts.minute() <= 59, "minuto inválido: {}", ts.minute());
            }
        }
    }

    /// Los adjuntos de imagen se detectan y clasifican correctamente.
    #[test]
    fn image_attachments_detected_and_classified(
        filename in prop::string::string_regex(r"IMG-\d{8}-WA\d{4}\.(jpg|jpeg|png|webp)").unwrap()
    ) {
        let line = format!("[21/7/26, 11:38:38 p. m.] Agente: ‎<adjunto: {filename}>");
        let chat = parser::parse_chat_txt(&line);

        prop_assert_eq!(chat.messages.len(), 1);
        let att = chat.messages[0]
            .attachment
            .as_ref()
            .expect("debe tener adjunto");
        prop_assert_eq!(&att.filename, &filename);
        prop_assert_eq!(att.kind, parser::AttachmentKind::Image);
    }

    /// Los adjuntos de video se detectan y clasifican correctamente.
    #[test]
    fn video_attachments_detected_and_classified(
        filename in prop::string::string_regex(r"VID-\d{8}-WA\d{4}\.(mp4|mov|3gp)").unwrap()
    ) {
        let line = format!("[21/7/26, 11:38:38 p. m.] Agente: ‎<adjunto: {filename}>");
        let chat = parser::parse_chat_txt(&line);

        prop_assert_eq!(chat.messages.len(), 1);
        let att = chat.messages[0]
            .attachment
            .as_ref()
            .expect("debe tener adjunto");
        prop_assert_eq!(&att.filename, &filename);
        prop_assert_eq!(att.kind, parser::AttachmentKind::Video);
    }

    /// Los mensajes de sistema se filtran de la salida final.
    #[test]
    fn system_messages_are_filtered_out(
        sys_msg in prop::sample::select(vec![
            "Los mensajes están cifrados de extremo a extremo",
            "creó el grupo",
            "cambió el nombre del grupo",
            "añadió a Juan",
            "eliminó a Pedro",
            "salió del grupo",
        ])
    ) {
        let line = format!("[21/7/26, 11:38:38 p. m.] : {sys_msg}");
        let chat = parser::parse_chat_txt(&line);

        prop_assert!(
            chat.messages.is_empty(),
            "mensaje de sistema no filtrado: {} -> {:?}",
            sys_msg,
            chat
        );
    }

    /// Los nombres de remitente con caracteres Unicode se preservan.
    #[test]
    fn unicode_sender_names_preserved(
        name in prop::string::string_regex(r"[\p{L}\p{N} \-_.]{1,50}").unwrap()
    ) {
        let line = format!("[21/7/26, 11:38:38 p. m.] {name}: Hola");
        let chat = parser::parse_chat_txt(&line);

        if !chat.messages.is_empty() && !name.trim().is_empty() {
            prop_assert_eq!(chat.messages[0].sender.as_str(), name.trim());
        }
    }

    /// El parser es idempotente: parsear dos veces da el mismo resultado.
    #[test]
    fn parser_is_idempotent(input in "(?s).*") {
        let chat1 = parser::parse_chat_txt(&input);
        let chat2 = parser::parse_chat_txt(&input);
        prop_assert_eq!(format!("{:?}", chat1), format!("{:?}", chat2));
    }
}

// ---------------------------------------------------------------------------
// Tests de la agrupación (grouping)
// ---------------------------------------------------------------------------

/// Genera un `ParsedChat` válido (mensajes aleatorios con adjuntos opcionales).
fn arb_parsed_chat() -> impl Strategy<Value = parser::ParsedChat> {
    prop::collection::vec(
        (
            prop::option::of(
                (1u32..=28, 1u32..=12, 2020i32..=2030, 0u32..=23, 0u32..=59).prop_map(
                    |(d, m, y, h, min)| {
                        NaiveDate::from_ymd_opt(y, m, d)
                            .and_then(|date| date.and_hms_opt(h, min, 0))
                            .unwrap()
                    },
                ),
            ),
            arb_sender(),
            prop::string::string_regex(r"[A-Za-z0-9 ]{0,60}").unwrap(),
            prop::option::of(prop::sample::select(vec![
                parser::AttachmentKind::Image,
                parser::AttachmentKind::Video,
                parser::AttachmentKind::Other,
            ])),
            prop::bool::weighted(0.05), // 5 % de mensajes marcados como sistema
            any::<u32>(),
        )
            .prop_map(|(ts, sender, text, att_kind, is_system, n)| {
                let attachment = att_kind.map(|kind| parser::Attachment {
                    filename: format!("IMG-{n}.jpg"),
                    kind,
                });
                parser::ChatMessage {
                    ts,
                    sender,
                    text,
                    attachment,
                    is_system,
                }
            }),
        1..=20,
    )
    .prop_map(|messages| parser::ParsedChat {
        messages,
        format: parser::ChatFormat::Bracket,
    })
}

proptest! {
    /// `group_novedades` nunca hace panic.
    #[test]
    fn group_novedades_never_panics(
        chat in arb_parsed_chat(),
        media_names in prop::collection::hash_set(
            prop::string::string_regex(r"[A-Za-z0-9_-]+\.(jpg|png|mp4)").unwrap(),
            0..=10,
        )
    ) {
        let _ = grouping::group_novedades(&chat, &media_names);
    }

    /// Cada novedad tiene un ID único y creciente.
    #[test]
    fn novedades_have_unique_increasing_ids(
        chat in arb_parsed_chat(),
        media_names in prop::collection::hash_set(
            prop::string::string_regex(r"[A-Za-z0-9_-]+\.(jpg|png)").unwrap(),
            0..=10,
        )
    ) {
        let novs = grouping::group_novedades(&chat, &media_names);

        let mut prev_id = 0;
        for n in &novs {
            prop_assert!(
                n.id > prev_id,
                "IDs no crecientes: {} -> {}",
                prev_id,
                n.id
            );
            prev_id = n.id;
        }
    }

    /// Las fotos referenciadas en el chat y presentes en el .zip se asignan
    /// a alguna novedad (nunca se descartan en silencio).
    #[test]
    fn referenced_photos_assigned_to_novedades(
        chat in arb_parsed_chat(),
        media_names in prop::collection::hash_set(
            prop::string::string_regex(r"IMG-\d{8}-WA\d{4}\.(jpg|png)").unwrap(),
            1..=5,
        )
    ) {
        let photo_name = media_names
            .iter()
            .next()
            .cloned()
            .expect("el set tiene al menos un elemento");

        // Forzamos el primer mensaje a abrir una novedad con esa foto adjunta.
        let mut chat_with_photo = chat.clone();
        if let Some(msg) = chat_with_photo.messages.first_mut() {
            msg.text = "Siendo las 20:00 horas del día 10/10/2026, operativo.".to_string();
            msg.attachment = Some(parser::Attachment {
                filename: photo_name,
                kind: parser::AttachmentKind::Image,
            });
        }

        let novs = grouping::group_novedades(&chat_with_photo, &media_names);
        let total_photos: usize = novs.iter().map(|n| n.photos.len()).sum();

        prop_assert!(
            total_photos > 0,
            "foto referenciada y presente no se asignó a ninguna novedad"
        );
    }

    /// Los videos se marcan como omitidos (`videos_skipped`) pero la novedad
    /// se conserva y no se rompe la agrupación.
    #[test]
    fn videos_are_skipped_not_lost(
        chat in arb_parsed_chat(),
        media_names in prop::collection::hash_set(
            prop::string::string_regex(r"VID-\d{8}-WA\d{4}\.(mp4|mov)").unwrap(),
            1..=3,
        )
    ) {
        let video_name = media_names
            .iter()
            .next()
            .cloned()
            .expect("el set tiene al menos un elemento");

        // El primer mensaje abre una novedad y lleva el video adjunto.
        let mut chat_with_video = chat.clone();
        if let Some(msg) = chat_with_video.messages.first_mut() {
            msg.text = "Siendo las 20:00 horas del día 10/10/2026, operativo.".to_string();
            msg.attachment = Some(parser::Attachment {
                filename: video_name,
                kind: parser::AttachmentKind::Video,
            });
        }

        let novs = grouping::group_novedades(&chat_with_video, &media_names);
        let total_videos: usize = novs.iter().map(|n| n.videos_skipped.len()).sum();

        prop_assert!(!novs.is_empty(), "la novedad con el video se perdió");
        prop_assert!(total_videos > 0, "video no registrado como omitido");
    }
}

// ---------------------------------------------------------------------------
// Tests deterministas de metadatos (no property-based, pero parte del suite)
// ---------------------------------------------------------------------------

/// La fecha mencionada se extrae del cuerpo de la nota.
#[test]
fn fecha_mencionada_extracted_from_text() {
    let chat = parser::ParsedChat {
        messages: vec![parser::ChatMessage {
            ts: None,
            sender: "Agente".to_string(),
            text: "Siendo las 20:00 horas del día 15/10/2026, se realizó operativo. Es lo que informo."
                .to_string(),
            attachment: None,
            is_system: false,
        }],
        format: parser::ChatFormat::Bracket,
    };

    let novs = grouping::group_novedades(&chat, &std::collections::HashSet::new());

    assert_eq!(novs.len(), 1);
    assert_eq!(
        novs[0].fecha_mencionada,
        Some(NaiveDate::from_ymd_opt(2026, 10, 15).unwrap())
    );
}

/// `infer_report_range` detecta la fecha mayoritaria y las novedades atípicas.
#[test]
fn infer_report_range_detects_majority_and_outliers() {
    fn novedad(id: usize, fecha: NaiveDate) -> grouping::Novedad {
        grouping::Novedad {
            id,
            headers: Vec::new(),
            body_raw: format!("Siendo las 20:00 del día {fecha}."),
            closing: Some("Es lo que informo.".to_string()),
            photos: Vec::new(),
            videos_skipped: Vec::new(),
            fecha_mencionada: Some(fecha),
            include: true,
            is_orphan: false,
            body_formal: String::new(),
            sender: "A".to_string(),
        }
    }

    let dia_mayor = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
    let dia_outlier = NaiveDate::from_ymd_opt(2026, 10, 11).unwrap();

    let mut novs: Vec<grouping::Novedad> = (1..=3).map(|i| novedad(i, dia_mayor)).collect();
    novs.push(novedad(4, dia_outlier));

    let (majority, outliers) = grouping::infer_report_range(&novs);

    assert_eq!(majority, Some(dia_mayor));
    assert_eq!(outliers, vec![4]);
}
