//! Library surface of the `tokenpress` binary. All logic lives here (not in
//! `main.rs`) so it is fully covered by the coverage gate.

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use tokenpress_core::{
    Error, FormatOptions, FormatResult, Formatter, Result, TokenizerKind, VerifyLevel,
};
use tokenpress_python::{PythonFormatter, PythonOptions};
use tokenpress_rust::{RustFormatter, RustOptions};

#[derive(Parser)]
#[command(
    name = "tokenpress",
    version,
    about = "Token-aware formatter for Python and Rust"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum VerifyArg {
    Reparse,
    Ast,
    External,
}

#[derive(Args)]
struct CommonOpts {
    /// Files or directories to process (`.py` / `.rs`).
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Tokenizer to optimize for: o200k_base | cl100k_base |
    /// hf:<tokenizer.json> | kimi:<tiktoken.model>.
    #[arg(long, default_value = "o200k_base")]
    tokenizer: String,
    /// Verification level applied to every output.
    #[arg(long, value_enum, default_value = "ast")]
    verify: VerifyArg,
    /// PYO1: strip `#` comments (kept by default).
    #[arg(long)]
    py_strip_comments: bool,
    /// PYO3: strip type annotations. Changes `__annotations__`; breaks
    /// dataclass/pydantic-style runtime introspection.
    #[arg(long)]
    py_strip_annotations: bool,
    /// PY09: disable merging of adjacent import statements.
    #[arg(long)]
    py_no_merge_imports: bool,
    /// RSO1: strip doc comments (`///`, `//!`).
    #[arg(long)]
    rs_strip_doc_comments: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// Rewrite files in place with token-minimized formatting.
    Format {
        #[command(flatten)]
        common: CommonOpts,
        /// Print the result to stdout instead of writing files.
        #[arg(long)]
        stdout: bool,
    },
    /// Exit 1 if any file would change (CI gate). Writes nothing.
    Check {
        #[command(flatten)]
        common: CommonOpts,
    },
    /// Show a unified diff of what `format` would do. Writes nothing.
    Diff {
        #[command(flatten)]
        common: CommonOpts,
    },
    /// Report token counts and savings. Writes nothing.
    Stats {
        #[command(flatten)]
        common: CommonOpts,
        /// Machine-readable output.
        #[arg(long)]
        json: bool,
    },
}

/// Runs the CLI and returns the process exit code.
/// Exit codes: 0 = success, 1 = `check` found changes, 2 = error.
pub fn run<I, T>(args: I, out: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let _ = write!(out, "{err}");
            return match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => 2,
            };
        }
    };
    let (common, action) = match &cli.cmd {
        Cmd::Format { common, stdout } => (common, Action::Format { to_stdout: *stdout }),
        Cmd::Check { common } => (common, Action::Check),
        Cmd::Diff { common } => (common, Action::Diff),
        Cmd::Stats { common, json } => (common, Action::Stats { json: *json }),
    };
    match execute(common, action, out) {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(out, "error: {err}");
            2
        }
    }
}

#[derive(Clone, Copy)]
enum Action {
    Format { to_stdout: bool },
    Check,
    Diff,
    Stats { json: bool },
}

struct FileOutcome {
    path: PathBuf,
    original: String,
    result: FormatResult,
}

fn formatters(common: &CommonOpts) -> Vec<Box<dyn Formatter>> {
    vec![
        Box::new(PythonFormatter::new(PythonOptions {
            strip_comments: common.py_strip_comments,
            strip_annotations: common.py_strip_annotations,
            merge_imports: !common.py_no_merge_imports,
        })),
        Box::new(RustFormatter::new(RustOptions {
            strip_doc_comments: common.rs_strip_doc_comments,
        })),
    ]
}

fn format_options(common: &CommonOpts) -> Result<FormatOptions> {
    Ok(FormatOptions {
        tokenizer: TokenizerKind::from_name(&common.tokenizer)?,
        verify: match common.verify {
            VerifyArg::Reparse => VerifyLevel::Reparse,
            VerifyArg::Ast => VerifyLevel::AstEquiv,
            VerifyArg::External => VerifyLevel::External,
        },
    })
}

