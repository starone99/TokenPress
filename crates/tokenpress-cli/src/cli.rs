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
#[cfg(feature = "csharp")]
use tokenpress_csharp::{CSharpFormatter, CSharpOptions};
#[cfg(feature = "go")]
use tokenpress_go::{GoFormatter, GoOptions};
#[cfg(feature = "java")]
use tokenpress_java::{JavaFormatter, JavaOptions};
use tokenpress_js::{JsFormatter, JsOptions};
use tokenpress_python::{PythonFormatter, PythonOptions};
#[cfg(feature = "ruby")]
use tokenpress_ruby::{RubyFormatter, RubyOptions};
use tokenpress_rust::{RustFormatter, RustOptions};

#[derive(Parser)]
#[command(name = "tokenpress", version, about = about())]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

/// The languages this build actually has a backend for, in dispatch order.
/// The `ruby`, `go`, `java` and `csharp` cargo features are default-on but
/// independent, and a build without one has no such backend at all, so nothing
/// may advertise one. A `Vec` and not an array: the length is a property of the
/// feature set.
fn languages() -> Vec<&'static str> {
    // The first three backends are unconditional, so the list never has fewer
    // than three entries.
    #[cfg_attr(
        not(any(feature = "ruby", feature = "go", feature = "java", feature = "csharp")),
        allow(unused_mut)
    )]
    let mut languages = vec!["Python", "Rust", "JavaScript/TypeScript"];
    #[cfg(feature = "ruby")]
    languages.push("Ruby");
    #[cfg(feature = "go")]
    languages.push("Go");
    #[cfg(feature = "java")]
    languages.push("Java");
    #[cfg(feature = "csharp")]
    languages.push("C#");
    languages
}

