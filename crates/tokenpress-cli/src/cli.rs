//! Library surface of the `tokenpress` binary. All logic lives here (not in
//! `main.rs`) so it is fully covered by the coverage gate.

pub mod config;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand, ValueEnum};
use config::{ConfigError, ConfigVerify, FileConfig};
use tokenpress_core::{
    Error, FormatOptions, FormatResult, Formatter, Result, TokenizerKind, VerifyLevel,
};
use tokenpress_js::{JsFormatter, JsOptions};
use tokenpress_python::{PythonFormatter, PythonOptions};
#[cfg(feature = "ruby")]
use tokenpress_ruby::{RubyFormatter, RubyOptions};
use tokenpress_rust::{RustFormatter, RustOptions};

#[derive(Parser)]
#[command(name = "tokenpress", version)]
// The `ruby` cargo feature is default-on; a build without it has no Ruby
// backend at all, so nothing here may advertise one.
#[cfg_attr(
    feature = "ruby",
    command(about = "Token-aware formatter for Python, Rust, JavaScript/TypeScript and Ruby")
)]
#[cfg_attr(
    not(feature = "ruby"),
    command(about = "Token-aware formatter for Python, Rust and JavaScript/TypeScript")
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
    /// Files or directories to process: `.py`, `.rs`, the
    /// JavaScript/TypeScript set `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts`
    // Written as `doc` attributes rather than `///` so the Ruby half can be
    // switched off with the backend itself; they concatenate in source order.
    #[cfg_attr(
        feature = "ruby",
        doc = " `.cts` `.tsx`, and the Ruby set `.rb` `.rake` `.gemspec` `.ru` plus the"
    )]
    #[cfg_attr(
        feature = "ruby",
        doc = " files named `Gemfile` and `Rakefile` (exact, case-sensitive names)."
    )]
    #[cfg_attr(not(feature = "ruby"), doc = " `.cts` and `.tsx`.")]
    #[arg(required = true)]
    paths: Vec<PathBuf>,
    /// Config file to read. Without it the nearest `tokenpress.toml` found
    /// walking up from the current directory is used, if there is one.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Tokenizer to optimize for: o200k_base | cl100k_base |
    /// hf:<tokenizer.json> | kimi:<tiktoken.model>. [default: o200k_base]
    // No clap default: an unset value must stay distinguishable from an
    // explicit one so the config file is not shadowed by the default.
    #[arg(long)]
    tokenizer: Option<String>,
    /// Verification level applied to every output. [default: ast]
    #[arg(long, value_enum)]
    verify: Option<VerifyArg>,
    /// PYO1: strip `#` comments (kept by default).
    #[arg(long)]
    py_strip_comments: bool,
    /// PYO2: strip docstrings — the leading string literal of a module, class
    /// or function body. Empties `__doc__`; breaks `help()` and doctests.
    #[arg(long)]
    py_strip_docstrings: bool,
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
    /// JSO1: strip JS/TS comments (kept by default — but see the caveat
    /// warning: trailing and expression-position comments are dropped either
    /// way).
    #[arg(long)]
    js_strip_comments: bool,
    /// RBO1: strip Ruby comments and embdocs (kept by default — and, unlike
    /// Rust and JS/TS, nothing is dropped without this flag). The shebang and
    /// the leading magic-comment window survive either way.
    #[cfg(feature = "ruby")]
    #[arg(long)]
    ruby_strip_comments: bool,
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

/// Name of the project config file looked for during auto-discovery.
const CONFIG_FILE_NAME: &str = "tokenpress.toml";

/// Default tokenizer, applied when neither the command line nor the config
/// file names one.
const DEFAULT_TOKENIZER: &str = "o200k_base";

/// Returns the nearest `tokenpress.toml` at or above `start`, if any.
fn discover_config(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .map(|dir| dir.join(CONFIG_FILE_NAME))
        .find(|candidate| candidate.is_file())
}

/// Loads the config file to apply. An explicit `--config` disables discovery
/// and must point at a readable file; without one the nearest `tokenpress.toml`
/// above `start` is used, and having none at all is not an error. A file that
/// exists but does not parse always fails, discovered or not.
// `Result` here is the core alias, so the config result is spelled in full.
fn load_config(
    explicit: Option<&Path>,
    start: &Path,
) -> std::result::Result<Option<FileConfig>, ConfigError> {
    let path = match explicit {
        Some(path) => Some(path.to_path_buf()),
        None => discover_config(start),
    };
    path.map(|path| FileConfig::load(&path)).transpose()
}

/// Verification level as spelled in the config file, mapped onto the
/// command line's own value.
fn verify_arg(verify: ConfigVerify) -> VerifyArg {
    match verify {
        ConfigVerify::Reparse => VerifyArg::Reparse,
        ConfigVerify::Ast => VerifyArg::Ast,
        ConfigVerify::External => VerifyArg::External,
    }
}

