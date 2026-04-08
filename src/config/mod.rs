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
    #[serde(default = "default_true")]
    pub number: bool,
    #[serde(default = "default_true")]
    pub relativenumber: bool,
    #[serde(default = "default_true")]
    pub cursorline: bool,
    #[serde(default = "default_tabstop")]
    pub tabstop: usize,
    #[serde(default = "default_tabstop")]
    pub shiftwidth: usize,
    #[serde(default = "default_true")]
    pub expandtab: bool,
    #[serde(default = "default_true")]
    pub ignorecase: bool,
    #[serde(default = "default_true")]
    pub smartcase: bool,
    #[serde(default = "default_true")]
    pub undofile: bool,
    #[serde(default = "default_true")]
    pub signcolumn: bool,
    #[serde(default = "default_true")]
    pub mouse: bool,
    #[serde(default = "default_true")]
    pub showmode: bool,
    #[serde(default = "default_laststatus")]
    pub laststatus: usize,
}

fn default_colorscheme() -> String { "gruvbox-material".to_string() }
fn default_wrap() -> bool { true }
fn default_true() -> bool { true }
fn default_false() -> bool { false }
fn default_tabstop() -> usize { 2 }
fn default_laststatus() -> usize { 3 }

impl Config {
    pub fn default() -> Self {
        Self {
            disable_autoformat: false,
            colorscheme: default_colorscheme(),
            wrap: true,
            number: true,
            relativenumber: true,
            cursorline: true,
            tabstop: 2,
            shiftwidth: 2,
            expandtab: true,
            ignorecase: true,
            smartcase: true,
            undofile: true,
            signcolumn: true,
            mouse: true,
            showmode: true,
            laststatus: 3,
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
