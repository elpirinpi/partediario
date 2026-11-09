//! Rutas de datos del usuario. Nunca se escribe junto al ejecutable, para que
//! una actualización no borre configuración, plantillas, historial ni logs.

use std::path::PathBuf;

/// Carpeta raíz de datos: `%APPDATA%\parte-diario` en Windows.
pub fn app_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("parte-diario")
}

pub fn config_dir() -> PathBuf {
    app_dir()
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn legacy_config_file() -> PathBuf {
    config_dir().join("config.json")
}

pub fn history_file() -> PathBuf {
    config_dir().join("history.toml")
}

pub fn legacy_history_file() -> PathBuf {
    config_dir().join("history.json")
}

pub fn logs_dir() -> PathBuf {
    app_dir().join("logs")
}

pub fn backups_dir() -> PathBuf {
    app_dir().join("backups")
}

pub fn updates_dir() -> PathBuf {
    app_dir().join("updates")
}

pub fn templates_dir() -> PathBuf {
    app_dir().join("templates")
}

pub fn secrets_file() -> PathBuf {
    if let Ok(p) = std::env::var("PARTE_DIARIO_SECRETS_PATH") {
        return PathBuf::from(p);
    }
    app_dir().join("secrets.toml")
}

/// Crea la carpeta si no existe y devuelve un error legible si no se puede.
pub fn ensure_dir(path: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("No se pudo crear la carpeta {}: {}", path.display(), e))
}
