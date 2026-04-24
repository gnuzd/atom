use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use mlua::prelude::*;

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
fn default_tabstop() -> usize { 2 }
fn default_laststatus() -> usize { 3 }

/// A user-defined keymap entry from init.lua
#[derive(Debug, Clone)]
pub struct UserKeymap {
    pub mode: String,   // "n", "i", "v", "x"
    pub key: String,    // e.g. "<C-s>", "jk", "<leader>ff"
    pub action: String, // action name or ":command"
}

/// A user-defined code snippet from init.lua
#[derive(Debug, Clone)]
pub struct UserSnippet {
    pub filetype: String,  // e.g. "svelte", "rs", "ts"
    pub trigger: String,   // prefix that triggers the snippet
    pub name: String,      // display name shown in menu
    pub body: String,      // snippet body with $1, ${1:default}, $0 syntax
}

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

    pub fn init_lua_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("atom").join("init.lua")
    }

    pub fn config_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("atom").join("config.json")
    }

    /// Load config + user keymaps + user snippets. Tries init.lua first, falls back to config.json.
    pub fn load_with_keymaps() -> (Self, Vec<UserKeymap>, Vec<UserSnippet>) {
        let lua_path = Self::init_lua_path();
        if let Ok(content) = fs::read_to_string(&lua_path) {
            match Self::from_lua(&content) {
                Ok(result) => return result,
                Err(e) => eprintln!("atom: init.lua error: {}", e),
            }
        }
        (Self::load(), vec![], vec![])
    }

    fn from_lua(content: &str) -> LuaResult<(Self, Vec<UserKeymap>, Vec<UserSnippet>)> {
        let lua = Lua::new();
        let mut config = Self::default();

        // vim.opt — plain table, read back after exec
        let opt_table = lua.create_table()?;

        // vim.keymap.set — accumulates into _atom_keymaps global
        lua.globals().set("_atom_keymaps", lua.create_table()?)?;

        let keymap_set = lua.create_function(|lua, (mode, key, action): (String, String, String)| {
            let t: LuaTable = lua.globals().get("_atom_keymaps")?;
            let len = t.raw_len();
            let entry = lua.create_table()?;
            entry.set(1, mode)?;
            entry.set(2, key)?;
            entry.set(3, action)?;
            t.set(len + 1, entry)?;
            Ok(())
        })?;

        let keymap_table = lua.create_table()?;
        keymap_table.set("set", keymap_set)?;

        // vim.snippet.add(filetype, trigger, name, body)
        lua.globals().set("_atom_snippets", lua.create_table()?)?;
        let snippet_add = lua.create_function(|lua, (ft, trigger, name, body): (String, String, String, String)| {
            let t: LuaTable = lua.globals().get("_atom_snippets")?;
            let len = t.raw_len();
            let entry = lua.create_table()?;
            entry.set(1, ft)?;
            entry.set(2, trigger)?;
            entry.set(3, name)?;
            entry.set(4, body)?;
            t.set(len + 1, entry)?;
            Ok(())
        })?;
        let snippet_table = lua.create_table()?;
        snippet_table.set("add", snippet_add)?;

        let vim_table = lua.create_table()?;
        vim_table.set("opt", opt_table)?;
        vim_table.set("keymap", keymap_table)?;
        vim_table.set("snippet", snippet_table)?;
        lua.globals().set("vim", vim_table)?;

        // Execute init.lua
        lua.load(content).set_name("init.lua").exec()?;

        // Read vim.opt back
        let vim: LuaTable = lua.globals().get("vim")?;
        let opt: LuaTable = vim.get("opt")?;
        for pair in opt.pairs::<String, LuaValue>() {
            let (k, v) = pair?;
            match k.as_str() {
                "colorscheme" => {
                    if let LuaValue::String(s) = v { config.colorscheme = s.to_str()?.to_string(); }
                }
                "number" => { if let LuaValue::Boolean(b) = v { config.number = b; } }
                "relativenumber" | "rnu" => { if let LuaValue::Boolean(b) = v { config.relativenumber = b; } }
                "wrap" => { if let LuaValue::Boolean(b) = v { config.wrap = b; } }
                "tabstop" | "ts" => {
                    if let LuaValue::Integer(n) = v { config.tabstop = n as usize; }
                }
                "shiftwidth" | "sw" => {
                    if let LuaValue::Integer(n) = v { config.shiftwidth = n as usize; }
                }
                "expandtab" | "et" => { if let LuaValue::Boolean(b) = v { config.expandtab = b; } }
                "cursorline" | "cul" => { if let LuaValue::Boolean(b) = v { config.cursorline = b; } }
                "ignorecase" | "ic" => { if let LuaValue::Boolean(b) = v { config.ignorecase = b; } }
                "smartcase" | "scs" => { if let LuaValue::Boolean(b) = v { config.smartcase = b; } }
                "undofile" => { if let LuaValue::Boolean(b) = v { config.undofile = b; } }
                "signcolumn" | "scl" => {
                    match &v {
                        LuaValue::Boolean(b) => config.signcolumn = *b,
                        LuaValue::String(s) => config.signcolumn = s.to_str()? != "no",
                        _ => {}
                    }
                }
                "mouse" => {
                    match &v {
                        LuaValue::Boolean(b) => config.mouse = *b,
                        LuaValue::String(s) => config.mouse = !s.to_str()?.is_empty(),
                        _ => {}
                    }
                }
                "laststatus" | "ls" => {
                    if let LuaValue::Integer(n) = v { config.laststatus = n as usize; }
                }
                "autoformat" => { if let LuaValue::Boolean(b) = v { config.disable_autoformat = !b; } }
                _ => {}
            }
        }

        // Read keymaps
        let km_list: LuaTable = lua.globals().get("_atom_keymaps")?;
        let mut user_keymaps = Vec::new();
        for i in 1..=km_list.raw_len() {
            let entry: LuaTable = km_list.get(i)?;
            let mode: String = entry.get(1)?;
            let key: String = entry.get(2)?;
            let action: String = entry.get(3)?;
            user_keymaps.push(UserKeymap { mode, key, action });
        }

        // Read snippets
        let sn_list: LuaTable = lua.globals().get("_atom_snippets")?;
        let mut user_snippets = Vec::new();
        for i in 1..=sn_list.raw_len() {
            let entry: LuaTable = sn_list.get(i)?;
            let filetype: String = entry.get(1)?;
            let trigger: String = entry.get(2)?;
            let name: String = entry.get(3)?;
            let body: String = entry.get(4)?;
            user_snippets.push(UserSnippet { filetype, trigger, name, body });
        }

        Ok((config, user_keymaps, user_snippets))
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

    /// Write a starter init.lua if none exists.
    pub fn write_default_lua() -> std::io::Result<()> {
        let path = Self::init_lua_path();
        if path.exists() { return Ok(()); }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, DEFAULT_INIT_LUA)?;
        Ok(())
    }
}

