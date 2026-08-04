//! Which paths the Java backend claims.
//!
//! A plain extension check, exactly Go's shape and unlike Ruby's basename
//! matching: `module-info.java` and `package-info.java` are ordinary `.java`
//! sources needing no special case, `.jsh` jshell snippets are not Java
//! source, and `.class` / `.jar` are binaries. There is therefore no
//! basename matching here, and nothing this module has to know about a
//! project layout.

use std::path::Path;

/// True when `path` names a Java source file.
///
/// Accepted by extension, and by extension only: `java`. The match is
/// **case-sensitive**, the same convention the Go and Ruby backends use: the
/// Java toolchain looks for `.java`, so `A.JAVA` is not a Java file to it and
/// is not one here either.
pub fn supports_path(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.to_str() == Some("java"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_java_extension() {
        for name in [
            "A.java",
            "Main.java",
            "src/main/java/org/example/deep/Thing.java",
            "/abs/path/A.java",
            // `module-info.java` and `package-info.java` are ordinary `.java`
            // sources — no basename rule is needed for either.
            "module-info.java",
            "package-info.java",
            // A dotted stem keeps the last extension.
            "A.b.java",
        ] {
            assert!(supports_path(Path::new(name)), "{name} should be supported");
        }
    }

    #[test]
    fn rejects_other_languages_and_near_misses() {
        let other_languages = ["a.go", "a.py", "a.rs", "a.rb", "a.txt"];
        // Case-sensitive: the toolchain looks for `.java`.
        let case_variants = ["A.JAVA", "A.Java", "A.jaVa"];
        // Near misses on the extension itself.
        let near_misses = ["A.javax", "A.jav", "A.j", "A.java.txt", "A."];
        // Compiled and packaged Java is not Java source, `.jsh` jshell
        // snippets are not Java source either, and no extensionless name is
        // matched, because no basename matching happens here.
        let not_source = ["A.class", "a.jar", "A.jsh", "java", "Makefile", "foo"];
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
