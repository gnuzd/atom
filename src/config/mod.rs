use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub disable_autoformat: bool,
    #[serde(default = "default_colorscheme")]
    pub colorscheme: String,
    #[serde(default = "default_wrap")]
    pub wrap: bool,
}

fn default_colorscheme() -> String {
    "gruvbox-material".to_string()
}

fn default_wrap() -> bool {
    true
}

impl Config {
    pub fn default() -> Self {
        Self {
            disable_autoformat: false,
            colorscheme: default_colorscheme(),
            wrap: true,
        }
    }

    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let mut path = PathBuf::from(home);
        path.push(".config");
        path.push("atom");
        path.push("config.json");
        path
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(config) = serde_json::from_str(&content) {
                return config;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}
