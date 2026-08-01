//! Project configuration file (`tokenpress.toml`).
//!
//! This module only turns the file into a typed value. Locating the file and
//! merging it with the command-line arguments happens elsewhere.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Everything a `tokenpress.toml` can express. Every field is optional: a
/// missing key means "not configured", never a default. Unknown keys are
/// rejected rather than ignored, so a typo fails loudly instead of silently
/// doing nothing.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FileConfig {
    /// Free-form tokenizer spec; validated where the CLI validates `--tokenizer`.
    pub tokenizer: Option<String>,
    pub verify: Option<ConfigVerify>,
    pub python: Option<PythonConfig>,
    pub rust: Option<RustConfig>,
}

/// `[python]` table.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PythonConfig {
    pub strip_comments: Option<bool>,
    pub strip_docstrings: Option<bool>,
    pub strip_annotations: Option<bool>,
    /// Positively named: `merge_imports = false` is the config spelling of the
    /// command line's `--py-no-merge-imports`.
    pub merge_imports: Option<bool>,
}

/// `[rust]` table.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RustConfig {
    pub strip_doc_comments: Option<bool>,
}

/// Verification level as spelled in the config file. The variants carry the
/// same lowercase names as the `--verify` values accepted on the command line.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConfigVerify {
    Reparse,
    Ast,
    External,
}

/// Errors raised while loading a config file.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("cannot read config file {}: {source}", path.display())]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The TOML error already carries the offending key and its line/column.
    #[error("invalid config file: {0}")]
    Parse(#[from] toml::de::Error),
}