/// Merges `cfg` into the parsed command line: the config file only fills in
/// what the command line left unsaid. The strip flags are presence-only, so
/// the command line can turn them on but never off — the config provides the
/// project baseline.
fn apply_config(common: &mut CommonOpts, cfg: FileConfig) {
    common.tokenizer = common.tokenizer.take().or(cfg.tokenizer);
    common.verify = common.verify.or(cfg.verify.map(verify_arg));
    if let Some(python) = cfg.python {
        common.py_strip_comments |= python.strip_comments.unwrap_or(false);
        common.py_strip_docstrings |= python.strip_docstrings.unwrap_or(false);
        common.py_strip_annotations |= python.strip_annotations.unwrap_or(false);
        // `merge_imports = false` is the config spelling of the negative flag.
        common.py_no_merge_imports |= !python.merge_imports.unwrap_or(true);
    }
    if let Some(rust) = cfg.rust {
        common.rs_strip_doc_comments |= rust.strip_doc_comments.unwrap_or(false);
    }
    if let Some(javascript) = cfg.javascript {
        common.js_strip_comments |= javascript.strip_comments.unwrap_or(false);
    }
    #[cfg(feature = "ruby")]
    if let Some(ruby) = cfg.ruby {
        common.ruby_strip_comments |= ruby.strip_comments.unwrap_or(false);
    }
}

/// Runs the CLI and returns the process exit code.
/// Exit codes: 0 = success, 1 = `check` found changes, 2 = error.
/// `out` receives the primary (pipeable) output, `err` diagnostics.
pub fn run<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let mut cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let _ = write!(out, "{err}");
            return match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
                _ => 2,
            };
        }
    };
    let (common, action) = match &mut cli.cmd {
        Cmd::Format { common, stdout } => (common, Action::Format { to_stdout: *stdout }),
        Cmd::Check { common } => (common, Action::Check),
        Cmd::Diff { common } => (common, Action::Diff),
        Cmd::Stats { common, json } => (common, Action::Stats { json: *json }),
    };
    // An inaccessible current directory simply leaves nothing to discover.
    let cwd = std::env::current_dir().unwrap_or_default();
    match load_config(common.config.as_deref(), &cwd) {
        Ok(Some(cfg)) => apply_config(common, cfg),
        Ok(None) => {}
        // Config problems are usage errors: report them before any file is
        // read, and do not leave the run half-configured.
        Err(cfg_err) => {
            let _ = writeln!(err, "error: {cfg_err}");
            return 2;
        }
    }
    match execute(common, action, out, err) {
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
    // The list has a conditional tail, so it is built and then extended; the
    // `mut` is only needed when the `ruby` feature is on.
    #[cfg_attr(not(feature = "ruby"), allow(unused_mut))]
    let mut formatters: Vec<Box<dyn Formatter>> = vec![
        Box::new(PythonFormatter::new(PythonOptions {
            strip_comments: common.py_strip_comments,
            strip_docstrings: common.py_strip_docstrings,
            strip_annotations: common.py_strip_annotations,
            merge_imports: !common.py_no_merge_imports,
        })),
        Box::new(RustFormatter::new(RustOptions {
            strip_doc_comments: common.rs_strip_doc_comments,
        })),
        Box::new(JsFormatter::new(JsOptions {
            strip_comments: common.js_strip_comments,
        })),
    ];
    #[cfg(feature = "ruby")]
    formatters.push(Box::new(RubyFormatter::new(RubyOptions {
        strip_comments: common.ruby_strip_comments,
    })));
    formatters
}

