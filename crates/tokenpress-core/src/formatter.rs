use std::path::Path;

use crate::{FormatOptions, Result};

/// Outcome of formatting one source file.
#[derive(Clone, Debug, PartialEq)]
pub struct FormatResult {
    pub code: String,
    pub original_tokens: usize,
    pub formatted_tokens: usize,
}

impl FormatResult {
    pub fn tokens_saved(&self) -> usize {
        self.original_tokens.saturating_sub(self.formatted_tokens)
    }

    /// Fraction of tokens saved, in `0.0..=1.0`. Zero-token input saves 0.
    pub fn saving_ratio(&self) -> f64 {
        if self.original_tokens == 0 {
            0.0
        } else {
            self.tokens_saved() as f64 / self.original_tokens as f64
        }
    }
}

/// A language-specific formatter. Implementations hold their own
/// language-level configuration (version, comment handling, ...).
pub trait Formatter: Send + Sync {
    /// Language name shown to users, e.g. `"python"`.
    fn language(&self) -> &'static str;

    /// Whether this formatter handles the given path (by extension).
    fn supports(&self, path: &Path) -> bool;

    /// Formats `source`, returning token-minimized code that passed the
    /// verification level in `options`.
    fn format(&self, source: &str, options: &FormatOptions) -> Result<FormatResult>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(original: usize, formatted: usize) -> FormatResult {
        FormatResult {
            code: String::new(),
            original_tokens: original,
            formatted_tokens: formatted,
        }
    }

    #[test]
    fn tokens_saved_is_the_difference() {
        assert_eq!(result(100, 73).tokens_saved(), 27);
    }

    #[test]
    fn tokens_saved_saturates_when_output_grew() {
        assert_eq!(result(10, 12).tokens_saved(), 0);
    }

    #[test]
    fn saving_ratio_is_a_fraction() {
        assert!((result(200, 150).saving_ratio() - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn saving_ratio_of_empty_input_is_zero() {
        assert_eq!(result(0, 0).saving_ratio(), 0.0);
    }

    #[test]
    fn formatter_trait_is_object_safe() {
        struct Fixed;
        impl Formatter for Fixed {
            fn language(&self) -> &'static str {
                "fixed"
            }
            fn supports(&self, path: &Path) -> bool {
                path.extension().is_some_and(|e| e == "fx")
            }
            fn format(&self, source: &str, _: &FormatOptions) -> Result<FormatResult> {
                Ok(FormatResult {
                    code: source.to_string(),
                    original_tokens: 1,
                    formatted_tokens: 1,
                })
            }
        }
        let f: Box<dyn Formatter> = Box::new(Fixed);
        assert_eq!(f.language(), "fixed");
        assert!(f.supports(Path::new("a.fx")));
        assert!(!f.supports(Path::new("a.py")));
        let r = f.format("src", &FormatOptions::default()).unwrap();
        assert_eq!(r.code, "src");
        assert_eq!(r, r.clone());
    }
}
