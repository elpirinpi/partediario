pub mod config;
pub mod history;
pub mod logging;
pub mod secrets;

use std::fs;
use std::path::Path;

pub use config::{load_config, save_config, Config};
pub use history::{
    already_processed, existing_report_for, last_report_end, load_history, normalize_txt,
    save_history, sha256_bytes, sha256_file, History, HistoryEntry,
};

pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| "La ruta no tiene carpeta padre".to_string())?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Nombre de archivo inválido".to_string())?;
    let tmp = dir.join(format!(".{}.{}.tmp", name, std::process::id()));
    fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_) if path.exists() => {
            fs::remove_file(path).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                e.to_string()
            })?;
            fs::rename(&tmp, path).map_err(|e| {
                let _ = fs::remove_file(&tmp);
                e.to_string()
            })
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e.to_string())
        }
    }
}