impl FileConfig {
    /// Parses the contents of a `tokenpress.toml`.
    pub fn from_toml_str(text: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(text)?)
    }

    /// Reads `path` and parses it. An unreadable file is a `Read` error that
    /// names the path; a malformed one is a `Parse` error.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml_str(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Unique scratch directory per test, cleaned up on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static N: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "tokenpress-config-test-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn file(&self, name: &str, content: &str) -> PathBuf {
            let p = self.0.join(name);
            std::fs::write(&p, content).unwrap();
            p
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Parses `text`, expecting success.
    fn parse(text: &str) -> FileConfig {
        FileConfig::from_toml_str(text).unwrap()
    }

    /// Parses `text`, expecting failure, and returns the rendered message.
    fn parse_err(text: &str) -> String {
        FileConfig::from_toml_str(text).unwrap_err().to_string()
    }

    #[test]
    fn empty_file_leaves_every_field_unset() {
        let cfg = parse("");
        assert_eq!(cfg.tokenizer, None);
        assert_eq!(cfg.verify, None);
        assert_eq!(cfg.python, None);
        assert_eq!(cfg.rust, None);
    }

    #[test]
    fn full_config_parses_every_field() {
        let cfg = parse(
            "tokenizer = \"cl100k_base\"\n\
             verify = \"reparse\"\n\
             [python]\n\
             strip_comments = true\n\
             strip_docstrings = true\n\
             strip_annotations = false\n\
             merge_imports = false\n\
             [rust]\n\
             strip_doc_comments = true\n",
        );
        assert_eq!(cfg.tokenizer.as_deref(), Some("cl100k_base"));
        assert_eq!(cfg.verify, Some(ConfigVerify::Reparse));
        assert_eq!(
            cfg.python,
            Some(PythonConfig {
                strip_comments: Some(true),
                strip_docstrings: Some(true),
                strip_annotations: Some(false),
                merge_imports: Some(false),
            })
        );
        assert_eq!(
            cfg.rust,
            Some(RustConfig {
                strip_doc_comments: Some(true)
            })
        );
    }

    #[test]
    fn partial_config_leaves_the_other_fields_unset() {
        let cfg = parse("tokenizer = \"o200k_base\"\n[python]\nstrip_comments = true\n");
        assert_eq!(cfg.tokenizer.as_deref(), Some("o200k_base"));
        assert_eq!(cfg.verify, None);
        assert_eq!(cfg.rust, None);
        let python = cfg.python.unwrap();
        assert_eq!(python.strip_comments, Some(true));
        assert_eq!(python.strip_docstrings, None);
        assert_eq!(python.strip_annotations, None);
        assert_eq!(python.merge_imports, None);
    }

    #[test]
    fn python_table_alone_parses() {
        let cfg = parse("[python]\nstrip_annotations = true\nmerge_imports = true\n");
        assert_eq!(cfg.rust, None);
        let python = cfg.python.unwrap();
        assert_eq!(python.strip_annotations, Some(true));
        assert_eq!(python.merge_imports, Some(true));
    }

    #[test]
    fn rust_table_alone_parses() {
        let cfg = parse("[rust]\nstrip_doc_comments = false\n");
        assert_eq!(cfg.python, None);
        assert_eq!(
            cfg.rust,
            Some(RustConfig {
                strip_doc_comments: Some(false)
            })
        );
    }

    #[test]
    fn empty_tables_are_valid_and_leave_their_keys_unset() {
        let cfg = parse("[python]\n[rust]\n");
        assert_eq!(
            cfg.python,
            Some(PythonConfig {
                strip_comments: None,
                strip_docstrings: None,
                strip_annotations: None,
                merge_imports: None,
            })
        );
        assert_eq!(
            cfg.rust,
            Some(RustConfig {
                strip_doc_comments: None
            })
        );
    }

    #[test]
    fn every_verify_value_is_accepted() {
        for (text, expected) in [
            ("reparse", ConfigVerify::Reparse),
            ("ast", ConfigVerify::Ast),
            ("external", ConfigVerify::External),
        ] {
            let cfg = parse(&format!("verify = \"{text}\"\n"));
            assert_eq!(cfg.verify, Some(expected));
        }
    }

    #[test]
    fn invalid_verify_value_names_the_value_and_the_alternatives() {
        let msg = parse_err("verify = \"strict\"\n");
        assert!(msg.contains("strict"), "{msg}");
        assert!(msg.contains("reparse"), "{msg}");
        assert!(msg.contains("ast"), "{msg}");
        assert!(msg.contains("external"), "{msg}");
    }

    #[test]
    fn unknown_top_level_key_is_an_error() {
        let msg = parse_err("tokeniser = \"o200k_base\"\n");
        assert!(msg.contains("tokeniser"), "{msg}");
    }

    #[test]
    fn unknown_python_key_is_an_error() {
        let msg = parse_err("[python]\nstrip_comment = true\n");
        assert!(msg.contains("strip_comment"), "{msg}");
    }

    #[test]
    fn unknown_rust_key_is_an_error() {
        let msg = parse_err("[rust]\nstrip_docs = true\n");
        assert!(msg.contains("strip_docs"), "{msg}");
    }

    #[test]
    fn wrong_value_type_is_an_error() {
        let msg = parse_err("[python]\nstrip_comments = \"yes\"\n");
        assert!(msg.contains("boolean"), "{msg}");
        let msg = parse_err("tokenizer = 1\n");
        assert!(msg.contains("string"), "{msg}");
    }

    #[test]
    fn malformed_toml_is_an_error() {
        let msg = parse_err("tokenizer = \n");
        assert!(msg.contains("invalid config file"), "{msg}");
    }

    #[test]
    fn load_reads_and_parses_a_file() {
        let dir = Scratch::new();
        let path = dir.file("tokenpress.toml", "verify = \"ast\"\n");
        let cfg = FileConfig::load(&path).unwrap();
        assert_eq!(cfg.verify, Some(ConfigVerify::Ast));
    }

    #[test]
    fn load_reports_an_unreadable_file_with_its_path() {
        let dir = Scratch::new();
        let path = dir.0.join("missing.toml");
        let err = FileConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Read { .. }));
        let msg = err.to_string();
        assert!(msg.contains("cannot read config file"), "{msg}");
        assert!(msg.contains("missing.toml"), "{msg}");
    }

    #[test]
    fn load_propagates_parse_errors() {
        let dir = Scratch::new();
        let path = dir.file("tokenpress.toml", "verify = \"nope\"\n");
        let err = FileConfig::load(&path).unwrap_err();
        assert!(matches!(err, ConfigError::Parse(_)));
    }

    #[test]
    fn errors_are_debug_printable() {
        let err = FileConfig::from_toml_str("verify = \"nope\"\n").unwrap_err();
        assert!(format!("{err:?}").contains("Parse"));
    }
}
