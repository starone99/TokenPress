//! Which paths the Go backend claims.
//!
//! Unlike Ruby's, this is a plain extension check: Go's build files are
//! ordinary `.go` sources (`//go:build` is a comment, not a file name), and
//! the extensionless files a Go repository does carry — `go.mod`, `go.sum` —
//! are not Go source at all. There is therefore no basename matching here,
//! and nothing this module has to know about a project layout.

use std::path::Path;

/// True when `path` names a Go source file.
///
/// Accepted by extension, and by extension only: `go`. The match is
/// **case-sensitive**, the same convention the Ruby backend uses: the Go
/// toolchain looks for `.go`, so `a.GO` is not a Go file to it and is not one
/// here either.
pub fn supports_path(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.to_str() == Some("go"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_go_extension() {
        for name in [
            "a.go",
            "main.go",
            "internal/deep/nested/thing_test.go",
            "/abs/path/a.go",
            // A dotted stem keeps the last extension.
            "a.b.go",
        ] {
            assert!(supports_path(Path::new(name)), "{name} should be supported");
        }
    }

    #[test]
    fn rejects_other_languages_and_near_misses() {
        let other_languages = ["a.py", "a.rs", "a.rb", "a.txt"];
        // Case-sensitive: the toolchain looks for `.go`.
        let case_variants = ["a.GO", "a.Go", "a.gO"];
        // Near misses on the extension itself.
        let near_misses = ["a.gox", "a.gopher", "a.g", "a.go.txt", "a."];
        // Go's own module metadata is not Go source, and neither is any
        // extensionless name — including the bare name of the language,
        // because no basename matching happens here.
        let not_source = ["go.mod", "go.sum", "go", "Gofile", "foo"];
        for name in other_languages
            .iter()
            .chain(&case_variants)
            .chain(&near_misses)
            .chain(&not_source)
        {
            assert!(!supports_path(Path::new(name)), "{name} should be rejected");
        }
    }

    #[test]
    fn rejects_a_path_with_no_file_name() {
        assert!(!supports_path(Path::new("..")));
    }
}
