use chrono::NaiveDate;
use parte_diario_core::paths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::Path;

use crate::write_atomic;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub zip_sha256: String,
    #[serde(default)]
    pub txt_sha256: String,
    pub zip_name: String,
    pub report_start: String,
    pub report_end: String,
    pub generated_at: String,
    pub output_path: String,
    pub novedades: usize,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct History {
    #[serde(default)]
    pub entries: Vec<HistoryEntry>,
}

pub fn load_history() -> History {
    fs::read_to_string(paths::history_file())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .or_else(|| {
            fs::read_to_string(paths::legacy_history_file())
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
        })
        .unwrap_or_default()
}

pub fn save_history(h: &History) -> Result<(), String> {
    let text = toml::to_string_pretty(h).map_err(|e| e.to_string())?;
    write_atomic(&paths::history_file(), &text)
}

pub fn sha256_file(path: &Path) -> Result<String, String> {
    const BUF: usize = 64 * 1024;
    let mut f = fs::File::open(path).map_err(|e| format!("No se pudo abrir el archivo: {e}"))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; BUF];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("Error leyendo el archivo: {e}"))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex_of(&h.finalize()))
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex_of(&h.finalize())
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn normalize_txt(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn already_processed<'a>(
    h: &'a History,
    zip_hash: &str,
    txt_hash: &str,
) -> Option<&'a HistoryEntry> {
    h.entries
        .iter()
        .find(|e| e.zip_sha256 == zip_hash || (!txt_hash.is_empty() && e.txt_sha256 == txt_hash))
}

pub fn last_report_end(h: &History) -> Option<NaiveDate> {
    h.entries
        .iter()
        .filter_map(|e| NaiveDate::parse_from_str(&e.report_end, "%Y-%m-%d").ok())
        .max()
}

pub fn existing_report_for<'a>(h: &'a History, start: &str, end: &str) -> Option<&'a HistoryEntry> {
    h.entries
        .iter()
        .find(|e| e.report_start == start && e.report_end == end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normaliza_y_detecta_duplicados() {
        let a = normalize_txt("  uno \n\n dos \n");
        let b = normalize_txt("uno\ndos");
        assert_eq!(a, b);
        assert_eq!(sha256_bytes(a.as_bytes()), sha256_bytes(b.as_bytes()));
    }

    #[test]
    fn encuentra_el_ultimo_fin_de_reporte() {
        let h = History {
            entries: vec![
                HistoryEntry {
                    zip_sha256: "a".into(),
                    txt_sha256: String::new(),
                    zip_name: "a.zip".into(),
                    report_start: "2026-01-01".into(),
                    report_end: "2026-01-05".into(),
                    generated_at: "2026-01-05 08:00:00".into(),
                    output_path: "a.docx".into(),
                    novedades: 1,
                },
                HistoryEntry {
                    zip_sha256: "b".into(),
                    txt_sha256: String::new(),
                    zip_name: "b.zip".into(),
                    report_start: "2026-01-06".into(),
                    report_end: "2026-01-10".into(),
                    generated_at: "2026-01-10 08:00:00".into(),
                    output_path: "b.docx".into(),
                    novedades: 2,
                },
            ],
        };
        assert_eq!(last_report_end(&h), NaiveDate::from_ymd_opt(2026, 1, 10));
    }
}
