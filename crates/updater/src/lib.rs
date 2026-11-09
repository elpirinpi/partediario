use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use parte_diario_core::paths;
use parte_diario_core::zip_safety::validate_zip;
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

const USER_AGENT: &str = "parte-diario-updater";
const GITHUB_API_BASE: &str = "https://api.github.com";

#[derive(Debug, Error)]
pub enum UpdaterError {
    #[error("No se pudo consultar GitHub: {0}")]
    Network(String),
    #[error("Respuesta inválida de GitHub: {0}")]
    InvalidResponse(String),
    #[error("No hay ninguna versión publicada")]
    NoRelease,
    #[error("El archivo de actualización no es válido: {0}")]
    InvalidPackage(String),
    #[error("El checksum no coincide. Se canceló la actualización")]
    ChecksumMismatch,
    #[error("Error de archivos: {0}")]
    Io(String),
}

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub notes: String,
    pub release_url: String,
    pub asset_url: String,
    pub asset_name: String,
    pub checksum_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

pub fn check_for_update(
    repository: &str,
    current: &str,
) -> Result<Option<UpdateInfo>, UpdaterError> {
    if repository.trim().is_empty() {
        return Err(UpdaterError::InvalidResponse(
            "Falta configurar el repositorio de actualizaciones".to_string(),
        ));
    }
    let url = format!(
        "{GITHUB_API_BASE}/repos/{}/releases/latest",
        repository.trim_matches('/')
    );
    check_for_update_at(&url, current)
}

/// Igual que [`check_for_update`], pero contra una URL de release concreta.
///
/// Se expone para pruebas de integración (servidor local) y para usar un proxy
/// o un espejo de la API de GitHub.
pub fn check_for_update_at(
    release_api_url: &str,
    current: &str,
) -> Result<Option<UpdateInfo>, UpdaterError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;
    let response = client
        .get(release_api_url)
        .send()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!(
            "GitHub respondió {}",
            response.status()
        )));
    }
    let release: GithubRelease = response
        .json()
        .map_err(|e| UpdaterError::InvalidResponse(e.to_string()))?;
    let latest_text = release.tag_name.trim_start_matches('v');
    let latest = Version::parse(latest_text)
        .map_err(|e| UpdaterError::InvalidResponse(format!("Versión {latest_text}: {e}")))?;
    let actual = Version::parse(current.trim_start_matches('v'))
        .map_err(|e| UpdaterError::InvalidResponse(format!("Versión actual: {e}")))?;
    if latest <= actual {
        return Ok(None);
    }

    let asset = release
        .assets
        .iter()
        .find(|a| a.name.to_lowercase().ends_with(".zip"))
        .ok_or_else(|| UpdaterError::InvalidPackage("El release no incluye un .zip".to_string()))?;
    let checksum_url = release
        .assets
        .iter()
        .find(|a| {
            a.name
                .eq_ignore_ascii_case(&format!("{}.sha256", asset.name))
        })
        .map(|a| a.browser_download_url.clone());

    Ok(Some(UpdateInfo {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        notes: release.body,
        release_url: release.html_url,
        asset_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
        checksum_url,
    }))
}

pub fn download_and_prepare(info: &UpdateInfo) -> Result<PathBuf, UpdaterError> {
    download_and_prepare_to_dir(&paths::updates_dir(), info)
}

/// Descarga y prepara la actualización dentro de `dir`.
///
/// Se expone para pruebas de integración; la aplicación usa
/// [`download_and_prepare`], que emplea la carpeta de actualizaciones del usuario.
pub fn download_and_prepare_to_dir(dir: &Path, info: &UpdateInfo) -> Result<PathBuf, UpdaterError> {
    paths::ensure_dir(dir).map_err(UpdaterError::Io)?;
    // El nombre viene de la API (o de un espejo): se usa solo el nombre de
    // archivo para impedir que un valor como "../x" escriba fuera de `dir`.
    let file_name = Path::new(&info.asset_name)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("actualizacion.zip");
    let zip_path = dir.join(file_name);
    let bytes = download(&info.asset_url)?;
    let expected = expected_checksum(info)?;
    if !expected.is_empty() {
        let actual = hex_of(&Sha256::digest(&bytes));
        if !actual.eq_ignore_ascii_case(&expected) {
            return Err(UpdaterError::ChecksumMismatch);
        }
    }
    fs::write(&zip_path, &bytes).map_err(|e| UpdaterError::Io(e.to_string()))?;
    let staging = dir.join("staging");
    let _ = fs::remove_dir_all(&staging);
    extract_exe(&zip_path, &staging)?;
    Ok(staging.join("parte-diario.exe"))
}