/// The `--help` one-liner. Assembled from [`languages`] rather than written
/// out per feature combination: four independent features would otherwise
/// need sixteen spellings of one sentence, and the conjunction moves with the
/// list.
fn about() -> String {
    let languages = languages();
    // Not an edge case: `languages` always has at least three entries.
    let last = languages.len() - 1;
    format!(
        "Token-aware formatter for {} and {}",
        languages[..last].join(", "),
        languages[last]
    )
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum VerifyArg {
    Reparse,
    Ast,
    External,
}

#[derive(Args)]
struct CommonOpts {
    /// Files or directories to process: `.py`, `.rs` and the
    /// JavaScript/TypeScript set `.js` `.mjs` `.cjs` `.jsx` `.ts` `.mts`
    /// `.cts` `.tsx`.
    // Written as `doc` attributes rather than `///` so each optional backend's
    // half can be switched off with the backend itself; they concatenate in
    // source order. One self-contained sentence per backend, so the four
    // optional features stay independent instead of needing a spelling per
    // combination.
    #[cfg_attr(
        feature = "ruby",
        doc = " Also the Ruby set `.rb` `.rake` `.gemspec` `.ru` plus the files"
    )]
    #[cfg_attr(
        feature = "ruby",
        doc = " named `Gemfile` and `Rakefile` (exact, case-sensitive names)."
    )]
    #[cfg_attr(feature = "go", doc = " Also `.go`.")]
    #[cfg_attr(feature = "java", doc = " Also `.java`.")]
    #[cfg_attr(feature = "csharp", doc = " Also `.cs`.")]
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
    /// GOO1: strip Go comments (kept by default — and, unlike Rust and JS/TS,
    /// nothing is dropped without this flag). The comments the Go toolchain
    /// reads as directives — `//go:` lines, `/*line*/`, build constraints and
    /// the cgo preamble — survive either way.
    // Deliberately no mention of the Ruby flag it mirrors: the two features
    // are independent, and a build with `go` but not `ruby` must not advertise
    // a backend it does not have.
    #[cfg(feature = "go")]
    #[arg(long)]
    go_strip_comments: bool,
    /// JVO1: strip Java comments (kept by default — and, unlike Rust and
    /// JS/TS, nothing is dropped without this flag). Javadoc (`/** */`) is an
    /// ordinary comment to the grammar and goes with the rest, so a stripped
    /// file loses its API documentation.
    // Deliberately no mention of the Ruby or Go flags it mirrors: the three
    // features are independent, and a build with `java` but not the others
    // must not advertise a backend it does not have.
    #[cfg(feature = "java")]
    #[arg(long)]
    java_strip_comments: bool,
    /// CSO1: strip C# comments (kept by default — and, unlike Rust and JS/TS,
    /// nothing is dropped without this flag). XML documentation (`///`) is an
    /// ordinary comment to the grammar and goes with the rest, so a stripped
    /// file loses its API documentation.
    // Deliberately no mention of the Ruby, Go or Java flags it mirrors: the
    // four features are independent, and a build with `csharp` but not the
    // others must not advertise a backend it does not have.
    #[cfg(feature = "csharp")]
    #[arg(long)]
    csharp_strip_comments: bool,
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
    #[cfg(feature = "go")]
    if let Some(go) = cfg.go {
        common.go_strip_comments |= go.strip_comments.unwrap_or(false);
    }
    #[cfg(feature = "java")]
    if let Some(java) = cfg.java {
        common.java_strip_comments |= java.strip_comments.unwrap_or(false);
    }
    #[cfg(feature = "csharp")]
    if let Some(csharp) = cfg.csharp {
        common.csharp_strip_comments |= csharp.strip_comments.unwrap_or(false);
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
    // `mut` is only needed when at least one of the four optional backends is
    // compiled in.
    #[cfg_attr(
        not(any(feature = "ruby", feature = "go", feature = "java", feature = "csharp")),
        allow(unused_mut)
    )]
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
    #[cfg(feature = "go")]
    formatters.push(Box::new(GoFormatter::new(GoOptions {
        strip_comments: common.go_strip_comments,
    })));
    #[cfg(feature = "java")]
    formatters.push(Box::new(JavaFormatter::new(JavaOptions {
        strip_comments: common.java_strip_comments,
    })));
    #[cfg(feature = "csharp")]
    formatters.push(Box::new(CSharpFormatter::new(CSharpOptions {
        strip_comments: common.csharp_strip_comments,
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

// There is deliberately no `GO_CAVEAT_WARNING`, no `JAVA_CAVEAT_WARNING` and
// no `CSHARP_CAVEAT_WARNING`,
// for the reason there is no Ruby one: a caveat warning exists where a backend drops something the user
// did not ask it to drop and verification cannot see the loss — Rust's `//`
// comments, JS/TS's trailing and expression-position comments. The Go emitter
// rewrites the whitespace between protected spans and copies everything else
// verbatim, so at the default settings **every** comment survives byte for
// byte; `--go-strip-comments` is the opt-in that deletes them, and an opt-in
// flag documents itself. What the Go backend does unconditionally — pinning an
// indented directive-shaped comment away from column 0, reproducing a
// build-constraint prologue verbatim, leaving a cgo file byte-identical — only
// ever preserves meaning, and none of it is a loss to warn about. The same
// holds for Java, with less to defend: it has no column-sensitive comment
// syntax at all, so its one unconditional rule is the unicode-escape bail-out,
// which leaves a file byte for byte identical rather than dropping anything.
// `--java-strip-comments` does delete Javadoc along with every other comment,
// but that is an opt-in flag documenting itself, not a loss behind the
// caller's back. C# is the same story once more, and its two unconditional
// rules are both preserving rather than lossy: a preprocessor directive keeps
// the newline that makes it one, and a file whose comments the grammar and a
// real compiler could disagree about is returned byte for byte unchanged --
// the first documented class of files this CLI reports as a successful run
// with zero savings rather than as an error. `--csharp-strip-comments` deletes
// XML documentation along with every other comment, an opt-in exactly as
// Java's flag is.

// `--verify external` is real for JavaScript/TypeScript, for Ruby, for Go, for
// Java and now for C#, but still equals `--verify ast` for Python and Rust, so
// a run containing files of those two languages must not read the level as a
// stronger guarantee than it is.
//
// The tail -- which backends do *not* have it -- is a single fixed string
// again. It varied while C# was on it, because `csharp` is a cargo feature and
// a build without the backend has no `.cs` path to name; with C5's `csc` gate
// landed the only two left are Python and Rust, both unconditional backends,
// so there is nothing for it to vary with. `NO_EXTERNAL_VERIFY_EXTENSIONS`
// went back to being unconditional with it.
//
// The head -- which backends *do* have it -- names only backends this binary
// was actually built with: promising Ruby's, Go's, Java's or C#'s checker in a
// build that refuses `.rb`, `.go`, `.java` or `.cs` outright would be a lie
// about this binary. JS/TS is unconditional and always leads; the other four
// are cargo features, which is why the head is a 2x2x2x2 cross-product of
// `ruby`/`go`/`java`/`csharp` and why the grammar of the closing clause
// ("it"/"both"/"each") differs per variant with the number of checkers named.
// C# now multiplies the head instead of the tail -- it changed sides the
// moment it got a checker -- so the head is 16 variants and the tail one: 16
// rendered messages from 17 constants, where the cross-product used to need
// 10. Head and tail are written out as one block by `warn_external_verify`.
#[cfg(all(feature = "ruby", feature = "go", feature = "java", feature = "csharp"))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, for
  Go, where it runs `gofmt -e`, for Java, where it runs `javac`, and for C#,
  where it runs `csc`; each fails if the tool it needs is not on PATH.";

#[cfg(all(
    feature = "ruby",
    feature = "go",
    feature = "java",
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, for
  Go, where it runs `gofmt -e`, and for Java, where it runs `javac`; each
  fails if the tool it needs is not on PATH.";

#[cfg(all(
    feature = "ruby",
    feature = "go",
    not(feature = "java"),
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, for
  Go, where it runs `gofmt -e`, and for C#, where it runs `csc`; each fails
  if the tool it needs is not on PATH.";

#[cfg(all(
    feature = "ruby",
    feature = "go",
    not(feature = "java"),
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, and
  for Go, where it runs `gofmt -e`; each fails if the tool it needs is not on
  PATH.";

#[cfg(all(
    feature = "ruby",
    not(feature = "go"),
    feature = "java",
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, for
  Java, where it runs `javac`, and for C#, where it runs `csc`; each fails if
  the tool it needs is not on PATH.";

#[cfg(all(
    feature = "ruby",
    not(feature = "go"),
    feature = "java",
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, and
  for Java, where it runs `javac`; each fails if the tool it needs is not on
  PATH.";

#[cfg(all(
    feature = "ruby",
    not(feature = "go"),
    not(feature = "java"),
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Ruby, where it runs `ruby -c`, and
  for C#, where it runs `csc`; each fails if the tool it needs is not on
  PATH.";

#[cfg(all(
    feature = "ruby",
    not(feature = "go"),
    not(feature = "java"),
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), and for Ruby, where it runs `ruby -c`;
  both fail if the tool they need is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    feature = "go",
    feature = "java",
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Go, where it runs `gofmt -e`, for
  Java, where it runs `javac`, and for C#, where it runs `csc`; each fails if
  the tool it needs is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    feature = "go",
    feature = "java",
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Go, where it runs `gofmt -e`, and for
  Java, where it runs `javac`; each fails if the tool it needs is not on
  PATH.";

#[cfg(all(
    not(feature = "ruby"),
    feature = "go",
    not(feature = "java"),
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Go, where it runs `gofmt -e`, and for
  C#, where it runs `csc`; each fails if the tool it needs is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    feature = "go",
    not(feature = "java"),
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), and for Go, where it runs `gofmt -e`;
  both fail if the tool they need is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    not(feature = "go"),
    feature = "java",
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), for Java, where it runs `javac`, and for
  C#, where it runs `csc`; each fails if the tool it needs is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    not(feature = "go"),
    feature = "java",
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), and for Java, where it runs `javac`; both
  fail if the tool they need is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    not(feature = "go"),
    not(feature = "java"),
    feature = "csharp"
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`), and for C#, where it runs `csc`; both
  fail if the tool they need is not on PATH.";

#[cfg(all(
    not(feature = "ruby"),
    not(feature = "go"),
    not(feature = "java"),
    not(feature = "csharp")
))]
const EXTERNAL_VERIFY_WARNING_HEAD: &str = "\
warning: external-tooling verification is implemented for
  JavaScript/TypeScript, where `--verify external` runs `tsc --noEmit`
  (falling back to `node --check`); it fails if the tool it needs is not on
  PATH.";

// The tail -- which backends the level does *not* reach. Python and Rust are
// unconditional backends, and every other backend's checker is real, so this
// no longer varies with anything. It opens on its own newline rather than on
// a space: it is written out immediately after a head whose last line differs
// per variant, and joining them mid-line would leave one seam per variant at
// a width nothing in this file controls.
const EXTERNAL_VERIFY_WARNING_TAIL: &str = "
  It is not implemented for Python and Rust: neither `py_compile` nor
  `rustc --emit=metadata` is invoked, so for `.py` and `.rs` this level
  behaves exactly like `--verify ast`, i.e. the output is re-parsed and
  compared for AST / token-stream equivalence.";

/// Extensions the warning above is about: the backends the external level does
/// not reach. Unconditional, because both of them are.
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
        #[cfg(feature = "go")]
        {
            assert!(text.contains(".go"), "'.go' missing from help:\n{text}");
            assert!(text.contains("--go-strip-comments"), "{text}");
        }
        // ... and the same for the `go` feature, which is independent of it.
        #[cfg(not(feature = "go"))]
        for absent in [".go", "Go"] {
            assert!(!text.contains(absent), "{absent} in help:\n{text}");
        }
        #[cfg(feature = "java")]
        {
            assert!(text.contains(".java"), "'.java' missing from help:\n{text}");
            assert!(text.contains("--java-strip-comments"), "{text}");
        }
        // ... and the same again for the `java` feature. The absent list is
        // the extension and the flag rather than the bare word "Java": the
        // paths blurb always names the unconditional JavaScript/TypeScript
        // backend, and "Java" is a prefix of "JavaScript".
        #[cfg(not(feature = "java"))]
        for absent in [".java", "--java-strip-comments"] {
            assert!(!text.contains(absent), "{absent} in help:\n{text}");
        }
        #[cfg(feature = "csharp")]
        {
            assert!(text.contains(".cs"), "'.cs' missing from help:\n{text}");
            assert!(text.contains("--csharp-strip-comments"), "{text}");
        }
        // ... and the same again for the `csharp` feature. `.cs` can be
        // asserted directly, unlike Java's `.java`: the unconditional JS/TS
        // blurb names `.cjs` and `.cts`, and neither has `.cs` as a substring.
        #[cfg(not(feature = "csharp"))]
        for absent in [".cs", "--csharp-strip-comments"] {
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

    #[cfg(feature = "go")]
    #[test]
    fn format_rewrites_go_files() {
        let dir = Scratch::new();
        let go = dir.file(
            "a.go",
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n",
        );
        let (code, text) = run_cli(&["format", go.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&go).unwrap(),
            "package main\nimport \"fmt\"\nfunc main() {\nfmt.Println(\"hi\")\n}\n"
        );
        assert!(text.contains("tokens"));
    }

    #[cfg(feature = "go")]
    #[test]
    fn go_files_are_discovered_by_the_walk_and_module_metadata_is_not() {
        // `.go` is the whole path set: `go.mod` and `go.sum` sit next to the
        // sources in every Go repository and are not Go source.
        let dir = Scratch::new();
        dir.file("a.go", "package  main\n");
        dir.file("go.mod", "module example.com/m\n");
        dir.file("go.sum", "\n");
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("a.go"), "{text}");
        assert!(!text.contains("go.mod"), "{text}");
        assert!(!text.contains("go.sum"), "{text}");
    }

    #[cfg(feature = "go")]
    #[test]
    fn go_strip_comments_flag_is_forwarded() {
        let dir = Scratch::new();
        let go = dir.file("a.go", "package main\n\n// note\nfunc f() {}\n");
        let (code, _) = run_cli(&["format", "--go-strip-comments", go.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&go).unwrap(),
            "package main\nfunc f() {}\n"
        );
        // Without the flag every comment survives, byte for byte.
        let kept = dir.file("b.go", "package main\n\n// note\nfunc f() {}\n");
        let (code, _) = run_cli(&["format", kept.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "package main\n// note\nfunc f() {}\n"
        );
    }

    #[cfg(feature = "go")]
    #[test]
    fn go_runs_emit_no_caveat_warning() {
        // Like Ruby, and unlike Rust and JS/TS, there is deliberately no Go
        // caveat warning: the Go emitter rewrites whitespace only and drops
        // nothing at the default settings, so there is nothing to warn about.
        let dir = Scratch::new();
        let go = dir.file("a.go", "package main\n\n// note\nfunc f() {}\n");
        let path = go.to_str().unwrap();
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

    #[cfg(feature = "go")]
    #[test]
    fn external_verify_warning_names_go_as_implemented() {
        // Go's `External` level really runs `gofmt -e`, so the warning has to
        // name it on the *implemented* side and must no longer list `.go`
        // among the extensions the level does not reach.
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("gofmt -e"), "{err}");
        assert!(!err.contains("`.go`"), "{err}");
        assert!(err.contains("py_compile"), "{err}");
    }

    #[cfg(feature = "go")]
    #[test]
    fn external_verify_warning_is_absent_for_go_paths() {
        // A Go-only run must not be told the level is a no-op for it.
        let dir = Scratch::new();
        let go = dir.file("a.go", "package  main\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", go.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(std::fs::read_to_string(&go).unwrap(), "package main\n");
    }

    #[cfg(feature = "go")]
    #[test]
    fn external_verify_runs_the_real_checker_over_go() {
        // End to end at the level that spawns `gofmt -e`: the output is
        // accepted, written, and stable on a second pass.
        let dir = Scratch::new();
        let go = dir.file(
            "real.go",
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n",
        );
        let path = go.to_str().unwrap();
        let (code, _, _) = run_cli_err(&["format", "--verify", "external", path]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&go).unwrap(),
            "package main\nimport \"fmt\"\nfunc main() {\nfmt.Println(\"hi\")\n}\n"
        );
        let (code, out, _) = run_cli_err(&["check", "--verify", "external", path]);
        assert_eq!(code, 0, "{out}");
    }

    #[cfg(feature = "go")]
    #[test]
    fn external_verify_does_not_blame_go_input_the_checker_already_rejects() {
        // A source file carrying only comments is a clean tree-sitter parse
        // and an "expected 'package'" error to `go/parser`, which is what
        // `gofmt -e` runs. The policy is that the external level is then
        // satisfied by the built-in equivalence check alone, so the run
        // succeeds instead of failing on the user's own input.
        let dir = Scratch::new();
        let go = dir.file("comments.go", "\n\n// note\n\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", go.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(std::fs::read_to_string(&go).unwrap(), "// note\n");
    }

    #[cfg(feature = "java")]
    #[test]
    fn format_rewrites_java_files() {
        let dir = Scratch::new();
        let java = dir.file(
            "A.java",
            "public class A {\n\n    void f() {\n        int x = 1;\n    }\n}\n",
        );
        let (code, text) = run_cli(&["format", java.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class A {\nvoid f() {\nint x = 1;\n}\n}\n"
        );
        assert!(text.contains("tokens"));
    }

    #[cfg(feature = "java")]
    #[test]
    fn java_files_are_discovered_by_the_walk() {
        // `.java` is the whole path set. A Java project's build descriptors
        // sit next to the sources and are not Java source: `pom.xml` and
        // `build.gradle` must be walked past, not rewritten.
        let dir = Scratch::new();
        dir.file("A.java", "public  class  A  {}\n");
        let pom = dir.file("pom.xml", "<project>  </project>\n");
        let gradle = dir.file("build.gradle", "plugins  {  id  'java'  }\n");
        let (code, text) = run_cli(&["format", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("A.java"), "{text}");
        assert!(!text.contains("pom.xml"), "{text}");
        assert!(!text.contains("build.gradle"), "{text}");
        assert_eq!(
            std::fs::read_to_string(dir.0.join("A.java")).unwrap(),
            "public class A {}\n"
        );
        // Untouched byte for byte, not merely unreported.
        assert_eq!(
            std::fs::read_to_string(&pom).unwrap(),
            "<project>  </project>\n"
        );
        assert_eq!(
            std::fs::read_to_string(&gradle).unwrap(),
            "plugins  {  id  'java'  }\n"
        );
    }

    #[cfg(feature = "java")]
    #[test]
    fn java_strip_comments_flag_is_forwarded() {
        let dir = Scratch::new();
        let java = dir.file(
            "A.java",
            "public class A {\n\n    // note\n    void f() {}\n}\n",
        );
        let (code, _) = run_cli(&["format", "--java-strip-comments", java.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class A {\nvoid f() {}\n}\n"
        );
        // Without the flag every comment survives, byte for byte — Javadoc
        // included, which is what the flag's opt-in status is about.
        let kept = dir.file(
            "B.java",
            "/** Doc. */\npublic class B {\n\n    // note\n    void f() {}\n}\n",
        );
        let (code, _) = run_cli(&["format", kept.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "/** Doc. */\npublic class B {\n// note\nvoid f() {}\n}\n"
        );
    }

    #[cfg(feature = "java")]
    #[test]
    fn java_runs_emit_no_caveat_warning() {
        // Like Ruby and Go, and unlike Rust and JS/TS, there is deliberately
        // no Java caveat warning: every comment survives byte for byte at the
        // default settings, and what the backend does unconditionally only
        // ever preserves meaning, so there is nothing to warn about.
        let dir = Scratch::new();
        let java = dir.file(
            "A.java",
            "/** Doc. */\npublic class A {\n\n    // note\n    void f() {}\n}\n",
        );
        let path = java.to_str().unwrap();
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

    #[cfg(feature = "java")]
    #[test]
    fn external_verify_warning_names_java_as_implemented() {
        // Java's `External` level really runs javac's parse gate, so the
        // warning has to name it on the *implemented* side and must no longer
        // list `.java` among the extensions the level does not reach.
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("javac"), "{err}");
        assert!(!err.contains("`.java`"), "{err}");
        assert!(err.contains("py_compile"), "{err}");
    }

    #[cfg(feature = "java")]
    #[test]
    fn external_verify_warning_is_absent_for_java_paths() {
        // A Java-only run must not be told the level is a no-op for it.
        let dir = Scratch::new();
        let java = dir.file("A.java", "public  class  A  {}\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", java.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class A {}\n"
        );
    }

    #[cfg(feature = "java")]
    #[test]
    fn external_verify_runs_the_real_checker_over_java() {
        // End to end at the level that spawns `javac`: the output is accepted,
        // written, and stable on a second pass. The file is named after
        // neither of the fixed scratch names the gate writes under, which is
        // part of what makes it an end-to-end check.
        let dir = Scratch::new();
        let java = dir.file(
            "Real.java",
            "public class Real {\n\n    int f(int a) {\n        return a;\n    }\n}\n",
        );
        let path = java.to_str().unwrap();
        let (code, _, _) = run_cli_err(&["format", "--verify", "external", path]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class Real {\nint f(int a) {\nreturn a;\n}\n}\n"
        );
        let (code, out, _) = run_cli_err(&["check", "--verify", "external", path]);
        assert_eq!(code, 0, "{out}");
    }

    #[cfg(feature = "java")]
    #[test]
    fn external_verify_does_not_blame_java_input_the_checker_already_rejects() {
        // `99999999999` with no `L` suffix is a clean tree-sitter parse and
        // `error: integer number too large` to javac. The policy is that the
        // external level is then satisfied by the built-in equivalence check
        // alone, so the run succeeds instead of failing on the user's own
        // input.
        let dir = Scratch::new();
        let java = dir.file("Big.java", "class Big {\n    long  x  =  99999999999;\n}\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", java.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "class Big {\nlong x = 99999999999;\n}\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn format_rewrites_csharp_files() {
        let dir = Scratch::new();
        let cs = dir.file(
            "A.cs",
            "public class A\n{\n\n    void F()\n    {\n        int x = 1;\n    }\n}\n",
        );
        let (code, text) = run_cli(&["format", cs.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "public class A\n{\nvoid F()\n{\nint x = 1;\n}\n}\n"
        );
        assert!(text.contains("tokens"));
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn csharp_files_are_discovered_by_the_walk() {
        // `.cs` is the whole path set. A C# project's build descriptors sit
        // next to the sources and are not C# source: `.csproj` and `.sln` must
        // be walked past, not rewritten.
        let dir = Scratch::new();
        dir.file("A.cs", "public  class  A  {}\n");
        let csproj = dir.file("App.csproj", "<Project>  </Project>\n");
        let sln = dir.file("App.sln", "Microsoft Visual Studio Solution  File\n");
        let (code, text) = run_cli(&["format", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("A.cs"), "{text}");
        assert!(!text.contains("App.csproj"), "{text}");
        assert!(!text.contains("App.sln"), "{text}");
        assert_eq!(
            std::fs::read_to_string(dir.0.join("A.cs")).unwrap(),
            "public class A {}\n"
        );
        // Untouched byte for byte, not merely unreported.
        assert_eq!(
            std::fs::read_to_string(&csproj).unwrap(),
            "<Project>  </Project>\n"
        );
        assert_eq!(
            std::fs::read_to_string(&sln).unwrap(),
            "Microsoft Visual Studio Solution  File\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn csharp_runs_emit_no_caveat_warning() {
        // Like Ruby, Go and Java, and unlike Rust and JS/TS, there is
        // deliberately no C# caveat warning: every comment survives byte for
        // byte at the default settings — XML documentation included — and what
        // the backend does unconditionally only ever preserves meaning, so
        // there is nothing to warn about.
        let dir = Scratch::new();
        let cs = dir.file(
            "A.cs",
            "/// <summary>Doc.</summary>\npublic class A\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let path = cs.to_str().unwrap();
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

    #[cfg(feature = "csharp")]
    #[test]
    fn csharp_strip_comments_flag_is_forwarded() {
        let dir = Scratch::new();
        let cs = dir.file(
            "A.cs",
            "public class A\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let (code, _) = run_cli(&["format", "--csharp-strip-comments", cs.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "public class A\n{\nvoid F() {}\n}\n"
        );
        // Without the flag every comment survives, byte for byte — XML
        // documentation included, which is what the flag's opt-in status is
        // about.
        let kept = dir.file(
            "B.cs",
            "/// <summary>Doc.</summary>\npublic class B\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let (code, _) = run_cli(&["format", kept.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&kept).unwrap(),
            "/// <summary>Doc.</summary>\npublic class B\n{\n// note\nvoid F() {}\n}\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn a_csharp_file_the_boundary_bail_out_claims_is_a_success_with_zero_savings() {
        // C# is the first backend with a documented class of files returned
        // unchanged: a comment whose end the grammar and a real compiler could
        // disagree about makes the whole file byte-identical. Through the CLI
        // that has to be an ordinary successful run reporting no savings, not
        // an error and not a skipped file — `check` agrees there is nothing to
        // reformat, and `format` leaves the bytes alone.
        let dir = Scratch::new();
        let source = "class A\n{\n    /* spans\n#if DEBUG\n    */\n    int  x  =  1;\n}\n";
        let cs = dir.file("A.cs", source);
        let path = cs.to_str().unwrap();
        let (code, text) = run_cli(&["stats", path]);
        assert_eq!(code, 0);
        assert!(text.contains("(-0.0%)"), "{text}");
        let (code, text) = run_cli(&["check", path]);
        assert_eq!(code, 0);
        assert!(text.contains("0 of 1 files would change"), "{text}");
        let (code, _) = run_cli(&["format", path]);
        assert_eq!(code, 0);
        assert_eq!(std::fs::read_to_string(&cs).unwrap(), source);
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn external_verify_warning_names_csharp_as_implemented() {
        // C#'s `External` level really runs Roslyn's `csc` and compares the
        // diagnostic multisets, so the warning has to name it on the
        // *implemented* side and must no longer list `.cs` among the
        // extensions the level does not reach.
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("csc"), "{err}");
        assert!(!err.contains("`.cs`"), "{err}");
        assert!(err.contains("py_compile"), "{err}");
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn external_verify_warning_is_absent_for_csharp_paths() {
        // A C#-only run must not be told the level is a no-op for it.
        let dir = Scratch::new();
        let cs = dir.file("A.cs", "public  class  A  {}\n");
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", cs.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(std::fs::read_to_string(&cs).unwrap(), "public class A {}\n");
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn external_verify_runs_the_real_checker_over_csharp() {
        // End to end at the level that spawns `csc`: the output is accepted,
        // written, and stable on a second pass. The file is named after none
        // of the fixed scratch names the gate writes under, which is part of
        // what makes it an end-to-end check.
        let dir = Scratch::new();
        let cs = dir.file(
            "Real.cs",
            "public class Real\n{\n\n    int F(int a)\n    {\n        return a;\n    }\n}\n",
        );
        let path = cs.to_str().unwrap();
        let (code, _, _) = run_cli_err(&["format", "--verify", "external", path]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "public class Real\n{\nint F(int a)\n{\nreturn a;\n}\n}\n"
        );
        let (code, out, _) = run_cli_err(&["check", "--verify", "external", path]);
        assert_eq!(code, 0, "{out}");
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn external_verify_does_not_blame_csharp_input_the_checker_already_rejects() {
        // `99999999999999999999999` is a clean tree-sitter parse and
        // `error CS1021: Integral constant is too large` to `csc`. The
        // diagnostic multiset design is what makes that a non-event: the same
        // complaint appears on both sides and cancels, so the run succeeds
        // instead of failing on the user's own input.
        let dir = Scratch::new();
        let cs = dir.file(
            "Big.cs",
            "class Big\n{\n    long  x  =  99999999999999999999999;\n}\n",
        );
        let (code, out, err) =
            run_cli_err(&["format", "--verify", "external", cs.to_str().unwrap()]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(err, "");
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "class Big\n{\nlong x = 99999999999999999999999;\n}\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn config_file_supplies_csharp_settings() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[csharp]\nstrip_comments = true\n");
        let cs = dir.file(
            "A.cs",
            "public class A\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            cs.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "public class A\n{\nvoid F() {}\n}\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn the_csharp_strip_flag_ors_with_the_config_file() {
        let dir = Scratch::new();
        // Presence-only flags: `false` in the config cannot cancel the flag,
        // and the flag cannot cancel a `true` in the config.
        let off = dir.file("off.toml", "[csharp]\nstrip_comments = false\n");
        let a = dir.file(
            "A.cs",
            "public class A\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            off.to_str().unwrap(),
            "--csharp-strip-comments",
            a.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&a).unwrap(),
            "public class A\n{\nvoid F() {}\n}\n"
        );

        let on = dir.file("on.toml", "[csharp]\nstrip_comments = true\n");
        let b = dir.file(
            "B.cs",
            "public class B\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            on.to_str().unwrap(),
            "--csharp-strip-comments",
            b.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&b).unwrap(),
            "public class B\n{\nvoid F() {}\n}\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn a_csharp_config_table_without_keys_keeps_the_built_in_defaults() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[csharp]\n");
        let cs = dir.file(
            "A.cs",
            "public class A\n{\n\n    // note\n    void F() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            cs.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "public class A\n{\n// note\nvoid F() {}\n}\n"
        );
    }

    #[cfg(feature = "csharp")]
    #[test]
    fn unknown_csharp_config_key_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[csharp]\nstrip_xml_doc = true\n");
        let cs = dir.file("A.cs", "public class A {}\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            cs.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("strip_xml_doc"), "{err}");
        assert_eq!(std::fs::read_to_string(&cs).unwrap(), "public class A {}\n");
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

    #[cfg(feature = "go")]
    #[test]
    fn config_file_supplies_go_settings() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[go]\nstrip_comments = true\n");
        let go = dir.file("a.go", "package main\n\n// note\nfunc f() {}\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            go.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&go).unwrap(),
            "package main\nfunc f() {}\n"
        );
    }

    #[cfg(feature = "go")]
    #[test]
    fn the_go_strip_flag_ors_with_the_config_file() {
        let dir = Scratch::new();
        // Presence-only flags: `false` in the config cannot cancel the flag,
        // and the flag cannot cancel a `true` in the config.
        let off = dir.file("off.toml", "[go]\nstrip_comments = false\n");
        let a = dir.file("a.go", "package main\n\n// note\nfunc f() {}\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            off.to_str().unwrap(),
            "--go-strip-comments",
            a.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&a).unwrap(),
            "package main\nfunc f() {}\n"
        );

        let on = dir.file("on.toml", "[go]\nstrip_comments = true\n");
        let b = dir.file("b.go", "package main\n\n// note\nfunc f() {}\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            on.to_str().unwrap(),
            "--go-strip-comments",
            b.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&b).unwrap(),
            "package main\nfunc f() {}\n"
        );
    }

    #[cfg(feature = "go")]
    #[test]
    fn a_go_config_table_without_keys_keeps_the_built_in_defaults() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[go]\n");
        let go = dir.file("a.go", "package main\n\n// note\nfunc f() {}\n");
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            go.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&go).unwrap(),
            "package main\n// note\nfunc f() {}\n"
        );
    }

    #[cfg(feature = "go")]
    #[test]
    fn unknown_go_config_key_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[go]\nstrip_directives = true\n");
        let go = dir.file("a.go", "package main\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            go.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("strip_directives"), "{err}");
        assert_eq!(std::fs::read_to_string(&go).unwrap(), "package main\n");
    }

    #[cfg(feature = "java")]
    #[test]
    fn config_file_supplies_java_settings() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[java]\nstrip_comments = true\n");
        let java = dir.file(
            "A.java",
            "public class A {\n\n    // note\n    void f() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            java.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class A {\nvoid f() {}\n}\n"
        );
    }

    #[cfg(feature = "java")]
    #[test]
    fn the_java_strip_flag_ors_with_the_config_file() {
        let dir = Scratch::new();
        // Presence-only flags: `false` in the config cannot cancel the flag,
        // and the flag cannot cancel a `true` in the config.
        let off = dir.file("off.toml", "[java]\nstrip_comments = false\n");
        let a = dir.file(
            "A.java",
            "public class A {\n\n    // note\n    void f() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            off.to_str().unwrap(),
            "--java-strip-comments",
            a.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&a).unwrap(),
            "public class A {\nvoid f() {}\n}\n"
        );

        let on = dir.file("on.toml", "[java]\nstrip_comments = true\n");
        let b = dir.file(
            "B.java",
            "public class B {\n\n    // note\n    void f() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            on.to_str().unwrap(),
            "--java-strip-comments",
            b.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&b).unwrap(),
            "public class B {\nvoid f() {}\n}\n"
        );
    }

    #[cfg(feature = "java")]
    #[test]
    fn a_java_config_table_without_keys_keeps_the_built_in_defaults() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[java]\n");
        let java = dir.file(
            "A.java",
            "public class A {\n\n    // note\n    void f() {}\n}\n",
        );
        let (code, _) = run_cli(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            java.to_str().unwrap(),
        ]);
        assert_eq!(code, 0);
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class A {\n// note\nvoid f() {}\n}\n"
        );
    }

    #[cfg(feature = "java")]
    #[test]
    fn unknown_java_config_key_is_an_error() {
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[java]\nstrip_javadoc = true\n");
        let java = dir.file("A.java", "public class A {}\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            java.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("strip_javadoc"), "{err}");
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public class A {}\n"
        );
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

    // The same three surfaces for a build without the `go` cargo feature.

    #[cfg(not(feature = "go"))]
    #[test]
    fn go_paths_are_unsupported_without_the_go_feature() {
        let dir = Scratch::new();
        let go = dir.file("a.go", "package  main\n");
        let (code, text) = run_cli(&["format", go.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("unsupported language"), "{text}");
        assert_eq!(std::fs::read_to_string(&go).unwrap(), "package  main\n");

        let py = dir.file("keep.py", "x = 1\n");
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("keep.py"), "{text}");
        assert!(!text.contains("a.go"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "go"))]
    #[test]
    fn the_go_strip_flag_does_not_exist_without_the_go_feature() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["format", "--go-strip-comments", py.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("--go-strip-comments"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "go"))]
    #[test]
    fn a_go_config_table_names_the_missing_feature_without_the_go_feature() {
        // A configured Go option must not be silently ignored: the run stops
        // with a message about the build, before any file is touched.
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[go]\nstrip_comments = true\n");
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("built without the `go` feature"), "{err}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "go"))]
    #[test]
    fn the_external_verify_warning_does_not_promise_go_without_the_feature() {
        // A build that cannot format Go at all must not advertise Go's
        // external checker, exactly as it must not advertise Ruby's.
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("py_compile"), "{err}");
        assert!(!err.contains("gofmt"), "{err}");
    }

    // The same four surfaces for a build without the `java` cargo feature.

    #[cfg(not(feature = "java"))]
    #[test]
    fn java_paths_are_unsupported_without_the_java_feature() {
        let dir = Scratch::new();
        let java = dir.file("A.java", "public  class  A  {}\n");
        let (code, text) = run_cli(&["format", java.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("unsupported language"), "{text}");
        assert_eq!(
            std::fs::read_to_string(&java).unwrap(),
            "public  class  A  {}\n"
        );

        let py = dir.file("keep.py", "x = 1\n");
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("keep.py"), "{text}");
        assert!(!text.contains("A.java"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "java"))]
    #[test]
    fn the_java_strip_flag_does_not_exist_without_the_java_feature() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["format", "--java-strip-comments", py.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("--java-strip-comments"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "java"))]
    #[test]
    fn a_java_config_table_names_the_missing_feature_without_the_java_feature() {
        // A configured Java option must not be silently ignored: the run stops
        // with a message about the build, before any file is touched.
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[java]\nstrip_comments = true\n");
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("built without the `java` feature"), "{err}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "java"))]
    #[test]
    fn the_external_verify_warning_does_not_mention_java_without_the_feature() {
        // A build that cannot format Java at all must not advertise Java's
        // external checker, exactly as it must not advertise Ruby's or Go's:
        // there is no `.java` path for it to reach.
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("py_compile"), "{err}");
        assert!(!err.contains("javac"), "{err}");
    }

    // The same four surfaces for a build without the `csharp` cargo feature.

    #[cfg(not(feature = "csharp"))]
    #[test]
    fn csharp_paths_are_unsupported_without_the_csharp_feature() {
        let dir = Scratch::new();
        let cs = dir.file("A.cs", "public  class  A  {}\n");
        let (code, text) = run_cli(&["format", cs.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("unsupported language"), "{text}");
        assert_eq!(
            std::fs::read_to_string(&cs).unwrap(),
            "public  class  A  {}\n"
        );

        let py = dir.file("keep.py", "x = 1\n");
        let (code, text) = run_cli(&["stats", dir.0.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert!(text.contains("keep.py"), "{text}");
        assert!(!text.contains("A.cs"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "csharp"))]
    #[test]
    fn the_csharp_strip_flag_does_not_exist_without_the_csharp_feature() {
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, text) = run_cli(&["format", "--csharp-strip-comments", py.to_str().unwrap()]);
        assert_eq!(code, 2);
        assert!(text.contains("--csharp-strip-comments"), "{text}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "csharp"))]
    #[test]
    fn a_csharp_config_table_names_the_missing_feature_without_the_csharp_feature() {
        // A configured C# option must not be silently ignored: the run stops
        // with a message about the build, before any file is touched.
        let dir = Scratch::new();
        let cfg = dir.file("tokenpress.toml", "[csharp]\nstrip_comments = true\n");
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&[
            "format",
            "--config",
            cfg.to_str().unwrap(),
            py.to_str().unwrap(),
        ]);
        assert_eq!(code, 2);
        assert!(err.contains("invalid config file"), "{err}");
        assert!(err.contains("built without the `csharp` feature"), "{err}");
        assert_eq!(std::fs::read_to_string(&py).unwrap(), "x = 1\n");
    }

    #[cfg(not(feature = "csharp"))]
    #[test]
    fn the_external_verify_warning_does_not_mention_csharp_without_the_feature() {
        // A build that cannot format C# at all must not advertise C#'s
        // external checker, exactly as it must not advertise Ruby's, Go's or
        // Java's: there is no `.cs` path for it to reach.
        let dir = Scratch::new();
        let py = dir.file("a.py", "x = 1\n");
        let (code, _, err) = run_cli_err(&["format", "--verify", "external", py.to_str().unwrap()]);
        assert_eq!(code, 0);
        assert_eq!(err.matches("warning:").count(), 1);
        assert!(err.contains("py_compile"), "{err}");
        assert!(!err.contains("csc"), "{err}");
        assert!(!err.contains("C#"), "{err}");
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
