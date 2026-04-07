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
        let repo_dir = self.parser_dir.join(format!("{}-repo", lang.name));
        
        // 1. Clone or Pull
        if repo_dir.exists() {
            Command::new("git")
                .arg("-C")
                .arg(&repo_dir)
                .arg("pull")
                .output()
                .map_err(|e| e.to_string())?;
        } else {
            Command::new("git")
                .arg("clone")
                .arg("--depth=1")
                .arg(lang.repo)
                .arg(&repo_dir)
                .output()
                .map_err(|e| e.to_string())?;
        }

        // 2. Compile
        self.compile(lang.name, &repo_dir)?;

        // 3. Ensure queries are available
        // In many repos, highlights.scm is in queries/
        // We'll just leave the repo there for now so we can read queries from it.
        
        Ok(())
    }

    fn compile(&self, name: &str, repo_dir: &Path) -> Result<(), String> {
        let src_dir = repo_dir.join("src");
        // For some languages (like typescript), the parser is in a subdirectory
        let (actual_src_dir, _) = if name == "typescript" {
            (repo_dir.join("typescript").join("src"), "typescript")
        } else if name == "tsx" {
            (repo_dir.join("tsx").join("src"), "tsx")
        } else {
            (src_dir, name)
        };

        let parser_c = actual_src_dir.join("parser.c");
        let scanner_c = actual_src_dir.join("scanner.c");
        let scanner_cc = actual_src_dir.join("scanner.cc");

        let mut output_path = self.parser_dir.join(format!("{}.so", name));
        if cfg!(target_os = "macos") {
            output_path.set_extension("dylib");
        } else if cfg!(target_os = "windows") {
            output_path.set_extension("dll");
        }

        let mut cmd = Command::new("cc");
        cmd.arg("-shared")
            .arg("-fPIC")
            .arg("-O3")
            .arg("-I")
            .arg(&actual_src_dir)
            .arg("-o")
            .arg(&output_path)
            .arg(&parser_c);

        if scanner_c.exists() {
            cmd.arg(&scanner_c);
        }
        if scanner_cc.exists() {
            cmd.arg(&scanner_cc);
            cmd.arg("-lstdc++");
        }

        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).to_string());
        }

        Ok(())
    }

    pub fn uninstall(&self, lang_name: &str) -> Result<(), String> {
        let mut so_path = self.parser_dir.join(format!("{}.so", lang_name));
        if cfg!(target_os = "macos") {
            so_path.set_extension("dylib");
        } else if cfg!(target_os = "windows") {
            so_path.set_extension("dll");
        }
        if so_path.exists() {
            std::fs::remove_file(so_path).map_err(|e| e.to_string())?;
        }
        let repo_dir = self.parser_dir.join(format!("{}-repo", lang_name));
        if repo_dir.exists() {
            std::fs::remove_dir_all(repo_dir).map_err(|e| e.to_string())?;
        }
        Ok(())
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
        
        // Find queries
        let queries_dir = if lang_name == "typescript" {
            repo_dir.join("typescript").join("queries")
        } else if lang_name == "tsx" {
            repo_dir.join("tsx").join("queries")
        } else {
            repo_dir.join("queries")
        };

        let highlights_scm = std::fs::read_to_string(queries_dir.join("highlights.scm")).unwrap_or_default();
        let injections_scm = std::fs::read_to_string(queries_dir.join("injections.scm")).unwrap_or_default();
        let locals_scm = std::fs::read_to_string(queries_dir.join("locals.scm")).unwrap_or_default();

        let mut config = HighlightConfiguration::new(
            lang,
            &highlights_scm,
            &injections_scm,
            &locals_scm,
            "",
        ).ok()?;

        // Define capture names that match our theme
        let captures = [
            "keyword", "function", "type", "string", "comment", "constant", "variable", "parameter", "label", "tag", "attribute"
        ];
        config.configure(&captures);
        
        Some(config)
    }
}