/// Expands the given paths into the sorted list of formattable files.
/// A directory is walked (`.gitignore`-aware); a file given explicitly must
/// be supported, otherwise it is an error.
fn discover(paths: &[PathBuf], formatters: &[Box<dyn Formatter>]) -> Result<Vec<PathBuf>> {
    let supported = |p: &Path| formatters.iter().any(|f| f.supports(p));
    let mut files = Vec::new();
    for path in paths {
        let meta = std::fs::metadata(path)?;
        if meta.is_dir() {
            // Unreadable entries are skipped, like ripgrep does.
            for entry in ignore::WalkBuilder::new(path).build().flatten() {
                let p = entry.path();
                if entry.file_type().is_some_and(|t| t.is_file()) && supported(p) {
                    files.push(p.to_path_buf());
                }
            }
        } else if supported(path) {
            files.push(path.clone());
        } else {
            return Err(Error::UnsupportedLanguage(path.display().to_string()));
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn execute(common: &CommonOpts, action: Action, out: &mut dyn Write) -> Result<i32> {
    let formatters = formatters(common);
    let options = format_options(common)?;
    let files = discover(&common.paths, &formatters)?;

    let mut outcomes = Vec::new();
    let mut errored = false;
    for path in files {
        // A single unreadable file (e.g. a non-UTF-8 test fixture) must not
        // abort the run: report it and continue like other per-file errors.
        let source = match std::fs::read_to_string(&path) {
            Ok(source) => source,
            Err(err) => {
                let _ = writeln!(out, "error: {}: {err}", path.display());
                errored = true;
                continue;
            }
        };
        let formatter = formatters
            .iter()
            .find(|f| f.supports(&path))
            .expect("discover only returns supported files");
        match formatter.format(&source, &options) {
            Ok(result) => outcomes.push(FileOutcome {
                path,
                original: source,
                result,
            }),
            Err(err) => {
                let _ = writeln!(out, "error: {}: {err}", path.display());
                errored = true;
            }
        }
    }

    let code = match action {
        Action::Format { to_stdout } => {
            for o in &outcomes {
                if to_stdout {
                    let _ = writeln!(out, "{}", o.result.code);
                } else if o.result.code != o.original {
                    std::fs::write(&o.path, &o.result.code)?;
                }
            }
            report_counts(&outcomes, out);
            0
        }
        Action::Check => {
            let changed: Vec<&FileOutcome> = outcomes
                .iter()
                .filter(|o| o.result.code != o.original)
                .collect();
            for o in &changed {
                let _ = writeln!(out, "would reformat: {}", o.path.display());
            }
            let _ = writeln!(
                out,
                "{} of {} files would change",
                changed.len(),
                outcomes.len()
            );
            if changed.is_empty() {
                0
            } else {
                1
            }
        }
        Action::Diff => {
            for o in &outcomes {
                if o.result.code != o.original {
                    let diff = similar::TextDiff::from_lines(&o.original, &o.result.code);
                    let name = o.path.display().to_string();
                    let _ = write!(
                        out,
                        "{}",
                        diff.unified_diff()
                            .header(&format!("a/{name}"), &format!("b/{name}"))
                    );
                }
            }
            0
        }
        Action::Stats { json } => {
            if json {
                report_json(&outcomes, out);
            } else {
                report_counts(&outcomes, out);
            }
            0
        }
    };
    Ok(if errored { 2 } else { code })
}

fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut with_sep = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            with_sep.push(',');
        }
        with_sep.push(c);
    }
    with_sep
}

fn counts_line(label: &str, original: usize, formatted: usize, out: &mut dyn Write) {
    let saved_pct = if original == 0 {
        0.0
    } else {
        original.saturating_sub(formatted) as f64 / original as f64 * 100.0
    };
    let _ = writeln!(
        out,
        "{label}  {} → {} tokens  (-{saved_pct:.1}%)",
        thousands(original),
        thousands(formatted),
    );
}

