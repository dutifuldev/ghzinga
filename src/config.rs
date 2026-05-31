use std::{
    env, fs, io,
    path::{Path, PathBuf},
    str::FromStr,
};

use serde::Deserialize;

use crate::render::{SymbolMode, ThemeName};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AppConfig {
    pub ui: UiConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiConfig {
    pub theme: ThemeName,
    pub symbols: SymbolMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedConfig {
    pub config: AppConfig,
    pub path: PathBuf,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    ui: Option<RawUiConfig>,
}

#[derive(Debug, Deserialize)]
struct RawUiConfig {
    theme: Option<String>,
    symbols: Option<String>,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: ThemeName::Default,
            symbols: SymbolMode::Ascii,
        }
    }
}

impl AppConfig {
    pub fn with_theme(mut self, theme: ThemeName) -> Self {
        self.ui.theme = theme;
        self
    }

    pub fn with_symbols(mut self, symbols: SymbolMode) -> Self {
        self.ui.symbols = symbols;
        self
    }

    pub fn to_toml(self) -> String {
        format!(
            "[ui]\ntheme = \"{}\"\nsymbols = \"{}\"\n",
            self.ui.theme, self.ui.symbols
        )
    }
}

pub fn load() -> LoadedConfig {
    let path = config_path();
    load_from_path(path)
}

pub fn load_from_path(path: PathBuf) -> LoadedConfig {
    let mut diagnostics = Vec::new();
    let config = match fs::read_to_string(&path) {
        Ok(contents) => parse_config(&contents, &mut diagnostics),
        Err(error) if error.kind() == io::ErrorKind::NotFound => AppConfig::default(),
        Err(error) => {
            diagnostics.push(format!("failed to read config {}: {error}", path.display()));
            AppConfig::default()
        }
    };

    LoadedConfig {
        config,
        path,
        diagnostics,
    }
}

pub fn save_to_path(path: &Path, config: AppConfig) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, config.to_toml())
}

pub fn config_path() -> PathBuf {
    config_path_from_env(
        env::var_os("GZG_CONFIG_PATH"),
        env::var_os("XDG_CONFIG_HOME"),
        env::var_os("HOME"),
    )
}

pub fn config_path_from_env(
    override_path: Option<impl Into<PathBuf>>,
    xdg_config_home: Option<impl Into<PathBuf>>,
    home: Option<impl Into<PathBuf>>,
) -> PathBuf {
    if let Some(path) = override_path {
        return path.into();
    }

    if let Some(path) = xdg_config_home {
        return path.into().join("ghzinga").join("config.toml");
    }

    if let Some(home) = home {
        return home
            .into()
            .join(".config")
            .join("ghzinga")
            .join("config.toml");
    }

    PathBuf::from(".").join("ghzinga").join("config.toml")
}

fn parse_config(contents: &str, diagnostics: &mut Vec<String>) -> AppConfig {
    let raw = match toml::from_str::<RawConfig>(contents) {
        Ok(raw) => raw,
        Err(error) => {
            diagnostics.push(format!("failed to parse config.toml: {error}"));
            return AppConfig::default();
        }
    };

    let mut config = AppConfig::default();
    if let Some(ui) = raw.ui {
        if let Some(theme) = ui.theme {
            match ThemeName::from_str(&theme) {
                Ok(theme) => config.ui.theme = theme,
                Err(error) => diagnostics.push(format!("invalid ui.theme `{theme}`: {error}")),
            }
        }
        if let Some(symbols) = ui.symbols {
            match SymbolMode::from_str(&symbols) {
                Ok(symbols) => config.ui.symbols = symbols,
                Err(error) => {
                    diagnostics.push(format!("invalid ui.symbols `{symbols}`: {error}"));
                }
            }
        }
    }
    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_uses_defaults() {
        let path = std::env::temp_dir().join("ghzinga-missing-config-for-test.toml");
        let _ = fs::remove_file(&path);

        let loaded = load_from_path(path);

        assert_eq!(loaded.config, AppConfig::default());
        assert!(loaded.diagnostics.is_empty());
    }

    #[test]
    fn parses_ui_config_values() {
        let mut diagnostics = Vec::new();

        let config = parse_config(
            "[ui]\ntheme = \"solarized-dark\"\nsymbols = \"emoji\"\n",
            &mut diagnostics,
        );

        assert_eq!(config.ui.theme, ThemeName::SolarizedDark);
        assert_eq!(config.ui.symbols, SymbolMode::Emoji);
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn invalid_known_values_fall_back_with_diagnostics() {
        let mut diagnostics = Vec::new();

        let config = parse_config(
            "[ui]\ntheme = \"loud\"\nsymbols = \"icons\"\n",
            &mut diagnostics,
        );

        assert_eq!(config, AppConfig::default());
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics[0].contains("invalid ui.theme"));
        assert!(diagnostics[1].contains("invalid ui.symbols"));
    }

    #[test]
    fn config_path_prefers_override_then_xdg_then_home() {
        assert_eq!(
            config_path_from_env(Some("/tmp/gzg.toml"), Some("/tmp/xdg"), Some("/home/alice")),
            PathBuf::from("/tmp/gzg.toml")
        );
        assert_eq!(
            config_path_from_env(None::<PathBuf>, Some("/tmp/xdg"), Some("/home/alice")),
            PathBuf::from("/tmp/xdg/ghzinga/config.toml")
        );
        assert_eq!(
            config_path_from_env(None::<PathBuf>, None::<PathBuf>, Some("/home/alice")),
            PathBuf::from("/home/alice/.config/ghzinga/config.toml")
        );
    }

    #[test]
    fn save_writes_small_toml_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/config.toml");
        let config = AppConfig::default()
            .with_theme(ThemeName::SolarizedDark)
            .with_symbols(SymbolMode::Emoji);

        save_to_path(&path, config).unwrap();

        let contents = fs::read_to_string(path).unwrap();
        assert_eq!(
            contents,
            "[ui]\ntheme = \"solarized-dark\"\nsymbols = \"emoji\"\n"
        );
    }
}
