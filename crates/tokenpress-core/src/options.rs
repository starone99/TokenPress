use crate::TokenizerKind;

/// How strictly the output is checked before it is accepted.
///
/// Output failing the selected level is discarded with
/// [`crate::Error::Verification`] — it is never written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VerifyLevel {
    /// Re-parse the output only (fast).
    Reparse,
    /// Re-parse and compare normalized ASTs / token streams.
    #[default]
    AstEquiv,
    /// Reserved for additionally running external tooling (`py_compile`,
    /// `rustc`). Not implemented yet: every backend currently treats this
    /// level exactly like [`VerifyLevel::AstEquiv`].
    External,
}

/// Language-agnostic formatting options.
///
/// Language-specific choices (comments, docstrings, ...) live in each
/// language crate's own options type, configured at formatter construction.
#[derive(Clone, Debug, Default)]
pub struct FormatOptions {
    pub tokenizer: TokenizerKind,
    pub verify: VerifyLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_o200k_and_ast_equivalence() {
        let opts = FormatOptions::default();
        assert_eq!(opts.tokenizer, TokenizerKind::O200kBase);
        assert_eq!(opts.verify, VerifyLevel::AstEquiv);
    }

    #[test]
    fn options_are_cloneable_and_debuggable() {
        let opts = FormatOptions {
            tokenizer: TokenizerKind::Cl100kBase,
            verify: VerifyLevel::External,
        };
        let copy = opts.clone();
        assert_eq!(copy.tokenizer, TokenizerKind::Cl100kBase);
        assert_eq!(copy.verify, VerifyLevel::External);
        assert!(format!("{opts:?}").contains("Cl100kBase"));
    }

    #[test]
    fn verify_levels_are_distinct() {
        assert_ne!(VerifyLevel::Reparse, VerifyLevel::AstEquiv);
        assert_ne!(VerifyLevel::AstEquiv, VerifyLevel::External);
    }
}
