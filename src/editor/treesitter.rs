use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tree_sitter::Language;
use libloading::Library;
use tree_sitter_highlight::HighlightConfiguration;

pub struct TreesitterLanguage {
    pub name: &'static str,
    pub repo: &'static str,
    pub file_types: &'static [&'static str],
}

pub const LANGUAGES: &[TreesitterLanguage] = &[
    TreesitterLanguage {
        name: "rust",
        repo: "https://github.com/tree-sitter/tree-sitter-rust",
        file_types: &["rs"],
    },
    TreesitterLanguage {
        name: "typescript",
        repo: "https://github.com/tree-sitter/tree-sitter-typescript",
        file_types: &["ts"],
    },
    TreesitterLanguage {
        name: "tsx",
        repo: "https://github.com/tree-sitter/tree-sitter-typescript",
        file_types: &["tsx"],
    },
    TreesitterLanguage {
        name: "javascript",
        repo: "https://github.com/tree-sitter/tree-sitter-javascript",
        file_types: &["js", "jsx"],
    },
    TreesitterLanguage {
        name: "python",
        repo: "https://github.com/tree-sitter/tree-sitter-python",
        file_types: &["py"],
    },
    TreesitterLanguage {
        name: "go",
        repo: "https://github.com/tree-sitter/tree-sitter-go",
        file_types: &["go"],
    },
    TreesitterLanguage {
        name: "c",
        repo: "https://github.com/tree-sitter/tree-sitter-c",
        file_types: &["c", "h"],
    },
    TreesitterLanguage {
        name: "cpp",
        repo: "https://github.com/tree-sitter/tree-sitter-cpp",
        file_types: &["cpp", "hpp", "cc", "hh"],
    },
    TreesitterLanguage {
        name: "lua",
        repo: "https://github.com/tree-sitter-grammars/tree-sitter-lua",
        file_types: &["lua"],
    },
    TreesitterLanguage {
        name: "json",
        repo: "https://github.com/tree-sitter/tree-sitter-json",
        file_types: &["json"],
    },
    TreesitterLanguage {
        name: "toml",
        repo: "https://github.com/ikatyang/tree-sitter-toml",
        file_types: &["toml"],
    },
    TreesitterLanguage {
        name: "html",
        repo: "https://github.com/tree-sitter/tree-sitter-html",
        file_types: &["html"],
    },
    TreesitterLanguage {
        name: "css",
        repo: "https://github.com/tree-sitter/tree-sitter-css",
        file_types: &["css"],
    },
    TreesitterLanguage {
        name: "svelte",
        repo: "https://github.com/tree-sitter-grammars/tree-sitter-svelte",
        file_types: &["svelte"],
    },
];

pub struct TreesitterManager {
    pub parser_dir: PathBuf,
    loaded_languages: std::collections::HashMap<String, (Language, Arc<Library>)>,
}

