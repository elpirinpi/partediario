//! Parseo del archivo .txt exportado por WhatsApp ("Exportar chat").
//!
//! Soporta los dos formatos reales:
//! - FORMATO "CORCHETE" (Android, es-DO, con segundos):
//!   [21/7/26, 11:38:38 p. m.] Digitador N3: texto del mensaje...
//!   [21/7/26, 11:38:40 p. m.] Digitador N3: ‎<adjunto: IMG-001.jpg>
//! - FORMATO "GUION" (iOS u otras variantes, sin segundos):
//!   21/7/26, 23:38 - DIG-N3: texto del mensaje...
//!   21/7/26, 23:38 - DIG-N3: ‎IMG-001.jpg (archivo adjunto)
//!
//! Reglas:
//! - Las líneas de continuación (sin timestamp) pertenecen al mensaje anterior.
//! - Los adjuntos de video se marcan como tales (se excluyen del .docx, pero la
//!   novedad se conserva).
//! - Los mensajes de sistema de WhatsApp se ignoran.

use std::sync::OnceLock;

use chrono::{Datelike, NaiveDate, NaiveDateTime, NaiveTime};
use regex::Regex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatFormat {
    Bracket,
    Dash,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    Video,
    Other,
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub kind: AttachmentKind,
}

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub ts: Option<NaiveDateTime>,
    pub sender: String,
    pub text: String,
    pub attachment: Option<Attachment>,
    pub is_system: bool,
}

#[derive(Debug, Clone)]
pub struct ParsedChat {
    pub messages: Vec<ChatMessage>,
    pub format: ChatFormat,
}

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];
const VIDEO_EXTS: &[&str] = &["mp4", "mov", "3gp", "avi", "mkv", "webm"];

fn attachment_kind(name: &str) -> AttachmentKind {
    let lower = name.to_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or("");
    if IMAGE_EXTS.contains(&ext) {
        AttachmentKind::Image
    } else if VIDEO_EXTS.contains(&ext) {
        AttachmentKind::Video
    } else {
        AttachmentKind::Other
    }
}

/// Patrones de mensajes de sistema de WhatsApp (se ignoran).
const SYSTEM_PATTERNS: &[&str] = &[
    "cifrados de extremo a extremo",
    "cifrado de extremo a extremo",
    "los mensajes y las llamadas",
    "los mensajes que envías a este chat",
    "creó el grupo",
    "cambió el nombre del grupo",
    "cambió la foto del grupo",
    "cambió la descripción del grupo",
    "añadió a ",
    "eliminó a ",
    "salió del grupo",
    "se unió usando el enlace",
    "ahora eres administrador",
    "cambió los ajustes del grupo",
    "activó los mensajes temporales",
    "desactivó los mensajes temporales",
];

fn is_system_text(text: &str, sender: &str) -> bool {
    if sender.trim().is_empty() {
        return true;
    }
    let t = text.to_lowercase();
    SYSTEM_PATTERNS.iter().any(|p| t.contains(p))
}

/// Quita marcas invisibles de dirección (U+200E/U+200F) que WhatsApp antepone
/// a los marcadores de adjunto, y recorta espacios.
fn clean_invisible(s: &str) -> String {
    s.replace(['\u{200E}', '\u{200F}', '\u{FEFF}'], "")
        .trim()
        .to_string()
}

fn re_adjunto_bracket() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)<adjunto:\s*([^<>\s]+\.[A-Za-z0-9]{2,4})\s*>").unwrap())
}

fn re_adjunto_ios() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"(?i)([^<>\s]+\.[A-Za-z0-9]{2,4})\s*\((?:archivo adjunto|file attached)\)")
            .unwrap()
    })
}

fn re_adjunto_omitido() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r"(?i)(imagen omitida|video omitido|gif omitido|\(archivo omitido\)|image omitted|video omitted)",
        )
        .unwrap()
    })
}

/// Extrae el adjunto del cuerpo del mensaje y devuelve (adjunto, texto_limpio).
fn extract_attachment(text: &str) -> (Option<Attachment>, String) {
    // Android: <adjunto: NOMBRE>   |   iOS: NOMBRE (archivo adjunto) / (file attached)
    let re_bracket = re_adjunto_bracket();
    let re_ios = re_adjunto_ios();

    let mut found: Option<String> = None;
    if let Some(c) = re_bracket.captures(text) {
        found = Some(c[1].to_string());
    } else if let Some(c) = re_ios.captures(text) {
        found = Some(c[1].to_string());
    }

    if let Some(name) = found {
        // El nombre puede traer marcas invisibles de WhatsApp (U+200E/U+200F)
        // pegadas; sin limpiarlas nunca coincide con el archivo del .zip.
        let name = clean_invisible(&name);
        let kind = attachment_kind(&name);
        let rest = re_bracket.replace_all(text, "");
        let rest = re_ios.replace_all(&rest, "");
        let att = Attachment {
            filename: name,
            kind,
        };
        return (Some(att), clean_invisible(&rest));
    }

    // "imagen omitida" / "(archivo omitido)": no hay nombre de archivo útil.
    let rest = re_adjunto_omitido().replace_all(text, "");
    (None, clean_invisible(&rest))
}