const DEFAULT_INIT_LUA: &str = r#"-- ~/.config/atom/init.lua
-- Atom editor configuration

-- Options
vim.opt.colorscheme = "gruvbox-material"
vim.opt.number = true
vim.opt.relativenumber = true
vim.opt.cursorline = true
vim.opt.wrap = true
vim.opt.tabstop = 2
vim.opt.shiftwidth = 2
vim.opt.expandtab = true
vim.opt.ignorecase = true
vim.opt.smartcase = true
vim.opt.signcolumn = true
vim.opt.mouse = true

-- Keymaps: vim.keymap.set(mode, lhs, rhs)
--   mode:   "n" normal, "i" insert, "v" visual
--   lhs:    key combo e.g. "<C-s>", "jk", "<leader>ff"
--   rhs:    action name OR ":ex-command"
--
-- Examples:
-- vim.keymap.set("i", "jk", "ExitMode")
-- vim.keymap.set("n", "<leader>ff", ":TelescopeFiles")
-- vim.keymap.set("n", "<C-h>", ":split")
-- vim.keymap.set("n", "<C-w>", "CloseBuffer")

-- Snippets: vim.snippet.add(filetype, trigger, display_name, body)
--   filetype: file extension without dot, e.g. "svelte", "rs", "ts"
--   trigger:  prefix typed to show the snippet
--   body:     snippet text with $1/$2 tabstops, ${1:default}, $0 = final cursor
--
-- Examples:
-- vim.snippet.add("svelte", "scri", "Script block",  "<script>\n\t$1\n</script>")
-- vim.snippet.add("svelte", "styl", "Style block",   "<style>\n\t$1\n</style>")
-- vim.snippet.add("svelte", "each", "Each block",    "{#each ${1:items} as ${2:item}}\n\t$3\n{/each}")
-- vim.snippet.add("ts",     "fn",   "Arrow function","const ${1:name} = (${2:args}) => {\n\t$3\n}")
-- vim.snippet.add("rs",     "fn",   "Function",      "fn ${1:name}(${2}) -> ${3:()} {\n\t$4\n}")
"#;