impl TreesitterManager {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let parser_dir = PathBuf::from(home).join(".local/share/atom/parsers");
        if !parser_dir.exists() {
            std::fs::create_dir_all(&parser_dir).unwrap_or_default();
        }
        Self {
            parser_dir,
            loaded_languages: std::collections::HashMap::new(),
        }
    }

    pub fn is_installed(&self, lang_name: &str) -> bool {
        let mut so_path = self.parser_dir.join(format!("{}.so", lang_name));
        if cfg!(target_os = "macos") {
            so_path.set_extension("dylib");
        } else if cfg!(target_os = "windows") {
            so_path.set_extension("dll");
        }
        so_path.exists()
    }

    pub fn install(&self, lang: &TreesitterLanguage) -> Result<(), String> {
        Self::install_to(lang, &self.parser_dir)
    }

    /// Blocking install that does NOT require holding the TreesitterManager lock.
    /// Call this from a background thread after extracting `parser_dir`.
    pub fn install_to(lang: &TreesitterLanguage, parser_dir: &Path) -> Result<(), String> {
        let repo_dir = parser_dir.join(format!("{}-repo", lang.name));

        if repo_dir.exists() {
            Command::new("git")
                .args(["-C", &repo_dir.to_string_lossy(), "pull"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|e| e.to_string())?;
        } else {
            Command::new("git")
                .args(["clone", "--depth=1", lang.repo, &repo_dir.to_string_lossy()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map_err(|e| e.to_string())?;
        }

        Self::compile_to(lang.name, &repo_dir, parser_dir)
    }

    pub fn uninstall_at(lang_name: &str, parser_dir: &Path) -> Result<(), String> {
        let ext = if cfg!(target_os = "macos") { "dylib" } else if cfg!(target_os = "windows") { "dll" } else { "so" };
        let so_path = parser_dir.join(format!("{}.{}", lang_name, ext));
        if so_path.exists() {
            std::fs::remove_file(&so_path).map_err(|e| e.to_string())?;
        }
        let repo_dir = parser_dir.join(format!("{}-repo", lang_name));
        if repo_dir.exists() {
            std::fs::remove_dir_all(&repo_dir).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn compile_to(name: &str, repo_dir: &Path, parser_dir: &Path) -> Result<(), String> {
        let src_dir = repo_dir.join("src");
        let actual_src_dir = if name == "typescript" {
            repo_dir.join("typescript").join("src")
        } else if name == "tsx" {
            repo_dir.join("tsx").join("src")
        } else {
            src_dir
        };

        let parser_c = actual_src_dir.join("parser.c");
        let scanner_c = actual_src_dir.join("scanner.c");
        let scanner_cc = actual_src_dir.join("scanner.cc");

        let ext = if cfg!(target_os = "macos") { "dylib" } else if cfg!(target_os = "windows") { "dll" } else { "so" };
        let output_path = parser_dir.join(format!("{}.{}", name, ext));

        let mut cmd = Command::new("cc");
        cmd.arg("-shared")
            .arg("-fPIC")
            .arg("-O3")
            .arg("-I").arg(&actual_src_dir)
            .arg("-o").arg(&output_path)
            .arg(&parser_c);

        if scanner_c.exists() { cmd.arg(&scanner_c); }
        if scanner_cc.exists() {
            cmd.arg(&scanner_cc).arg("-lstdc++");
        }

        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }
        Ok(())
    }

    pub fn uninstall(&self, lang_name: &str) -> Result<(), String> {
        Self::uninstall_at(lang_name, &self.parser_dir)
    }

    /// Returns extra highlight query patterns to append after the grammar's highlights.scm.
    /// In tree-sitter-highlight, the LAST pattern matching a node wins. Grammars sometimes
    /// place specific captures (e.g. @string.special.key) before the general one (@string),
    /// causing the general one to override. Appending the specific pattern again ensures it wins.
    fn highlight_overrides(lang_name: &str) -> &'static str {
        match lang_name {
            "json" => "\n(pair key: (_) @string.special.key)\n",
            _ => "",
        }
    }

    /// JavaScript base highlights prepended to TypeScript/TSX configs.
    ///
    /// The tree-sitter-typescript `queries/highlights.scm` only captures
    /// TypeScript-specific additions; it assumes JavaScript base highlights
    /// are inherited separately. Since we don't chain grammars, we inline the
    /// essential JS constructs here so keywords like `class`, `import`,
    /// `function`, etc. get colored.
    fn js_base_highlights() -> &'static str {
        r#"
[
  "as" "async" "await" "break" "case" "catch" "class" "const"
  "continue" "debugger" "default" "delete" "do" "else" "export"
  "extends" "finally" "for" "from" "function" "if" "import"
  "in" "instanceof" "let" "new" "of" "return" "static" "super"
  "switch" "target" "this" "throw" "try" "typeof" "var" "void"
  "while" "with" "yield"
] @keyword

(true)  @constant
(false) @constant
(null)  @constant

(comment) @comment

(string)          @string
(template_string) @string
(regex)           @string

(number) @number

(function_declaration  name: (identifier) @function)
(function_expression   name: (identifier) @function)
(method_definition     name: [(property_identifier)(private_property_identifier)] @function)
(call_expression
  function: [
    (identifier) @function
    (member_expression property: [(property_identifier)(private_property_identifier)] @function)
  ])
(new_expression constructor: (identifier) @type)

(identifier) @variable
(member_expression property: (property_identifier) @property)
(shorthand_property_identifier)         @property
(shorthand_property_identifier_pattern) @property
"#
    }

    pub fn get_language(&mut self, lang_name: &str) -> Option<Language> {
        if let Some((lang, _)) = self.loaded_languages.get(lang_name) {
            return Some(lang.clone());
        }

        let mut so_path = self.parser_dir.join(format!("{}.so", lang_name));
        if cfg!(target_os = "macos") {
            so_path.set_extension("dylib");
        } else if cfg!(target_os = "windows") {
            so_path.set_extension("dll");
        }

        if !so_path.exists() {
            return None;
        }

        unsafe {
            let lib = Arc::new(Library::new(so_path).ok()?);
            let symbol_name = format!("tree_sitter_{}", lang_name.replace("-", "_"));
            let constructor: libloading::Symbol<unsafe extern "C" fn() -> Language> =
                lib.get(symbol_name.as_bytes()).ok()?;
            let lang = constructor();
            self.loaded_languages.insert(lang_name.to_string(), (lang.clone(), lib));
            Some(lang)
        }
    }

    pub fn get_highlight_config(&mut self, lang_name: &str) -> Option<HighlightConfiguration> {
        let lang = self.get_language(lang_name)?;
        let repo_dir = self.parser_dir.join(format!("{}-repo", lang_name));
        
        // Find queries. For tree-sitter-typescript, both "typescript" and "tsx"
        // share a single queries/ dir at the repo root (not under typescript/queries
        // or tsx/queries, which only contain src/).
        let queries_dir = repo_dir.join("queries");

        let base_highlights = std::fs::read_to_string(queries_dir.join("highlights.scm")).unwrap_or_default();
        let injections_scm = std::fs::read_to_string(queries_dir.join("injections.scm")).unwrap_or_default();
        let locals_scm = std::fs::read_to_string(queries_dir.join("locals.scm")).unwrap_or_default();

        // For TypeScript/TSX, prepend the JS base highlights so that constructs
        // like `class`, `import`, `function` etc. get colored. The grammar's own
        // highlights.scm only has TS-specific additions; JS base is assumed to be
        // inherited from a separate grammar (which we inline here instead).
        let js_prefix = match lang_name {
            "typescript" | "tsx" => Self::js_base_highlights(),
            _ => "",
        };

        // Append language-specific overrides last (highest pattern index wins).
        let highlights_scm = format!("{}{}{}", js_prefix, base_highlights, Self::highlight_overrides(lang_name));

        let mut config = HighlightConfiguration::new(
            lang,
            lang_name,
            &highlights_scm,
            &injections_scm,
            &locals_scm,
        ).ok()?;

        // Define capture names that match our theme.
        // Order must match the captures array in highlighter.rs.
        // Tree-sitter picks the LONGEST prefix match, so more specific
        // names (e.g. "string.special.key") must come before "string".
        let captures = [
            "keyword",            // 0  → Keyword
            "function",           // 1  → Function
            "type",               // 2  → Type
            "string.special.key", // 3  → Property  (JSON keys: @string.special.key)
            "string",             // 4  → String    (all other strings)
            "comment",            // 5  → Comment
            "constant",           // 6  → Constant  (matches constant.builtin, etc.)
            "variable",           // 7  → Variable
            "parameter",          // 8  → Identifier
            "label",              // 9  → Keyword
            "tag",                // 10 → Tag
            "attribute",          // 11 → Attribute
            "number",             // 12 → Constant
            "operator",           // 13 → Keyword
            "property",           // 14 → Property  (@property in other grammars)
            "namespace",          // 15 → Type
            "punctuation",        // 16 → Normal
        ];
        config.configure(&captures);
        
        Some(config)
    }
}
