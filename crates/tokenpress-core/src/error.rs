/// Errors shared across all TokenPress crates.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("parse error: {0}")]
    Parse(String),
    /// The transformed output failed re-parsing or equivalence checking.
    /// Output that fails verification is never written anywhere.
    #[error("verification failed: {0}")]
    Verification(String),
    #[error("unsupported language for path: {0}")]
    UnsupportedLanguage(String),
    #[error("unknown tokenizer: {0}")]
    UnknownTokenizer(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages() {
        assert_eq!(
            Error::Parse("bad token".into()).to_string(),
            "parse error: bad token"
        );
        assert_eq!(
            Error::Verification("ast mismatch".into()).to_string(),
            "verification failed: ast mismatch"
        );
        assert_eq!(
            Error::UnsupportedLanguage("a.txt".into()).to_string(),
            "unsupported language for path: a.txt"
        );
        assert_eq!(
            Error::UnknownTokenizer("nope".into()).to_string(),
            "unknown tokenizer: nope"
        );
    }

    #[test]
    fn io_error_converts_and_displays_transparently() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing.py");
        let err: Error = io.into();
        assert_eq!(err.to_string(), "missing.py");
        assert!(matches!(err, Error::Io(_)));
    }

    #[test]
    fn debug_is_derived() {
        let err = Error::Parse("x".into());
        assert!(format!("{err:?}").contains("Parse"));
    }
}