pub fn apply_on_exit(new_exe: &Path) -> Result<(), UpdaterError> {
    let current = std::env::current_exe().map_err(|e| UpdaterError::Io(e.to_string()))?;
    let backup_dir = paths::backups_dir();
    paths::ensure_dir(&backup_dir).map_err(UpdaterError::Io)?;
    let backup = backup_dir.join(format!(
        "parte-diario-{}.exe",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    let script = write_update_script(&current, new_exe, &backup)?;
    std::process::Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("")
        .arg("/MIN")
        .arg(&script)
        .spawn()
        .map_err(|e| UpdaterError::Io(e.to_string()))?;
    Ok(())
}

pub fn rollback() -> Result<PathBuf, UpdaterError> {
    let backup = latest_backup(&paths::backups_dir())?;
    let current = std::env::current_exe().map_err(|e| UpdaterError::Io(e.to_string()))?;
    let script = write_rollback_script(&current, &backup)?;
    std::process::Command::new("cmd")
        .arg("/C")
        .arg("start")
        .arg("")
        .arg("/MIN")
        .arg(&script)
        .spawn()
        .map_err(|e| UpdaterError::Io(e.to_string()))?;
    Ok(backup)
}

/// Devuelve el respaldo `.exe` más reciente de `backup_dir`.
///
/// Los nombres incluyen la marca de tiempo (`parte-diario-YYYYMMDD-HHMMSS.exe`),
/// por lo que el orden lexicográfico coincide con el orden cronológico.
fn latest_backup(backup_dir: &Path) -> Result<PathBuf, UpdaterError> {
    let mut backups: Vec<PathBuf> = fs::read_dir(backup_dir)
        .map_err(|e| UpdaterError::Io(e.to_string()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("exe"))
        .collect();
    backups.sort();
    backups
        .pop()
        .ok_or_else(|| UpdaterError::InvalidPackage("No hay respaldos disponibles".to_string()))
}

fn download(url: &str) -> Result<Vec<u8>, UpdaterError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;
    let response = client
        .get(url)
        .send()
        .map_err(|e| UpdaterError::Network(e.to_string()))?;
    if !response.status().is_success() {
        return Err(UpdaterError::Network(format!(
            "Descarga falló con {}",
            response.status()
        )));
    }
    response
        .bytes()
        .map(|b| b.to_vec())
        .map_err(|e| UpdaterError::Network(e.to_string()))
}

fn expected_checksum(info: &UpdateInfo) -> Result<String, UpdaterError> {
    let Some(url) = &info.checksum_url else {
        return Ok(String::new());
    };
    let bytes = download(url)?;
    let text = String::from_utf8_lossy(&bytes);
    let first = text
        .split_whitespace()
        .next()
        .ok_or_else(|| UpdaterError::InvalidPackage("Checksum vacío".to_string()))?;
    Ok(first.to_string())
}

fn extract_exe(zip_path: &Path, staging: &Path) -> Result<(), UpdaterError> {
    let bytes = fs::read(zip_path).map_err(|e| UpdaterError::Io(e.to_string()))?;
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| UpdaterError::InvalidPackage(e.to_string()))?;
    validate_zip(&mut archive).map_err(UpdaterError::InvalidPackage)?;
    paths::ensure_dir(staging).map_err(UpdaterError::Io)?;
    let mut found = false;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| UpdaterError::InvalidPackage(e.to_string()))?;
        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        if name.file_name().and_then(|n| n.to_str()) != Some("parte-diario.exe") {
            continue;
        }
        let target = staging.join("parte-diario.exe");
        let mut out = File::create(&target).map_err(|e| UpdaterError::Io(e.to_string()))?;
        std::io::copy(&mut entry, &mut out).map_err(|e| UpdaterError::Io(e.to_string()))?;
        found = true;
        break;
    }
    if !found {
        return Err(UpdaterError::InvalidPackage(
            "El .zip no contiene parte-diario.exe".to_string(),
        ));
    }
    Ok(())
}

fn write_update_script(
    current: &Path,
    new_exe: &Path,
    backup: &Path,
) -> Result<PathBuf, UpdaterError> {
    let dir = paths::updates_dir();
    paths::ensure_dir(&dir).map_err(UpdaterError::Io)?;
    let script = dir.join("aplicar-actualizacion.cmd");
    let content = update_script_content(current, new_exe, backup);
    write_script(&script, &content)?;
    Ok(script)
}