fn report_counts(outcomes: &[FileOutcome], out: &mut dyn Write) {
    for o in outcomes {
        counts_line(
            &o.path.display().to_string(),
            o.result.original_tokens,
            o.result.formatted_tokens,
            out,
        );
    }
    if outcomes.len() > 1 {
        let orig: usize = outcomes.iter().map(|o| o.result.original_tokens).sum();
        let new: usize = outcomes.iter().map(|o| o.result.formatted_tokens).sum();
        counts_line(&format!("{} files", outcomes.len()), orig, new, out);
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn report_json(outcomes: &[FileOutcome], out: &mut dyn Write) {
    let entries: Vec<String> = outcomes
        .iter()
        .map(|o| {
            format!(
                "{{\"path\":\"{}\",\"original_tokens\":{},\"formatted_tokens\":{}}}",
                json_escape(&o.path.display().to_string()),
                o.result.original_tokens,
                o.result.formatted_tokens
            )
        })
        .collect();
    let orig: usize = outcomes.iter().map(|o| o.result.original_tokens).sum();
    let new: usize = outcomes.iter().map(|o| o.result.formatted_tokens).sum();
    let _ = writeln!(
        out,
        "{{\"files\":[{}],\"original_tokens\":{orig},\"formatted_tokens\":{new}}}",
        entries.join(",")
    );
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
                "tokenpress-cli-test-{}-{}",
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

    fn run_cli(args: &[&str]) -> (i32, String) {
        let mut argv = vec!["tokenpress"];
        argv.extend(args);
        let mut out = Vec::new();
        let code = run(argv, &mut out);
        (code, String::from_utf8(out).unwrap())
    }

    #[test]
    fn help_and_version_exit_zero() {
        let (code, text) = run_cli(&["--help"]);
        assert_eq!(code, 0);
        assert!(text.contains("format"));
        let (code, _) = run_cli(&["--version"]);
        assert_eq!(code, 0);
    }

    #[test]
    fn bad_arguments_exit_two() {
        let (code, _) = run_cli(&["explode"]);
        assert_eq!(code, 2);
        let (code, _) = run_cli(&["format"]);
        assert_eq!(code, 2);
    }

    #[test]
    fn format_rewrites_python_and_rust_files() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n\ny = 2\n");
        let rs = dir.file("b.rs", "fn f() -> u8 {\n    1 + 2\n}\n");
        let (code, text) = run_cli(&["format", py.to_str().unwrap(), rs.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x=1\ny=2");
        assert_eq!(std::fs::read_to_string(&rs).unwrap(), "fn f()->u8{1+2}");
        assert!(text.contains("tokens"));
        assert!(text.contains("2 files"));
    }

    #[test]
    fn format_stdout_leaves_files_untouched() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["format", "--stdout", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("x=1"));
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[test]
    fn format_skips_writing_already_minimal_files() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x=1");
        let before = std::fs::metadata(&py).unwrap().modified().unwrap();
        let (code, _) = run_cli(&["format", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::metadata(&py).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn check_reports_and_exits_one_on_changes() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["check", py.to_str().unwrap()]);
        assert_eq!(code, 1);
        assert!(text.contains("would reformat"));
        assert!(text.contains("1 of 1 files would change"));
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[test]
    fn check_exits_zero_when_clean() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x=1");
        let (code, text) = run_cli(&["check", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("0 of 1 files would change"));
    }

    #[test]
    fn diff_shows_unified_diff_only_for_changed_files() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let clean = dir.file("b.py", "y=2");
        let (code, text) = run_cli(&["diff", py.to_str().unwrap(), clean.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("-x = 1"));
        assert!(text.contains("+x=1"));
        assert!(!text.contains("b.py"));
    }

    #[test]
    fn stats_reports_without_writing() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "value = 1 + 2\n");
        let (code, text) = run_cli(&["stats", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("tokens"));
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "value = 1 + 2\n");
    }

    #[test]
    fn stats_json_is_machine_readable() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["stats", "--json", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.starts_with("{\"files\":[{\"path\":"));
        assert!(text.contains("\"original_tokens\":"));
        // Windows path separators must be escaped.
        assert!(!text.contains("\\a.py") || text.contains("\\\\a.py"));
    }

    #[test]
    fn directories_are_walked_recursively() {
        let dir = Scratch::new();
        dir.file("a.py", "x = 1\n");
        std::fs::create_dir_all(dir.0.join("sub")).unwrap();
        dir.file("sub/b.rs", "fn f() {}\n");
        dir.file("notes.txt", "ignored");
        let (code, text) = run_cli(&["check", dir.0.to_str().unwrap()]);
        assert_eq!(code, 1);
        assert!(text.contains("a.py"));
        assert!(text.contains("b.rs"));
        assert!(!text.contains("notes.txt"));
    }

    #[test]
    fn explicit_unsupported_file_is_an_error() {
        let dir = Scratch::new();
        let txt = dir.file("notes.txt", "hi");
        let (code, text) = run_cli(&["format", txt.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("unsupported language"));
    }

    #[test]
    fn non_utf8_file_is_reported_but_does_not_abort_the_run() {
        let dir = Scratch::new();
        let bad = dir.0.join("bad.py");
        std::fs::write(&bad, [0xFF, 0xFE, 0x00]).unwrap();
        let good = dir.file("good.py", "x = 1\n");
        let (code, text) = run_cli(&["format", bad.to_str().unwrap(), good.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("bad.py"));
        assert_eq!(std::fs::read_to_string(&good).unwrap(), "x=1");
    }

    #[test]
    fn missing_path_is_an_error() {
        let (code, text) = run_cli(&["format", "no-such-file.py"]);
        assert_eq!(code, 2);
        assert!(text.contains("error:"));
    }

    #[test]
    fn unknown_tokenizer_is_an_error() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["format", "--tokenizer", "nope", py.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("unknown tokenizer: nope"));
    }

    #[test]
    fn parse_error_continues_with_other_files_but_exits_two() {
        let dir = Scratch::new();
        let bad = dir.file("bad.py", "def f(:\n");
        let good = dir.file("good.py", "x = 1\n");
        let (code, text) = run_cli(&["format", bad.to_str().unwrap(), good.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("parse error"));
        assert_eq!(std::fs::read_to_string(&good).unwrap(), "x=1");
    }

    #[test]
    fn language_flags_are_forwarded() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "# note\nx: int = 1\n");
        let rs = dir.file("b.rs", "/// doc\nfn f() {}\n");
        let (code, _) = run_cli(&[
            "format",
            "--py-strip-comments",
            "--py-strip-annotations",
            "--rs-strip-doc-comments",
            py.to_str().unwrap(),
            rs.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x=1");
        assert_eq!(std::fs::read_to_string(&rs).unwrap(), "fn f(){}");
    }

    #[test]
    fn comments_and_annotations_are_kept_and_imports_merged_by_default() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "# note\nimport os\nimport sys\nx: int = 1\n");
        let (code, _) = run_cli(&["format", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&py).unwrap(),
            "# note\nimport os,sys\nx:int=1"
        );
    }

    #[test]
    fn import_merging_can_be_disabled() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "import os\nimport sys\n");
        let (code, _) = run_cli(&["format", "--py-no-merge-imports", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&py).unwrap(),
            "import os\nimport sys"
        );
    }

    #[test]
    fn verify_levels_are_selectable() {
        let dir = Scratch::new();
        for level in ["reparse", "ast", "external"] {
            let py = dir.file(&format!("{level}.py"), "x = 1\n");
            let (code, _) = run_cli(&["format", "--verify", level, py.to_str().unwrap()]);
            assert_eq!(code, 0);
        }
    }

    #[test]
    fn cl100k_tokenizer_is_accepted() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _) = run_cli(&["stats", "--tokenizer", "cl100k_base", py.to_str().unwrap()]);
        assert_eq!(code, 0);
    }

    #[test]
    fn thousands_separators_format_correctly() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1,000");
        assert_eq!(thousands(1234567), "1,234,567");
    }

    #[test]
    fn counts_line_handles_zero_token_input() {
        let mut out = Vec::new();
        counts_line("empty.py", 0, 0, &mut out);
        assert!(String::from_utf8(out).unwrap().contains("(-0.0%)"));
    }

    #[test]
    fn json_escape_handles_quotes_and_backslashes() {
        assert_eq!(json_escape("a\\b\"c"), "a\\\\b\\\"c");
    }
}
