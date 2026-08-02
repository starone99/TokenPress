//! Which paths the Ruby backend claims.
//!
//! The other backends match on the extension alone, but Ruby's build files
//! carry no extension: `Gemfile` and `Rakefile` are ordinary Ruby, and a Ruby
//! formatter that skipped them would miss the files most repositories have.
//! `Formatter::supports` receives a full `&Path`, so the file name is
//! available and basename matching is used for exactly those two names.

use std::path::Path;

/// True when `path` names a Ruby source file.
///
/// Accepted by extension: `rb`, `rake`, `gemspec`, `ru` (`config.ru`, the
/// Rack entry point). Accepted by exact, **case-sensitive** file name:
/// `Gemfile`, `Rakefile` — the names the tools themselves look for, so
/// `gemfile` is not a Ruby file and `Gemfile.lock` is not Ruby at all.
pub fn supports_path(path: &Path) -> bool {
    if path
        .extension()
        .is_some_and(|e| matches!(e.to_str(), Some("rb" | "rake" | "gemspec" | "ru")))
    {
        return true;
    }
    path.file_name()
        .is_some_and(|n| matches!(n.to_str(), Some("Gemfile" | "Rakefile")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_ruby_extensions() {
        for name in [
            "a.rb",
            "tasks.rake",
            "tokenpress.gemspec",
            "config.ru",
            "lib/deep/nested.rb",
        ] {
            assert!(supports_path(Path::new(name)), "{name} should be supported");
        }
    }

    #[test]
    fn accepts_the_extensionless_build_files_by_name() {
        for name in ["Gemfile", "Rakefile", "sub/dir/Gemfile", "sub/dir/Rakefile"] {
            assert!(supports_path(Path::new(name)), "{name} should be supported");
        }
    }

    #[test]
    fn rejects_other_languages_and_near_misses() {
        for name in [
            "a.py",
            "a.rs",
            "a.txt",
            // Case-sensitive: the tools look for the capitalized names.
            "gemfile",
            "rakefile",
            // A lockfile is not Ruby, and its extension is not accepted.
            "Gemfile.lock",
            // Extensionless and not one of the two known names.
            "foo",
            "rb",
            "a.",
        ] {
            assert!(!supports_path(Path::new(name)), "{name} should be rejected");
        }
    }

    #[test]
    fn rejects_a_path_with_no_file_name() {
        assert!(!supports_path(Path::new("..")));
    }
}