fn update_script_content(current: &Path, new_exe: &Path, backup: &Path) -> String {
    format!(
        "@echo off\r\n\
         setlocal\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         :esperar\r\n\
         move /y \"{current}\" \"{backup}\" >nul 2>&1\r\n\
         if errorlevel 1 (\r\n\
           timeout /t 1 /nobreak >nul\r\n\
           goto esperar\r\n\
         )\r\n\
         copy /y \"{new}\" \"{current}\" >nul\r\n\
         if errorlevel 1 (\r\n\
           copy /y \"{backup}\" \"{current}\" >nul\r\n\
           start \"\" \"{current}\"\r\n\
           exit /b 1\r\n\
         )\r\n\
         start \"\" \"{current}\"\r\n\
         del \"{new}\" >nul 2>&1\r\n\
         del \"%~f0\" >nul 2>&1\r\n",
        current = current.display(),
        backup = backup.display(),
        new = new_exe.display()
    )
}

fn write_rollback_script(current: &Path, backup: &Path) -> Result<PathBuf, UpdaterError> {
    let dir = paths::updates_dir();
    paths::ensure_dir(&dir).map_err(UpdaterError::Io)?;
    let script = dir.join("restaurar-respaldo.cmd");
    let content = rollback_script_content(current, backup);
    write_script(&script, &content)?;
    Ok(script)
}

fn rollback_script_content(current: &Path, backup: &Path) -> String {
    format!(
        "@echo off\r\n\
         setlocal\r\n\
         timeout /t 2 /nobreak >nul\r\n\
         :esperar\r\n\
         copy /y \"{backup}\" \"{current}\" >nul 2>&1\r\n\
         if errorlevel 1 (\r\n\
           timeout /t 1 /nobreak >nul\r\n\
           goto esperar\r\n\
         )\r\n\
         start \"\" \"{current}\"\r\n\
         del \"%~f0\" >nul 2>&1\r\n",
        current = current.display(),
        backup = backup.display()
    )
}

fn write_script(path: &Path, content: &str) -> Result<(), UpdaterError> {
    let mut file = File::create(path).map_err(|e| UpdaterError::Io(e.to_string()))?;
    file.write_all(content.as_bytes())
        .map_err(|e| UpdaterError::Io(e.to_string()))
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub fn read_checksum(path: &Path) -> Result<String, UpdaterError> {
    let mut text = String::new();
    File::open(path)
        .and_then(|mut f| f.read_to_string(&mut text))
        .map_err(|e| UpdaterError::Io(e.to_string()))?;
    Ok(text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compara_versiones_con_semver() {
        assert!(Version::parse("2.0.0").unwrap() > Version::parse("1.9.9").unwrap());
        assert_eq!(
            Version::parse("v2.0.0".trim_start_matches('v')).unwrap(),
            Version::new(2, 0, 0)
        );
    }

    #[test]
    fn hex_es_minuscula() {
        assert_eq!(hex_of(&[0, 15, 255]), "000fff");
    }

    #[test]
    fn latest_backup_elige_el_mas_reciente() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("parte-diario-20260101-000000.exe"),
            b"viejo",
        )
        .unwrap();
        fs::write(
            dir.path().join("parte-diario-20260202-000000.exe"),
            b"nuevo",
        )
        .unwrap();

        let latest = latest_backup(dir.path()).unwrap();
        assert_eq!(
            latest.file_name().unwrap().to_string_lossy(),
            "parte-diario-20260202-000000.exe"
        );
    }

    #[test]
    fn latest_backup_falla_sin_respaldos() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            latest_backup(dir.path()),
            Err(UpdaterError::InvalidPackage(_))
        ));
    }

    #[test]
    fn script_de_actualizacion_contiene_comandos() {
        let content = update_script_content(
            Path::new(r"C:\app\parte-diario.exe"),
            Path::new(r"C:\tmp\parte-diario.exe"),
            Path::new(r"C:\bak\parte-diario-20260101-000000.exe"),
        );
        assert!(content.contains("move /y"));
        assert!(content.contains("copy /y"));
        assert!(content.contains(r"C:\app\parte-diario.exe"));
        assert!(content.contains(r"C:\bak\parte-diario-20260101-000000.exe"));
        assert!(content.contains("start \"\""));
        assert!(content.contains("goto esperar"));
        assert!(content.contains("exit /b 1"));
    }

    #[test]
    fn script_de_rollback_contiene_comandos() {
        let content = rollback_script_content(
            Path::new(r"C:\app\parte-diario.exe"),
            Path::new(r"C:\bak\parte-diario-20260101-000000.exe"),
        );
        assert!(content.contains("copy /y"));
        assert!(content.contains(r"C:\bak\parte-diario-20260101-000000.exe"));
        assert!(content.contains(r"C:\app\parte-diario.exe"));
        assert!(content.contains("start \"\""));
        assert!(content.contains("goto esperar"));
    }

    #[test]
    fn lee_el_primer_token_del_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archivo.sha256");
        fs::write(&path, "abc123  parte-diario-windows-x64.zip\n").unwrap();
        assert_eq!(read_checksum(&path).unwrap(), "abc123");
    }
}
