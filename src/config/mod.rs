use crate::core::models::Scope;
use crate::error::{Error, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Unknown keys are ignored (forward compatible); wrong types/values are errors.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub general: General,
    pub search: Search,
    pub cache: Cache,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct General {
    pub default_scope: Scope,
    pub default_remote: String,
    pub confirm_install: bool,
    pub confirm_remove: bool,
    pub color: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct Search {
    pub include_descriptions: bool,
    pub fuzzy_matching: bool,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(default)]
pub struct Cache {
    pub enabled: bool,
    pub max_age_hours: u64,
}

impl Default for General {
    fn default() -> Self {
        General {
            default_scope: Scope::User,
            default_remote: String::new(),
            confirm_install: true,
            confirm_remove: true,
            color: true,
        }
    }
}
impl Default for Search {
    fn default() -> Self {
        Search {
            include_descriptions: true,
            fuzzy_matching: true,
        }
    }
}
impl Default for Cache {
    fn default() -> Self {
        Cache {
            enabled: true,
            max_age_hours: 24,
        }
    }
}

impl Config {
    pub fn parse(text: &str, origin: &str) -> Result<Config> {
        let cfg: Config = toml::from_str(text).map_err(|e| {
            Error::Config(format!(
                "Invalid configuration in {origin}:\n{e}\nFix the file, or delete it to use the defaults."
            ))
        })?;
        if !cfg.general.default_remote.is_empty()
            && !crate::core::models::is_valid_remote_name(&cfg.general.default_remote)
        {
            return Err(Error::Config(format!(
                "Invalid configuration in {origin}:\ngeneral.default_remote '{}' is not a valid remote name.",
                cfg.general.default_remote
            )));
        }
        Ok(cfg)
    }

    /// A missing file is fine (defaults); an unreadable or invalid one is an error.
    pub fn load(path: Option<&Path>) -> Result<Config> {
        let Some(path) = path else {
            return Ok(Config::default());
        };
        match std::fs::read_to_string(path) {
            Ok(text) => Config::parse(&text, &path.display().to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(e) => Err(Error::Config(format!(
                "Cannot read {}: {e}",
                path.display()
            ))),
        }
    }
}

fn xdg(var: &str, fallback: &str) -> Option<PathBuf> {
    match std::env::var_os(var) {
        Some(v) if !v.is_empty() && Path::new(&v).is_absolute() => Some(PathBuf::from(v)),
        _ => std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join(fallback)),
    }
}

pub fn config_path() -> Option<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config").map(|d| d.join("ifhf").join("config.toml"))
}

pub fn cache_dir() -> Option<PathBuf> {
    xdg("XDG_CACHE_HOME", ".cache").map(|d| d.join("ifhf"))
}

/// Flatpak's user installation directory (honours FLATPAK_USER_DIR like Flatpak does).
pub fn flatpak_user_dir() -> Option<PathBuf> {
    match std::env::var_os("FLATPAK_USER_DIR") {
        Some(v) if !v.is_empty() => Some(PathBuf::from(v)),
        _ => xdg("XDG_DATA_HOME", ".local/share").map(|d| d.join("flatpak")),
    }
}

pub fn flatpak_system_dir() -> PathBuf {
    match std::env::var_os("FLATPAK_SYSTEM_DIR") {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => PathBuf::from("/var/lib/flatpak"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = Config::default();
        assert_eq!(c.general.default_scope, Scope::User);
        assert!(c.general.confirm_install && c.general.confirm_remove && c.general.color);
        assert!(c.search.include_descriptions && c.search.fuzzy_matching);
        assert!(c.cache.enabled);
        assert_eq!(c.cache.max_age_hours, 24);
    }

    #[test]
    fn empty_and_partial_files_use_defaults() {
        assert_eq!(Config::parse("", "t").unwrap(), Config::default());
        let c = Config::parse("[general]\ndefault_scope = \"system\"\n", "t").unwrap();
        assert_eq!(c.general.default_scope, Scope::System);
        assert!(c.general.confirm_install);
    }

    #[test]
    fn unknown_keys_ignored() {
        let c = Config::parse("[general]\nfuture_option = 3\n[other]\nx=1\n", "t").unwrap();
        assert_eq!(c, Config::default());
    }

    #[test]
    fn invalid_values_are_actionable() {
        let e = Config::parse("[general]\ndefault_scope = \"galaxy\"\n", "cfg.toml").unwrap_err();
        let m = e.to_string();
        assert!(
            m.contains("cfg.toml") && m.contains("default_scope") && m.contains("delete it"),
            "{m}"
        );
        assert!(Config::parse("[cache]\nmax_age_hours = \"soon\"\n", "t").is_err());
        assert!(Config::parse("[general]\ndefault_remote = \"-x\"\n", "t").is_err());
        assert!(Config::parse("not toml [", "t").is_err());
    }

    #[test]
    fn missing_file_is_fine() {
        let p = Path::new("/nonexistent/ifhf/config.toml");
        assert_eq!(Config::load(Some(p)).unwrap(), Config::default());
        assert_eq!(Config::load(None).unwrap(), Config::default());
    }
}
