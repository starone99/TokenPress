use std::sync::{Arc, OnceLock};

use tiktoken_rs::CoreBPE;

use crate::{Error, Result};

/// Counts tokens for a specific LLM vocabulary.
///
/// Language crates only ever need [`Tokenizer::count`] to decide which of two
/// equivalent renderings is cheaper, so adding new tokenizer backends (e.g.
/// HuggingFace `tokenizer.json`) never touches language crates.
pub trait Tokenizer: Send + Sync {
    fn name(&self) -> &str;
    fn count(&self, text: &str) -> usize;
}

/// A named, loadable tokenizer.
///
/// Loading parses multi-megabyte vocabulary data, so [`TokenizerKind::load`]
/// caches one instance per kind for the lifetime of the process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TokenizerKind {
    #[default]
    O200kBase,
    Cl100kBase,
}

impl TokenizerKind {
    /// Resolves a CLI-facing name like `"o200k_base"`.
    pub fn from_name(name: &str) -> Result<Self> {
        match name {
            "o200k_base" => Ok(Self::O200kBase),
            "cl100k_base" => Ok(Self::Cl100kBase),
            other => Err(Error::UnknownTokenizer(other.to_string())),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::O200kBase => "o200k_base",
            Self::Cl100kBase => "cl100k_base",
        }
    }

    pub fn load(self) -> Arc<dyn Tokenizer> {
        static O200K: OnceLock<Arc<Tiktoken>> = OnceLock::new();
        static CL100K: OnceLock<Arc<Tiktoken>> = OnceLock::new();
        let cached = match self {
            Self::O200kBase => O200K.get_or_init(|| {
                Arc::new(Tiktoken {
                    name: "o200k_base",
                    bpe: tiktoken_rs::o200k_base().expect("embedded o200k_base vocabulary"),
                })
            }),
            Self::Cl100kBase => CL100K.get_or_init(|| {
                Arc::new(Tiktoken {
                    name: "cl100k_base",
                    bpe: tiktoken_rs::cl100k_base().expect("embedded cl100k_base vocabulary"),
                })
            }),
        };
        cached.clone()
    }
}

struct Tiktoken {
    name: &'static str,
    bpe: CoreBPE,
}

impl Tokenizer for Tiktoken {
    fn name(&self) -> &str {
        self.name
    }

    fn count(&self, text: &str) -> usize {
        self.bpe.encode_ordinary(text).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_resolves_known_tokenizers() {
        assert_eq!(
            TokenizerKind::from_name("o200k_base").unwrap(),
            TokenizerKind::O200kBase
        );
        assert_eq!(
            TokenizerKind::from_name("cl100k_base").unwrap(),
            TokenizerKind::Cl100kBase
        );
    }

    #[test]
    fn from_name_rejects_unknown_tokenizers() {
        let err = TokenizerKind::from_name("gpt9_base").unwrap_err();
        assert_eq!(err.to_string(), "unknown tokenizer: gpt9_base");
    }

    #[test]
    fn default_is_o200k() {
        assert_eq!(TokenizerKind::default(), TokenizerKind::O200kBase);
    }

    #[test]
    fn kind_and_loaded_tokenizer_agree_on_name() {
        for kind in [TokenizerKind::O200kBase, TokenizerKind::Cl100kBase] {
            assert_eq!(kind.load().name(), kind.name());
        }
    }

    #[test]
    fn counts_empty_as_zero_and_code_as_nonzero() {
        let tok = TokenizerKind::O200kBase.load();
        assert_eq!(tok.count(""), 0);
        assert!(tok.count("def add(a, b):\n    return a + b\n") > 0);
        let tok = TokenizerKind::Cl100kBase.load();
        assert_eq!(tok.count(""), 0);
        assert!(tok.count("fn main() {}\n") > 0);
    }

    #[test]
    fn load_caches_one_instance_per_kind() {
        let a = TokenizerKind::O200kBase.load();
        let b = TokenizerKind::O200kBase.load();
        assert!(Arc::ptr_eq(&a, &b));
    }

    #[test]
    fn whitespace_removal_reduces_token_count() {
        // The premise of the whole project: fewer syntax tokens in, fewer
        // LLM tokens out.
        let tok = TokenizerKind::O200kBase.load();
        assert!(tok.count("x=f(a,b)") <= tok.count("x  =  f( a , b )"));
    }
}
