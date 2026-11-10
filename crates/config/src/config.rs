use std::fs;

use parte_diario_ai::LlmProvider;
use parte_diario_core::paths;
use serde::{Deserialize, Serialize};

use crate::secrets;
use crate::write_atomic;

fn default_provider() -> String {
    LlmProvider::OpenRouter.as_str().to_string()
}

fn default_anonymize() -> bool {
    true
}

fn default_true() -> bool {
    true
}

/// Extrae `usuario/repositorio` de una URL de GitHub.
///
/// Devuelve una cadena vacía si la URL no es de GitHub o si todavía es el
/// marcador `TU-USUARIO` del proyecto.
fn parse_github_repository(url: &str) -> String {
    let url = url.trim();
    let path = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git@github.com:"))
        .unwrap_or("");
    let path = path.trim_end_matches(".git").trim_matches('/');
    if path.is_empty() || path.starts_with("TU-USUARIO/") {
        String::new()
    } else {
        path.to_string()
    }
}

/// Repositorio de actualizaciones por defecto.
///
/// Prioriza `PARTE_DIARIO_UPDATE_REPOSITORY` (definido por el workflow de
/// release con `github.repository`); si no existe, lo deriva del `repository`
/// del workspace. Al publicar con la URL real, la app queda configurada sola.
fn default_update_repository() -> String {
    if let Some(repo) = option_env!("PARTE_DIARIO_UPDATE_REPOSITORY") {
        let repo = repo.trim();
        if !repo.is_empty() {
            return repo.to_string();
        }
    }
    parse_github_repository(env!("CARGO_PKG_REPOSITORY"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(skip)]
    pub api_key: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub output_dir: String,
    #[serde(default)]
    pub last_template: String,
    #[serde(default = "default_anonymize")]
    pub anonymize: bool,
    #[serde(default = "default_update_repository")]
    pub update_repository: String,
    #[serde(default = "default_true")]
    pub check_updates_on_startup: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            provider: default_provider(),
            model: LlmProvider::OpenRouter.default_model().to_string(),
            output_dir: String::new(),
            last_template: String::new(),
            anonymize: true,
            update_repository: default_update_repository(),
            check_updates_on_startup: true,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct LegacyConfig {
    #[serde(default)]
    api_key: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    output_dir: String,
    #[serde(default)]
    last_template: String,
}

pub fn load_config() -> Config {
    let mut cfg = fs::read_to_string(paths::config_file())
        .ok()
        .and_then(|s| toml::from_str::<Config>(&s).ok())
        .unwrap_or_default();

    if let Some(legacy) = load_legacy() {
        cfg.provider = if legacy.provider.is_empty() {
            default_provider()
        } else {
            legacy.provider
        };
        cfg.model = if legacy.model.is_empty() {
            LlmProvider::OpenRouter.default_model().to_string()
        } else {
            legacy.model
        };
        cfg.output_dir = legacy.output_dir;
        cfg.last_template = legacy.last_template;
        if !legacy.api_key.is_empty() {
            cfg.api_key = legacy.api_key;
        }
        if save_config(&cfg).is_ok() {
            let _ = fs::remove_file(paths::legacy_config_file());
        }
    }

    if cfg.api_key.is_empty() {
        cfg.api_key = secrets::load_api_key().unwrap_or_default();
    }

    // Las versiones publicadas por el workflow embeben el repositorio; si una
    // configuración antigua lo tenía vacío, se adopta el valor embebido.
    if cfg.update_repository.is_empty() {
        let default_repo = default_update_repository();
        if !default_repo.is_empty() {
            cfg.update_repository = default_repo;
        }
    }

    migrate_config(cfg)
}

fn load_legacy() -> Option<LegacyConfig> {
    fs::read_to_string(paths::legacy_config_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

fn migrate_config(mut cfg: Config) -> Config {
    if cfg.provider == LlmProvider::OpenRouter.as_str()
        && cfg.model == "nvidia/nemotron-3-ultra-550b-a55b:free"
    {
        cfg.model = LlmProvider::OpenRouter.default_model().to_string();
    }
    cfg
}

pub fn save_config(cfg: &Config) -> Result<(), String> {
    secrets::save_api_key(&cfg.api_key)?;
    let text = toml::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    write_atomic(&paths::config_file(), &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_provider_matches_model() {
        let cfg = Config::default();
        assert_eq!(cfg.provider, LlmProvider::OpenRouter.as_str());
        assert_eq!(cfg.model, LlmProvider::OpenRouter.default_model());
        assert!(cfg.check_updates_on_startup);
    }

    #[test]
    fn parsea_repositorio_de_github() {
        assert_eq!(
            parse_github_repository("https://github.com/acme/parte-diario"),
            "acme/parte-diario"
        );
        assert_eq!(
            parse_github_repository("https://github.com/acme/parte-diario.git"),
            "acme/parte-diario"
        );
        assert_eq!(
            parse_github_repository("git@github.com:acme/parte-diario.git"),
            "acme/parte-diario"
        );
        assert_eq!(
            parse_github_repository("https://github.com/TU-USUARIO/parte-diario"),
            ""
        );
        assert_eq!(parse_github_repository(""), "");
        assert_eq!(
            parse_github_repository("https://gitlab.com/acme/parte-diario"),
            ""
        );
    }

    #[test]
    fn embebe_repositorio_de_actualizaciones_en_compilacion() {
        if let Some(expected) = option_env!("PARTE_DIARIO_UPDATE_REPOSITORY") {
            let expected = expected.trim();
            if !expected.is_empty() {
                assert_eq!(default_update_repository(), expected);
            }
        }
        assert!(!default_update_repository().starts_with("TU-USUARIO/"));
    }

    #[test]
    fn migra_el_modelo_viejo_de_razonamiento() {
        let cfg = Config {
            provider: LlmProvider::OpenRouter.as_str().to_string(),
            model: "nvidia/nemotron-3-ultra-550b-a55b:free".to_string(),
            ..Config::default()
        };
        let migrated = migrate_config(cfg);
        assert_eq!(migrated.model, LlmProvider::OpenRouter.default_model());
        assert_ne!(migrated.model, "nvidia/nemotron-3-ultra-550b-a55b:free");
    }
}