fn format_options(common: &CommonOpts) -> Result<FormatOptions> {
    Ok(FormatOptions {
        tokenizer: TokenizerKind::from_name(
            common.tokenizer.as_deref().unwrap_or(DEFAULT_TOKENIZER),
        )?,
        verify: match common.verify.unwrap_or(VerifyArg::Ast) {
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

/// Caveats that apply to every rewritten Rust file. Both stem from re-emitting
/// the `syn` token stream, so they are reported together as one warning.
const RUST_CAVEAT_WARNING: &str = "\
warning: Rust output is not comment-preserving: `//` and `/* */` comments are
  always dropped; only `///` and `//!` doc comments survive, and only without
  --rs-strip-doc-comments. Whitespace inside macro bodies is minimized, which
  can change the runtime output of whitespace-sensitive macros such as
  `stringify!` — token-canonical verification cannot detect that.";

/// Writes the shared Rust caveat warning to `err` at most once per run: only
/// for the commands that rewrite code, and only if a `.rs` file is processed.
fn warn_rust_caveats(files: &[PathBuf], action: Action, err: &mut dyn Write) {
    let rewrites = matches!(action, Action::Format { .. } | Action::Check | Action::Diff);
    let any_rust = files
        .iter()
        .any(|p| p.extension().is_some_and(|ext| ext == "rs"));
    if rewrites && any_rust {
        let _ = writeln!(err, "{RUST_CAVEAT_WARNING}");
    }
}

/// Extensions the JS/TS backend claims. Kept next to the caveat warning
/// because that is the only place the CLI has to recognize them by name;
/// `JsFormatter::supports` remains the authority for dispatch.
const JS_EXTENSIONS: [&str; 8] = ["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];

/// Caveat that applies to every rewritten JS/TS file. It is a property of the
/// code generator, so no option can switch it off — hence a warning rather
/// than a note attached to `--js-strip-comments`.
const JS_CAVEAT_WARNING: &str = "\
warning: JavaScript/TypeScript output is not comment-preserving: trailing
  comments and comments in expression position are always dropped when
  re-emitting, even without --js-strip-comments. Only leading statement-level
  comments, jsdoc (`/** */`), annotation comments (such as `#__PURE__`) and
  legal comments (`//!`, `/*!`, `@license`, `@preserve`) survive. Verification
  cannot detect this: its canonical form is comment-free by construction.
  In JSX/TSX, JSX text is never compressed -- whitespace inside element
  children is significant -- so savings there come from the surrounding
  JavaScript only, and a comment-only expression container `{/* c */}` becomes
  `{}` under --js-strip-comments (valid JSX, renders identically).";

/// Writes the shared JS/TS caveat warning to `err` at most once per run: only
/// for the commands that rewrite code, and only if a JS/TS file is processed.
fn warn_js_caveats(files: &[PathBuf], action: Action, err: &mut dyn Write) {
    let rewrites = matches!(action, Action::Format { .. } | Action::Check | Action::Diff);
    let any_js = files.iter().any(|p| {
        p.extension()
            .is_some_and(|ext| JS_EXTENSIONS.iter().any(|js| ext == *js))
    });
    if rewrites && any_js {
        let _ = writeln!(err, "{JS_CAVEAT_WARNING}");
    }
}

// `--verify external` is real for JavaScript/TypeScript and for Ruby but
// still equals `--verify ast` for Python and Rust, so a run containing files
// of those two languages must not read the level as a stronger guarantee than
// it is. Only the first half — which backends do have it — depends on the
// `ruby` feature, so the warning is a conditional head plus a shared tail,
// written out as one block by `warn_external_verify`.
#[cfg(feature = "ruby")]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for JavaScript/TypeScript,
  where `--verify external` runs `tsc --noEmit` (falling back to
  `node --check`), and for Ruby, where it runs `ruby -c`; both fail if the tool
  they need is not on PATH.";

#[cfg(not(feature = "ruby"))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for JavaScript/TypeScript,
  where `--verify external` runs `tsc --noEmit` (falling back to
  `node --check`); it fails if the tool it needs is not on PATH.";

const EXTERNAL_VERIFY_WARNING_TAIL: &str = " It is not implemented for Python and Rust: neither
  `py_compile` nor `rustc --emit=metadata` is invoked, so for `.py` and `.rs`
  this level behaves exactly like `--verify ast`, i.e. the output is re-parsed
  and compared for AST / token-stream equivalence.";

/// Extensions the warning above is about: the backends the external level does
/// not reach yet.
const NO_EXTERNAL_VERIFY_EXTENSIONS: [&str; 2] = ["py", "rs"];

/// True when `path` belongs to a backend the external level does not reach.
fn lacks_external_verify(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        NO_EXTERNAL_VERIFY_EXTENSIONS
            .iter()
            .any(|affected| ext == *affected)
    })
}

/// Writes the external-verification warning to `err` at most once per run:
/// only at `--verify external`, and only when the run contains a file of a
/// language the level does not reach. Verification runs for every subcommand,
/// so the warning is not restricted to the rewriting ones.
fn warn_external_verify(common: &CommonOpts, files: &[PathBuf], err: &mut dyn Write) {
    let any_affected = files.iter().any(|p| lacks_external_verify(p));
    if matches!(common.verify, Some(VerifyArg::External)) && any_affected {
        let _ = writeln!(
            err,
            "{EXTERNAL_VERIFY_WARNING_HEAD}{EXTERNAL_VERIFY_WARNING_TAIL}"
        );
    }
}

fn execute(
    common: &CommonOpts,
    action: Action,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<i32> {
    let formatters = formatters(common);
    let options = format_options(common)?;
    let files = discover(&common.paths, &formatters)?;
    warn_external_verify(common, &files, err);
    warn_rust_caveats(&files, action, err);
    warn_js_caveats(&files, action, err);

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
        match formatter.format(&path, &source, &options) {
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

    /// Runs the CLI, returning the exit code together with stdout and stderr.
    fn run_cli_err(args: &[&str]) -> (i32, String, String) {
        let mut argv = vec!["tokenpress"];
        argv.extend(args);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(argv, &mut out, &mut err);
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn run_cli(args: &[&str]) -> (i32, String) {
        let (code, out, _) = run_cli_err(args);
        (code, out)
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
    fn help_lists_every_supported_extension_and_no_unsupported_one() {
        let (code, text) = run_cli(&["format", "--help"]);
        assert_eq!(code, 0);
        for ext in [
            ".py", ".rs", ".js", ".mjs", ".cjs", ".jsx", ".ts", ".mts", ".cts", ".tsx",
        ] {
            assert!(text.contains(ext), "{ext} missing from help:\n{text}");
        }
        assert!(text.contains("--js-strip-comments"), "{text}");
        #[cfg(feature = "ruby")]
        {
            for ext in [".rb", ".rake", ".gemspec", ".ru"] {
                assert!(text.contains(ext), "{ext} missing from help:\n{text}");
            }
            // Ruby is the only backend that also claims extensionless names,
            // so the blurb has to spell them out.
            for name in ["Gemfile", "Rakefile"] {
                assert!(text.contains(name), "{name} missing from help:\n{text}");
            }
            assert!(text.contains("--ruby-strip-comments"), "{text}");
        }
        // A build without the `ruby` feature has no Ruby backend, so the help
        // must not advertise one.
        #[cfg(not(feature = "ruby"))]
        for absent in [".rb", ".gemspec", "Gemfile", "Rakefile", "Ruby"] {
            assert!(!text.contains(absent), "{absent} in help:\n{text}");
        }
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
    fn docstring_stripping_flag_is_forwarded() {
        let dir = Scratch::new();
        let source = "\"\"\"Module doc.\"\"\"\ndef f():\n    \"\"\"Doc.\"\"\"\n    return 1\n";
        let py = dir.file("a.py", source);
        let (code, _) = run_cli(&["format", "--py-strip-docstrings", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "def f():\n return 1");
        // Without the flag the docstrings stay.
        let kept = dir.file("b.py", source);
        let (code, _) = run_cli(&["format", kept.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "\"\"\"Module doc.\"\"\"\ndef f():\n \"\"\"Doc.\"\"\"\n return 1"
        );
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
    fn rust_caveat_warning_is_emitted_once_per_run() {
        let dir = Scratch::new();
        let a = dir.file("a.rs", "// note\nfn f() {}\n");
        let b = dir.file("b.rs", "// note\nfn g() {}\n");
        let (code, out, err) = run_cli_err(&["format", a.to_str().unwrap(), b.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("`//`"));
        assert!(err.contains("stringify!"));
        // stdout stays clean and pipeable.
        assert!(!out.contains("warning:"));
    }

    #[test]
    fn rust_caveat_warning_is_absent_for_python_only_runs() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err, "");
    }

    #[test]
    fn rust_caveat_warning_covers_check_and_diff_but_not_stats() {
        let dir = Scratch::new();
        let rs = dir.file("a.rs", "fn f() {}\n");
        let (code, _, err) = run_cli_err(&["check", rs.to_str().unwrap()]);
        assert_eq!(code, 1);
        assert_eq!(err.matches("warning:").count(), 1);
        let (code, _, err) = run_cli_err(&["diff", rs.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        let (code, _, err) = run_cli_err(&["stats", rs.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err, "");
    }

    #[test]
    fn format_rewrites_javascript_and_typescript_files() {
        let dir = Scratch::new();
        let js = dir.file("a.js", "function add( a , b ) {\n    return a + b;\n}\n");
        let ts = dir.file("b.ts", "interface Shape {\n    name : string ;\n}\n");
        let (code, text) = run_cli(&["format", js.to_str().unwrap(), ts.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&js).unwrap(),
            "function add(a,b){return a+b}"
        );
        assert_eq!(
            std::fs::read_to_string(&ts).unwrap(),
            "interface Shape{name:string;}"
        );
        assert!(text.contains("tokens"));
        assert!(text.contains("2 files"));
    }

    #[test]
    fn every_javascript_extension_including_jsx_and_tsx_is_discovered() {
        let dir = Scratch::new();
        for name in [
            "a.js", "a.mjs", "a.cjs", "a.jsx", "a.ts", "a.mts", "a.cts", "a.tsx",
        ] {
            dir.file(name, "const a = 1;\n");
        }
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("8 files"), "{text}");
        assert!(text.contains("a.jsx"), "{text}");
        assert!(text.contains("a.tsx"), "{text}");
    }

    #[test]
    fn format_rewrites_jsx_and_tsx_files() {
        let dir = Scratch::new();
        let jsx = dir.file(
            "a.jsx",
            "const el = <div a=\"1\" { ...p }>keep  me</div>;\nconst x = 1;\n",
        );
        let tsx = dir.file(
            "b.tsx",
            "const Greet = ( n : string ) => <span>Hi, { n }!</span>;\n",
        );
        let (code, text) = run_cli(&["format", jsx.to_str().unwrap(), tsx.to_str().unwrap()]);
        assert_eq!(code, 0);
        // JSX text keeps its double space; the statement next to it does not.
        assert_eq!(
            std::fs::read_to_string(&jsx).unwrap(),
            "const el=<div a=\"1\"{...p}>keep  me</div>;const x=1;"
        );
        assert_eq!(
            std::fs::read_to_string(&tsx).unwrap(),
            "const Greet=(n:string)=><span>Hi, {n}!</span>;"
        );
        assert!(text.contains("2 files"));
    }

    #[test]
    fn the_js_strip_comments_flag_reaches_jsx_containers() {
        let dir = Scratch::new();
        let jsx = dir.file("a.jsx", "const a = <div>{/* c */}</div>;\n");
        let (code, _) = run_cli(&["format", "--js-strip-comments", jsx.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&jsx).unwrap(),
            "const a=<div>{}</div>;"
        );
    }

    #[test]
    fn invalid_jsx_is_reported_and_nothing_is_written() {
        let dir = Scratch::new();
        let jsx = dir.file("a.jsx", "const a = <div>hi;\n");
        let (code, text) = run_cli(&["format", jsx.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("parse error"), "{text}");
        assert_eq!(
            std::fs::read_to_string(&jsx).unwrap(),
            "const a = <div>hi;\n"
        );
    }

    #[test]
    fn js_strip_comments_flag_is_forwarded() {
        let dir = Scratch::new();
        let js = dir.file("a.js", "// note\nconst a = 1;\n");
        let (code, _) = run_cli(&["format", "--js-strip-comments", js.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&js).unwrap(), "const a=1;");
        // Without the flag the leading comment stays.
        let kept = dir.file("b.js", "// note\nconst a = 1;\n");
        let (code, _) = run_cli(&["format", kept.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "// note\nconst a=1;"
        );
    }

    #[test]
    fn js_caveat_warning_is_emitted_once_per_run() {
        let dir = Scratch::new();
        let a = dir.file("a.js", "// note\nconst a = 1;\n");
        let b = dir.file("b.ts", "const b: number = 2;\n");
        let (code, out, err) = run_cli_err(&["format", a.to_str().unwrap(), b.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("trailing"), "{err}");
        assert!(err.contains("--js-strip-comments"), "{err}");
        assert!(err.contains("@license"), "{err}");
        // stdout stays clean and pipeable.
        assert!(!out.contains("warning:"));
    }

    #[test]
    fn js_caveat_warning_covers_jsx_and_states_the_jsx_caveats() {
        let dir = Scratch::new();
        let jsx = dir.file("a.jsx", "const a = <div>hi</div>;\n");
        let tsx = dir.file("b.tsx", "const b = <p>hi</p>;\n");
        let (code, _, err) = run_cli_err(&["format", jsx.to_str().unwrap(), tsx.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("JSX text"), "{err}");
        assert!(err.contains("{/* c */}"), "{err}");
    }

    #[test]
    fn js_caveat_warning_is_absent_for_python_only_runs() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err, "");
    }

    #[test]
    fn js_caveat_warning_covers_check_and_diff_but_not_stats() {
        let dir = Scratch::new();
        let js = dir.file("a.js", "const a = 1;\n");
        let (code, _, err) = run_cli_err(&["check", js.to_str().unwrap()]);
        assert_eq!(code, 1);
        assert_eq!(err.matches("warning:").count(), 1);
        let (code, _, err) = run_cli_err(&["diff", js.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        let (code, _, err) = run_cli_err(&["stats", js.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err, "");
    }

    #[test]
    fn the_rust_and_js_caveats_are_reported_independently() {
        let dir = Scratch::new();
        let rs = dir.file("a.rs", "fn f() {}\n");
        let js = dir.file("b.js", "const a = 1;\n");
        let (code, _, err) = run_cli_err(&["format", rs.to_str().unwrap(), js.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 2);
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn format_rewrites_ruby_files() {
        let dir = Scratch::new();
        let rb = dir.file(
            "a.rb",
            "def add(a, b)\n    sum  =  a + b\n\n\n    sum\nend\n",
        );
        let gemfile = dir.file(
            "Gemfile",
            "source  \"https://rubygems.org\"\n\ngem  \"rake\"\n",
        );
        let (code, text) = run_cli(&["format", rb.to_str().unwrap(), gemfile.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&rb).unwrap(),
            "def add(a, b)\nsum = a + b\nsum\nend\n"
        );
        assert_eq!(
            std::fs::read_to_string(&gemfile).unwrap(),
            "source \"https://rubygems.org\"\ngem \"rake\"\n"
        );
        assert!(text.contains("tokens"));
        assert!(text.contains("2 files"));
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn every_ruby_path_including_the_extensionless_build_files_is_discovered() {
        let dir = Scratch::new();
        for name in [
            "a.rb",
            "tasks.rake",
            "pkg.gemspec",
            "config.ru",
            "Gemfile",
            "Rakefile",
        ] {
            dir.file(name, "x  =  1\n");
        }
        // A lockfile sits next to them and must not be picked up.
        dir.file("Gemfile.lock", "GEM\n");
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("6 files"), "{text}");
        assert!(text.contains("config.ru"), "{text}");
        assert!(text.contains("Rakefile"), "{text}");
        assert!(!text.contains("Gemfile.lock"), "{text}");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn ruby_strip_comments_flag_is_forwarded() {
        let dir = Scratch::new();
        let rb = dir.file("a.rb", "x  =  1  # trailing\n");
        let (code, _) = run_cli(&["format", "--ruby-strip-comments", rb.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x = 1\n");
        // Without the flag every comment survives, byte for byte.
        let kept = dir.file("b.rb", "x  =  1  # trailing\n");
        let (code, _) = run_cli(&["format", kept.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "x = 1 # trailing\n"
        );
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn ruby_runs_emit_no_caveat_warning() {
        // There is deliberately no Ruby analogue of the Rust and JS/TS caveat
        // warnings: the Ruby emitter rewrites whitespace only and drops
        // nothing at the default settings, so there is nothing to warn about.
        let dir = Scratch::new();
        let rb = dir.file("a.rb", "x  =  1  # trailing\n");
        let path = rb.to_str().unwrap();
        for args in [
            vec!["format", path],
            vec!["check", path],
            vec!["diff", path],
            vec!["stats", path],
        ] {
            let (_, _, err) = run_cli_err(&args);
            assert_eq!(err, "", "{args:?}");
        }
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn external_verify_warning_is_absent_for_ruby_paths() {
        // Ruby's `External` level really runs `ruby -c`, so a Ruby-only run
        // must not be told the level is a no-op — including for the
        // extensionless build files, which no extension check would catch.
        let dir = Scratch::new();
        let rb = dir.file("a.rb", "x  =  1\n");
        let gemfile = dir.file("Gemfile", "gem  \"rake\"\n");
        let (code, out, err) = run_cli_err(&[
            "format",
            "--verify",
            "external",
            rb.to_str().unwrap(),
            gemfile.to_str().unwrap(),
        ]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x = 1\n");
        assert_eq!(std::fs::read_to_string(&gemfile).unwrap(), "gem \"rake\"\n");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn external_verify_runs_the_real_checker_over_ruby() {
        // End to end at the level that spawns `ruby -c`: the output is
        // accepted, written, and stable on a second pass.
        let dir = Scratch::new();
        let rb = dir.file("real.rb", "def add(a, b)\n    a  +  b\nend\n");
        let path = rb.to_str().unwrap();
        let (code, _, _) = run_cli_err(&["format", "--verify", "external", path]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&rb).unwrap(),
            "def add(a, b)\na + b\nend\n"
        );
        let (code, out, _) = run_cli_err(&["check", "--verify", "external", path]);
        assert_eq!(code, 0, "{out}");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn external_verify_does_not_blame_ruby_input_the_checker_already_rejects() {
        // `/[z-a]/` is a well-formed regexp literal to prism and an "empty
        // range in char class" SyntaxError to MRI, which compiles it. The
        // policy is that the external level is then satisfied by the built-in
        // equivalence check alone, so the run succeeds instead of failing on
        // the user's own input.
        let dir = Scratch::new();
        let rb = dir.file("range.rb", "x  =  /[z-a]/\n");
        let (code, out, _) = run_cli_err(&["format", "--verify", "external", rb.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x = /[z-a]/\n");
    }

    #[test]
    fn external_verify_warning_is_emitted_once_per_run() {
        let dir = Scratch::new();
        let a = dir.file("a.py", "x = 1\n");
        let b = dir.file("b.py", "y = 2\n");
        let (code, out, err) = run_cli_err(&[
            "format",
            "--verify",
            "external",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("py_compile"));
        assert!(err.contains("rustc"));
        assert!(err.contains("--verify ast"));
        // ... and says which backend the level *is* implemented for.
        assert!(err.contains("JavaScript/TypeScript"), "{err}");
        // stdout stays clean and pipeable.
        assert!(!out.contains("warning:"));
    }

    #[test]
    fn external_verify_warning_is_scoped_to_the_backends_without_it() {
        // JS/TS really does run external tooling at this level, so a run that
        // touches no Python or Rust file must not be told otherwise. The JS
        // caveat warning is the only one left.
        let dir = Scratch::new();
        let js = dir.file("a.js", "const a = 1;\n");
        let (code, _, err) = run_cli_err(&["format", "--verify", "external", js.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(!err.contains("py_compile"), "{err}");
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("comment-preserving"), "{err}");
        assert_eq!(std::fs::read_to_string(&js).unwrap(), "const a=1;");
    }

    #[test]
    fn external_verify_warning_returns_for_a_mixed_run() {
        // One Python file in the run is enough: the level is a no-op for it.
        let dir = Scratch::new();
        let js = dir.file("b.js", "const a = 1;\n");
        let py = dir.file("b.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--verify",
            "external",
            js.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("py_compile").count(), 1);
    }

    #[test]
    fn external_verify_runs_the_real_checker_over_javascript() {
        // End to end at the level that spawns `tsc`/`node`: the output is
        // accepted, written, and stable on a second pass.
        let dir = Scratch::new();
        let js = dir.file("real.mjs", "export const add = ( a , b ) => a + b;\n");
        let path = js.to_str().unwrap();
        let (code, _, _) = run_cli_err(&["format", "--verify", "external", path]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&js).unwrap(),
            "export const add=(a,b)=>a+b;"
        );
        let (code, out, _) = run_cli_err(&["check", "--verify", "external", path]);
        assert_eq!(code, 0, "{out}");
    }

    #[test]
    fn external_verify_does_not_blame_input_the_checker_already_rejects() {
        // A `.cjs` file using ESM syntax: oxc parses it, `node --check`
        // rejects it as CommonJS. The policy is that the external level is
        // then satisfied by the built-in equivalence check alone, so the run
        // succeeds instead of failing on the user's own input.
        let dir = Scratch::new();
        let cjs = dir.file("legacy.cjs", "export const a = 1;\n");
        let (code, out, _) =
            run_cli_err(&["format", "--verify", "external", cjs.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(std::fs::read_to_string(&cjs).unwrap(), "export const a=1;");
    }

    #[test]
    fn external_verify_warning_is_absent_for_reparse_and_ast() {
        let dir = Scratch::new();
        for level in ["reparse", "ast"] {
            let py = dir.file(&format!("{level}.py"), "x = 1\n");
            let (code, _, err) = run_cli_err(&["format", "--verify", level, py.to_str().unwrap()]);
            assert_eq!(code, 0);
            assert_eq!(err, "");
        }
        // The default level must stay silent too.
        let py = dir.file("default.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err, "");
    }

    #[test]
    fn external_verify_warning_covers_every_subcommand() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let path = py.to_str().unwrap();
        for (args, expected) in [
            (vec!["check", "--verify", "external", path], 1),
            (vec!["diff", "--verify", "external", path], 0),
            (vec!["stats", "--verify", "external", path], 0),
        ] {
            let (code, _, err) = run_cli_err(&args);
            assert_eq!(code, expected);
            assert_eq!(err.matches("warning:").count(), 1);
        }
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

    #[test]
    fn discovery_finds_the_config_file_in_a_parent_directory() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "verify = \"ast\"\n");
        let deep = dir.0.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        assert_eq!(discover_config(&deep), Some(cfg));
    }

    #[test]
    fn discovery_returns_nothing_when_no_config_file_exists() {
        let dir = Scratch::new();
        assert_eq!(discover_config(&dir.0), None);
    }

    #[test]
    fn discovery_prefers_the_nearest_config_file() {
        let dir = Scratch::new();
        dir.file("tokenpress.toml", "verify = \"ast\"\n");
        let sub = dir.0.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let near = sub.join("tokenpress.toml");
        std::fs::write(&near, "verify = \"reparse\"\n").unwrap();
        assert_eq!(discover_config(&sub), Some(near));
    }

    #[test]
    fn an_explicit_config_disables_discovery() {
        let dir = Scratch::new();
        // A `tokenpress.toml` next to the explicit file must be ignored.
        dir.file("tokenpress.toml", "nonsense = true\n");
        let cfg = dir.file("other.toml", "verify = \"ast\"\n");
        let loaded = load_config(Some(&cfg), &dir.0).unwrap().unwrap();
        assert_eq!(loaded.verify, Some(ConfigVerify::Ast));
    }

    #[test]
    fn a_discovered_config_file_is_loaded() {
        let dir = Scratch::new();
        dir.file("tokenpress.toml", "tokenizer = \"cl100k_base\"\n");
        let loaded = load_config(None, &dir.0).unwrap().unwrap();
        assert_eq!(loaded.tokenizer.as_deref(), Some("cl100k_base"));
    }

    #[test]
    fn a_discovered_config_file_that_does_not_parse_is_an_error() {
        let dir = Scratch::new();
        dir.file("tokenpress.toml", "verify = \"strict\"\n");
        let err = load_config(None, &dir.0).unwrap_err();
        assert!(err.to_string().contains("invalid config file"), "{err}");
    }

    #[test]
    fn config_file_supplies_language_tokenizer_and_verify_settings() {
        let dir = Scratch::new();
        let cfg = dir.file(
            "tokenpress.toml",
            "tokenizer = \"cl100k_base\"\n\
             verify = \"reparse\"\n\
             [python]\n\
             strip_comments = true\n\
             strip_docstrings = true\n\
             strip_annotations = true\n\
             [rust]\n\
             strip_doc_comments = true\n",
        );
        let py = dir.file("a.py", "# note\n\"\"\"Doc.\"\"\"\nx: int = 1\n");
        let rs = dir.file("b.rs", "/// doc\nfn f() {}\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
            rs.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x=1");
        assert_eq!(std::fs::read_to_string(&rs).unwrap(), "fn f(){}");
        // `verify = "reparse"` must not trigger the external-verify warning.
        assert!(!err.contains("py_compile"), "{err}");
    }

    #[test]
    fn explicit_cli_flags_win_over_the_config_file() {
        let dir = Scratch::new();
        let cfg = dir.file(
            "tokenpress.toml",
            "tokenizer = \"nope\"\n\
             verify = \"external\"\n\
             [python]\n\
             strip_comments = false\n\
             merge_imports = true\n",
        );
        let py = dir.file("a.py", "# note\nimport os\nimport sys\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            "--tokenizer",
            "cl100k_base",
            "--verify",
            "ast",
            "--py-strip-comments",
            "--py-no-merge-imports",
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&py).unwrap(),
            "import os\nimport sys"
        );
        assert_eq!(err, "");
    }

    #[test]
    fn config_file_supplies_javascript_settings() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[javascript]\nstrip_comments = true\n");
        let js = dir.file("a.js", "// note\nconst a = 1;\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            js.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&js).unwrap(), "const a=1;");
    }

    #[test]
    fn the_js_strip_flag_ors_with_the_config_file() {
        let dir = Scratch::new();
        // Presence-only flags: `false` in the config cannot cancel the flag,
        // and the flag cannot cancel a `true` in the config.
        let off = dir.file("off.toml", "[javascript]\nstrip_comments = false\n");
        let a = dir.file("a.js", "// note\nconst a = 1;\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            off.to_str().unwrap(),
            "--js-strip-comments",
            a.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "const a=1;");

        let on = dir.file("on.toml", "[javascript]\nstrip_comments = true\n");
        let b = dir.file("b.js", "// note\nconst a = 1;\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            on.to_str().unwrap(),
            "--js-strip-comments",
            b.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "const a=1;");
    }

    #[test]
    fn a_javascript_config_table_without_keys_keeps_the_built_in_defaults() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[javascript]\n");
        let js = dir.file("a.js", "// note\nconst a = 1;\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            js.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&js).unwrap(), "// note\nconst a=1;");
    }

    #[test]
    fn unknown_javascript_config_key_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[javascript]\nstrip_comment = true\n");
        let js = dir.file("a.js", "const a = 1;\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            js.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("strip_comment"), "{err}");
        assert_eq!(std::fs::read_to_string(&js).unwrap(), "const a = 1;\n");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn config_file_supplies_ruby_settings() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[ruby]\nstrip_comments = true\n");
        let rb = dir.file("a.rb", "x  =  1  # trailing\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            rb.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x = 1\n");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn the_ruby_strip_flag_ors_with_the_config_file() {
        let dir = Scratch::new();
        // Presence-only flags: `false` in the config cannot cancel the flag,
        // and the flag cannot cancel a `true` in the config.
        let off = dir.file("off.toml", "[ruby]\nstrip_comments = false\n");
        let a = dir.file("a.rb", "x  =  1  # trailing\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            off.to_str().unwrap(),
            "--ruby-strip-comments",
            a.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&a).unwrap(), "x = 1\n");

        let on = dir.file("on.toml", "[ruby]\nstrip_comments = true\n");
        let b = dir.file("b.rb", "x  =  1  # trailing\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            on.to_str().unwrap(),
            "--ruby-strip-comments",
            b.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&b).unwrap(), "x = 1\n");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn a_ruby_config_table_without_keys_keeps_the_built_in_defaults() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[ruby]\n");
        let rb = dir.file("a.rb", "x  =  1  # trailing\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            rb.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x = 1 # trailing\n");
    }

    #[cfg(feature = "ruby")]
    #[test]
    fn unknown_ruby_config_key_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[ruby]\nstrip_embdocs = true\n");
        let rb = dir.file("a.rb", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            rb.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("strip_embdocs"), "{err}");
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x = 1\n");
    }

    #[test]
    fn config_can_disable_import_merging() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[python]\nmerge_imports = false\n");
        let py = dir.file("a.py", "import os\nimport sys\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&py).unwrap(),
            "import os\nimport sys"
        );
    }

    #[test]
    fn config_without_language_tables_keeps_the_built_in_defaults() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "tokenizer = \"cl100k_base\"\n");
        let py = dir.file("a.py", "# note\nimport os\nimport sys\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&py).unwrap(),
            "# note\nimport os,sys"
        );
    }

    #[test]
    fn every_config_verify_level_is_applied() {
        let dir = Scratch::new();
        for (level, warns) in [("reparse", false), ("ast", false), ("external", true)] {
            let cfg = dir.file(&format!("{level}.toml"), &format!("verify = \"{level}\"\n"));
            let py = dir.file(&format!("{level}.py"), "x = 1\n");
            let (code, _, err) = run_cli_err(&[
                "format",
                "--config",
                cfg.to_str().unwrap(),
                py.to_str().unwrap(),
            ]);
            assert_eq!(code, 0);
            assert_eq!(err.contains("py_compile"), warns, "{level}: {err}");
        }
    }

    #[test]
    fn missing_explicit_config_file_is_an_error() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let missing = dir.0.join("nope.toml");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            missing.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("cannot read config file"), "{err}");
        assert!(err.contains("nope.toml"), "{err}");
        // The run stops before any file is touched.
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[test]
    fn malformed_config_file_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[python]\nstrip_comment = true\n");
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("strip_comment"), "{err}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    // What a build without the `ruby` cargo feature does with the three Ruby
    // surfaces: the paths, the flag and the config table.

    #[cfg(not(feature = "ruby"))]
    #[test]
    fn ruby_paths_are_unsupported_without_the_ruby_feature() {
        // No special case: a Ruby path is exactly as unsupported as any other
        // extension the build does not claim — an error when named
        // explicitly, silently skipped by the directory walk.
        let dir = Scratch::new();
        let rb = dir.file("a.rb", "x  =  1\n");
        let (code, text) = run_cli(&["format", rb.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("unsupported language"), "{text}");
        assert_eq!(std::fs::read_to_string(&rb).unwrap(), "x  =  1\n");

        dir.file("Gemfile", "gem  \"rake\"\n");
        let py = dir.file("keep.py", "x = 1\n");
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("keep.py"), "{text}");
        assert!(!text.contains("a.rb"), "{text}");
        assert!(!text.contains("Gemfile"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "ruby"))]
    #[test]
    fn the_ruby_strip_flag_does_not_exist_without_the_ruby_feature() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["format", "--ruby-strip-comments", py.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("--ruby-strip-comments"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "ruby"))]
    #[test]
    fn a_ruby_config_table_names_the_missing_feature_without_the_ruby_feature() {
        // A configured Ruby option must not be silently ignored: the run stops
        // with a message about the build, before any file is touched.
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[ruby]\nstrip_comments = true\n");
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("built without the `ruby` feature"), "{err}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "ruby"))]
    #[test]
    fn the_external_verify_warning_does_not_promise_ruby_without_the_feature() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("JavaScript/TypeScript"), "{err}");
        assert!(!err.contains("ruby -c"), "{err}");
        assert!(err.contains("py_compile"), "{err}");
    }

    #[test]
    fn unknown_config_tokenizer_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "tokenizer = \"nope\"\n");
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(text.contains("unknown tokenizer: nope"), "{text}");
    }
}
