use std::fs;

use parte_diario_core::paths;

use crate::write_atomic;

#[cfg(windows)]
const SERVICE: &str = "parte-diario";
#[cfg(windows)]
const USER: &str = "llm-api-key";

#[cfg(windows)]
fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, USER).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn keyring_disabled() -> bool {
    std::env::var("PARTE_DIARIO_DISABLE_KEYRING")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub fn load_api_key() -> Result<String, String> {
    #[cfg(windows)]
    if !keyring_disabled() {
        if let Ok(entry) = keyring_entry() {
            if let Ok(value) = entry.get_password() {
                if !value.is_empty() {
                    return Ok(value);
                }
            }
        }
    }
    load_from_file()
}

pub fn save_api_key(value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return delete_api_key();
    }

    #[cfg(windows)]
    if !keyring_disabled() {
        if let Ok(entry) = keyring_entry() {
            if entry.set_password(value).is_ok() {
                let _ = fs::remove_file(paths::secrets_file());
                return Ok(());
            }
        }
    }

    let text = format!("api_key = {}\n", toml_string(value));
    write_atomic(&paths::secrets_file(), &text)
}

pub fn delete_api_key() -> Result<(), String> {
    #[cfg(windows)]
    if !keyring_disabled() {
        if let Ok(entry) = keyring_entry() {
            let _ = entry.delete_credential();
        }
    }
    match fs::remove_file(paths::secrets_file()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

fn load_from_file() -> Result<String, String> {
    let path = paths::secrets_file();
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let value: toml::Value = toml::from_str(&text).map_err(|e| e.to_string())?;
    Ok(value
        .get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string())
}

fn toml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_string_escapes_quotes_and_backslashes() {
        assert_eq!(toml_string(r#"a"b\c"#), r#""a\"b\\c""#);
    }
}