fn norm_year(y: &str) -> i32 {
    match y.len() {
        2 => {
            let v: i32 = y.parse().unwrap_or(26);
            if v < 70 {
                2000 + v
            } else {
                1900 + v
            }
        }
        _ => y.parse().unwrap_or_else(|_| chrono::Local::now().year()),
    }
}

fn make_ts(y: i32, m: u32, d: u32, hh: u32, mm: u32) -> Option<NaiveDateTime> {
    let date = NaiveDate::from_ymd_opt(y, m, d)?;
    let time = NaiveTime::from_hms_opt(hh.min(23), mm.min(59), 0)?;
    Some(NaiveDateTime::new(date, time))
}

/// Parsea el contenido completo del .txt exportado.
pub fn parse_chat_txt(content: &str) -> ParsedChat {
    // [21/7/26, 11:38:38 p. m.] Nombre: texto   (segundos y a.m./p.m. opcionales)
    // También acepta 24 h: [21/7/26, 23:38:40] Nombre: texto
    let re_bracket = Regex::new(
        r"(?i)^\[(\d{1,2})/(\d{1,2})/(\d{2,4}),\s*(\d{1,2}):(\d{2})(?::(\d{2}))?\s*([ap])?\.?\s*m?\.?\s*\]\s*([^:]+):\s?(.*)$",
    )
    .unwrap();
    // 21/7/26, 23:38 - Nombre: texto
    // También acepta 12 h: 21/7/26, 11:38 p. m. - Nombre: texto
    let re_dash = Regex::new(
        r"(?i)^(\d{1,2})/(\d{1,2})/(\d{2,4}),\s*(\d{1,2}):(\d{2})(?::(\d{2}))?\s*([ap])?\.?\s*m?\.?\s*-\s*([^:]+):\s?(.*)$",
    )
    .unwrap();

    let mut messages: Vec<ChatMessage> = Vec::new();
    let mut format = ChatFormat::Unknown;
    // El export puede empezar con BOM.
    let content = content.trim_start_matches('\u{FEFF}');

    for raw_line in content.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            // Línea vacía: continuación del mensaje anterior (párrafo).
            if let Some(last) = messages.last_mut() {
                last.text.push('\n');
            }
            continue;
        }

        let mut parsed: Option<(Option<NaiveDateTime>, String, String)> = None;

        if let Some(c) = re_bracket.captures(line) {
            if format == ChatFormat::Unknown {
                format = ChatFormat::Bracket;
            }
            let (d, m) = (
                c[1].parse::<u32>().unwrap_or(1),
                c[2].parse::<u32>().unwrap_or(1),
            );
            let y = norm_year(&c[3]);
            let (mut hh, mm) = (
                c[4].parse::<u32>().unwrap_or(0),
                c[5].parse::<u32>().unwrap_or(0),
            );
            let ampm = c.get(7).map(|m| m.as_str()).unwrap_or("");
            if ampm.eq_ignore_ascii_case("p") && hh < 12 {
                hh += 12;
            }
            if ampm.eq_ignore_ascii_case("a") && hh == 12 {
                hh = 0;
            }
            let ts = make_ts(y, m, d, hh, mm);
            parsed = Some((ts, c[8].trim().to_string(), c[9].to_string()));
        } else if let Some(c) = re_dash.captures(line) {
            if format == ChatFormat::Unknown {
                format = ChatFormat::Dash;
            }
            let (d, m) = (
                c[1].parse::<u32>().unwrap_or(1),
                c[2].parse::<u32>().unwrap_or(1),
            );
            let y = norm_year(&c[3]);
            let (mut hh, mm) = (
                c[4].parse::<u32>().unwrap_or(0),
                c[5].parse::<u32>().unwrap_or(0),
            );
            let ampm = c.get(7).map(|m| m.as_str()).unwrap_or("");
            if ampm.eq_ignore_ascii_case("p") && hh < 12 {
                hh += 12;
            }
            if ampm.eq_ignore_ascii_case("a") && hh == 12 {
                hh = 0;
            }
            let ts = make_ts(y, m, d, hh, mm);
            parsed = Some((ts, c[8].trim().to_string(), c[9].to_string()));
        }

        match parsed {
            Some((ts, sender, body)) => {
                let (attachment, text) = extract_attachment(&body);
                let is_system = is_system_text(&text, &sender);
                messages.push(ChatMessage {
                    ts,
                    sender,
                    text,
                    attachment,
                    is_system,
                });
            }
            None => {
                // Línea de continuación: pertenece al mensaje anterior.
                if let Some(last) = messages.last_mut() {
                    if !last.text.is_empty() && !last.text.ends_with('\n') {
                        last.text.push('\n');
                    }
                    last.text.push_str(line.trim());
                }
            }
        }
    }

    messages.retain(|m| !m.is_system);
    ParsedChat { messages, format }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bracket_format() {
        let txt = "[21/7/26, 11:38:38 p. m.] Digitador N3: Siendo las 18:00 horas\ncontinuación de la nota\n[21/7/26, 11:38:40 p. m.] Digitador N3: ‎<adjunto: IMG-20260721-WA0001.jpg>";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.format, ChatFormat::Bracket);
        assert_eq!(chat.messages.len(), 2);
        assert!(chat.messages[0].text.contains("continuación"));
        assert_eq!(
            chat.messages[1].attachment.as_ref().unwrap().filename,
            "IMG-20260721-WA0001.jpg"
        );
        assert_eq!(
            chat.messages[1].attachment.as_ref().unwrap().kind,
            AttachmentKind::Image
        );
    }

    #[test]
    fn parse_dash_format() {
        let txt = "21/7/26, 23:38 - DIG-N3: ‎IMG-20260721-WA0001.jpg (archivo adjunto)\n21/7/26, 23:39 - DIG-N3: ‎VID-20260721-WA0002.mp4 (archivo adjunto)";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.format, ChatFormat::Dash);
        assert_eq!(chat.messages.len(), 2);
        assert_eq!(
            chat.messages[1].attachment.as_ref().unwrap().kind,
            AttachmentKind::Video
        );
    }

    #[test]
    fn parse_bracket_24h_without_ampm() {
        let txt = "[21/7/26, 23:38:40] DIG-N3: Nota en formato 24 h.";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.format, ChatFormat::Bracket);
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(
            chat.messages[0].ts.unwrap().time(),
            NaiveTime::from_hms_opt(23, 38, 0).unwrap()
        );
        assert_eq!(chat.messages[0].sender, "DIG-N3");
    }

    #[test]
    fn parse_bracket_uppercase_ampm() {
        let txt = "[21/7/26, 11:38:40 PM] DIG-N3: Nota con PM mayúscula.";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.messages.len(), 1);
        assert_eq!(
            chat.messages[0].ts.unwrap().time(),
            NaiveTime::from_hms_opt(23, 38, 0).unwrap()
        );
    }

    #[test]
    fn parse_dash_12h_ampm() {
        let txt = "21/7/26, 11:38 p. m. - DIG-N3: Nota con p. m.";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.format, ChatFormat::Dash);
        assert_eq!(
            chat.messages[0].ts.unwrap().time(),
            NaiveTime::from_hms_opt(23, 38, 0).unwrap()
        );
    }

    #[test]
    fn system_messages_ignored() {
        let txt = "[21/7/26, 11:38:38 p. m.] Digitador N3: Nota real\n[21/7/26, 11:39:00 p. m.] : Los mensajes están cifrados de extremo a extremo";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.messages.len(), 1);
    }
}

#[cfg(test)]
mod real_format_tests {
    use super::*;

    /// Formato del export real del usuario: guion 24h sin a.m./p.m.,
    /// adjunto como "NOMBRE (archivo adjunto)" con U+200E pegado al nombre.
    /// El nombre extraído debe quedar limpio para coincidir con el .zip.
    #[test]
    fn dash_24h_con_u200e_en_adjunto() {
        let txt = "8/10/2026, 02:37 - DIG-N3: \u{200E}IMG-20261008-WA0076.jpg (archivo adjunto)\n\
                   8/10/2026, 02:38 - DIG-N3: Siendo las 18:00 horas del día 07/10/2026, nota de prueba.\n\
                   Es lo que informo para su conocimiento.";
        let chat = parse_chat_txt(txt);
        assert_eq!(chat.format, ChatFormat::Dash);
        assert_eq!(chat.messages.len(), 2);
        let att = chat.messages[0].attachment.as_ref().unwrap();
        assert_eq!(att.filename, "IMG-20261008-WA0076.jpg");
        assert!(!att.filename.contains('\u{200E}'));
        assert_eq!(att.kind, AttachmentKind::Image);
    }
}
