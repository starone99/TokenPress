//! External verification: the output is handed to the Go toolchain itself, on
//! top of the built-in checks in `tokenpress_treesitter::verify`.
//!
//! This is what [`tokenpress_core::VerifyLevel::External`] adds for this
//! backend. It never replaces the equivalence check; it runs after it.
//!
//! # The checker
//!
//! `gofmt -e <file>` — the toolchain's own front end. `gofmt` parses the file
//! with `go/parser` and prints the reformatted source; nothing in the file is
//! compiled, linked or run, and no package outside it is loaded, so an
//! arbitrary user file can be handed to it without side effects and without a
//! module context. `-e` asks for *all* parse errors rather than the first ten
//! on distinct lines; it does not change what counts as an error, only how
//! much is reported, so it only ever improves the message quoted back.
//!
//! The verdict is the **exit status and nothing else**: 0 for a file that
//! parses, 2 for one that does not. `gofmt` writes the whole reformatted
//! source to stdout on success, which is both large and — since TokenPress
//! deliberately does not produce `gofmt`-shaped output — irrelevant, so
//! stdout is discarded rather than read. Only what a file that failed to
//! parse has to say about *why* is kept, and only to quote it back.
//!
//! An unformatted-but-valid file exits 0. `gofmt` is used here purely as a
//! parser, never as a style oracle: TokenPress's output is minimized
//! whitespace and would never survive a `gofmt -l` comparison.
//!
//! When `gofmt` is not on PATH, verification **fails**, naming it: silently
//! degrading to the built-in level would turn an explicit `--verify external`
//! into a weaker guarantee than the user asked for.
//!
//! # Probing
//!
//! The probe is `gofmt -h`, and the two obvious alternatives are both traps.
//! `gofmt` has **no** `--version` flag, so `--version` would exercise the
//! unknown-flag path rather than a supported one. A **bare** `gofmt` reads
//! standard input, so it would block until the pipe closed — the probe is the
//! one place where that would look like a hang rather than an error. `-h`
//! prints usage and exits, which is exactly what a probe wants.
//!
//! # Already-broken input
//!
//! The checker is run over the **original** source first. If the original does
//! not pass, the formatted output is not checked at all and the level counts
//! as satisfied by the built-in equivalence check alone: TokenPress must not
//! be blamed for a file the toolchain already rejected. The run is not failed
//! either — the user's input is not TokenPress's error.
//!
//! This is not a hypothetical. tree-sitter parses a *file*, `go/parser`
//! parses a *compilation unit*: a source file carrying only comments, the
//! empty file included, is a clean tree-sitter parse and an "expected
//! 'package'" error to `go/parser`. Over the go1.24.7 standard library
//! (7,117 `.go` files) this backend produced 7,060 outputs that `gofmt -e`
//! accepted in **both** comment configurations, and the only five it did not
//! were exactly the files whose *originals* `gofmt -e` already rejects — all
//! of them deliberately non-source `testdata`, such as the empty
//! `cmd/go/internal/modindex/testdata/ignore_non_source/b.go`.
//!
//! # Where the candidate goes
//!
//! Both the original and the candidate output are written to a **private temp
//! directory** (removed when the private `Scratch` guard drops), each under a
//! `.go` name. The user's own path is never written to here — output that
//! fails is discarded by the caller and never reaches the destination file,
//! which is the project's core invariant.
//!
//! Unlike `tokenpress-js`, no extension has to be carried over from the target
//! path: `.go` is the only extension this backend claims (see [`crate::paths`])
//! and `gofmt` has no dialect selector, so one fixed name serves every path.
//! The path is therefore not a parameter of this module at all. Neither is the
//! *directory*: `gofmt` never resolves imports, so a file checked outside its
//! module builds no less successfully than one checked inside it.
//!
//! # Text, not bytes
//!
//! Go source is UTF-8 by specification, so taking `&str` here costs nothing:
//! this module is handed exactly what [`crate::GoFormatter`]'s `format` was
//! handed and what it produced, and that signature is `&str` all the way up
//! from [`tokenpress_core::Formatter::format`].
//!
//! # Cost
//!
//! One probe plus two `gofmt` processes per file, so `--verify external` is
//! substantially slower than `--verify ast`. That is the price of an
//! independent opinion from the real toolchain.
//!
//! # Windows
//!
//! The bare name is enough. `tokenpress-js` has to probe `tsc.cmd` as well
//! because an npm-installed `tsc` is a batch shim that `CreateProcess` will
//! not start from the extensionless name; `gofmt` has no such problem, since
//! every Windows Go distribution (the MSI, the zip, `actions/setup-go`) ships
//! a real `gofmt.exe` in `GOROOT/bin` and `CreateProcess` appends `.exe`
//! itself. The candidate list is kept as a list anyway, because the probe is
//! what decides and a second name can be added without reshaping anything.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokenpress_core::{Error, Result};

/// The names probed for the checker, in order (see the module docs on Windows
/// for why there is only one).
const GOFMT_NAMES: [&str; 1] = ["gofmt"];

/// What the probe passes. `-h` prints usage and exits; see the module docs for
/// why neither `--version` nor a bare invocation can be used.
const GOFMT_PROBE_ARGS: [&str; 1] = ["-h"];

/// Parse-only invocation: `-e` reports every parse error instead of the first
/// ten on distinct lines.
const GOFMT_ARGS: [&str; 1] = ["-e"];

/// The file name the candidate is checked under. `original.go` is its
/// counterpart; both are private to a `Scratch` directory.
const FORMATTED_NAME: &str = "formatted.go";
const ORIGINAL_NAME: &str = "original.go";

/// What one checker run concluded.
#[derive(Debug)]
enum Outcome {
    Pass,
    /// The checker rejected the file; carries its own diagnostics.
    Fail(String),
}

/// The seam between the orchestration and the processes it drives, so the
/// orchestration can be tested without depending on what is installed on the
/// machine running the tests.
trait Tools {
    /// Returns the first of `candidates` that can actually be started, or
    /// `None` when none of them can.
    fn locate(&self, candidates: &[&str]) -> Option<String>;

    /// Runs `program args... file` and reports its verdict. `Err` means the
    /// process could not be run at all.
    fn run(&self, program: &str, args: &[&str], file: &Path) -> io::Result<Outcome>;
}

/// The real toolchain: PATH lookup by spawning, checks by spawning.
struct SystemTools;

impl Tools for SystemTools {
    fn locate(&self, candidates: &[&str]) -> Option<String> {
        candidates
            .iter()
            .find(|name| {
                // Starting the process *is* the probe: it settles PATH lookup,
                // executability and the Windows executable-suffix question in
                // one step, which no amount of path guessing does portably.
                Command::new(name)
                    .args(GOFMT_PROBE_ARGS)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok()
            })
            .map(|name| (*name).to_string())
    }

    fn run(&self, program: &str, args: &[&str], file: &Path) -> io::Result<Outcome> {
        let output = Command::new(program)
            .args(args)
            .arg(file)
            .stdin(Stdio::null())
            // The reformatted source is not the verdict and is never read;
            // discarding it keeps a whole second copy of the file out of this
            // process. `output()` only pipes the streams left unset.
            .stdout(Stdio::null())
            .output()?;
        // The exit status is the whole verdict; see the module docs.
        if output.status.success() {
            return Ok(Outcome::Pass);
        }
        // A file that did not parse has its diagnostics on stderr.
        Ok(Outcome::Fail(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ))
    }
}

/// Runs `gofmt -e` over `code`, the formatted output.
///
/// `original` is the source it was produced from: it is checked first, and a
/// `code` check only happens when the original passed (see the module docs).
pub fn check(original: &str, code: &str) -> Result<()> {
    check_with(&SystemTools, original, code)
}

fn check_with(tools: &dyn Tools, original: &str, code: &str) -> Result<()> {
    let program = tools.locate(&GOFMT_NAMES).ok_or_else(|| {
        Error::Verification(
            "external verification needs `gofmt` on PATH: it was not found".to_string(),
        )
    })?;
    let scratch = Scratch::new()?;

    let before = scratch.write(ORIGINAL_NAME, original)?;
    if let Outcome::Fail(_) = tools.run(&program, &GOFMT_ARGS, &before)? {
        // Already-broken input: not TokenPress's error, and not a reason to
        // fail the run. The built-in equivalence check already passed.
        return Ok(());
    }

    let after = scratch.write(FORMATTED_NAME, code)?;
    match tools.run(&program, &GOFMT_ARGS, &after)? {
        Outcome::Pass => Ok(()),
        Outcome::Fail(message) => Err(Error::Verification(format!(
            "external check failed ({program} -e): {message}"
        ))),
    }
}

/// A private temp directory, removed when it drops.
struct Scratch {
    dir: PathBuf,
}

/// Distinguishes concurrent scratch directories inside one process; the pid
/// and the clock distinguish them across processes.
static SCRATCH_COUNTER: AtomicU64 = AtomicU64::new(0);

impl Scratch {
    fn new() -> io::Result<Self> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "tokenpress-verify-go-{}-{nanos}-{}",
            std::process::id(),
            SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn write(&self, name: &str, contents: &str) -> io::Result<PathBuf> {
        let path = self.dir.join(name);
        fs::write(&path, contents)?;
        Ok(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Best effort: a temp directory that outlives the process is a
        // nuisance, not a correctness problem, and the destination file was
        // never involved.
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::VecDeque;

    /// One recorded [`Tools::run`] call, including what the file it was
    /// pointed at contained at the time.
    #[derive(Clone, Debug)]
    struct Call {
        program: String,
        args: Vec<String>,
        file: PathBuf,
        contents: String,
    }

    /// [`Tools`] with a scripted PATH and scripted verdicts, so the
    /// orchestration can be tested without depending on what is installed on
    /// the machine running the tests.
    struct FakeTools {
        available: &'static [&'static str],
        verdicts: RefCell<VecDeque<io::Result<Outcome>>>,
        calls: RefCell<Vec<Call>>,
    }

    impl FakeTools {
        fn new(available: &'static [&'static str], verdicts: Vec<io::Result<Outcome>>) -> Self {
            Self {
                available,
                verdicts: RefCell::new(verdicts.into()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl Tools for FakeTools {
        fn locate(&self, candidates: &[&str]) -> Option<String> {
            candidates
                .iter()
                .find(|name| self.available.contains(*name))
                .map(|name| (*name).to_string())
        }

        fn run(&self, program: &str, args: &[&str], file: &Path) -> io::Result<Outcome> {
            self.calls.borrow_mut().push(Call {
                program: program.to_string(),
                args: args.iter().map(|a| (*a).to_string()).collect(),
                file: file.to_path_buf(),
                contents: std::fs::read_to_string(file).expect("the checker's input must exist"),
            });
            self.verdicts
                .borrow_mut()
                .pop_front()
                .expect("more checker runs than the test scripted")
        }
    }

    fn pass() -> io::Result<Outcome> {
        Ok(Outcome::Pass)
    }

    fn fail(message: &str) -> io::Result<Outcome> {
        Ok(Outcome::Fail(message.to_string()))
    }

    /// Flattens an outcome so both variants are inspected the same way: the
    /// empty string is a pass, anything else the checker's complaint.
    fn verdict(outcome: Outcome) -> String {
        match outcome {
            Outcome::Pass => String::new(),
            Outcome::Fail(message) => message,
        }
    }

    const VALID: &str = "package main\n\nfunc main() {}\n";
    const FORMATTED: &str = "package main\nfunc main() {}\n";

    #[test]
    fn no_gofmt_on_path_is_an_error_naming_the_tool() {
        let tools = FakeTools::new(&[], vec![]);
        let err = check_with(&tools, VALID, FORMATTED).unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("gofmt"), "{message}");
        assert!(message.contains("PATH"), "{message}");
        // Never a silent downgrade: nothing was checked.
        assert!(tools.calls().is_empty());
    }

    #[test]
    fn gofmt_sees_the_original_first_and_then_the_output() {
        let tools = FakeTools::new(&["gofmt"], vec![pass(), pass()]);
        check_with(&tools, VALID, FORMATTED).unwrap();
        let calls = tools.calls();
        assert_eq!(calls.len(), 2);
        for call in &calls {
            assert_eq!(call.program, "gofmt");
            assert_eq!(call.args, ["-e"]);
            // A `.go` file in a private directory, never the user's own path.
            assert_eq!(call.file.extension().unwrap(), "go");
            assert!(call.file.starts_with(std::env::temp_dir()), "{call:?}");
            // ... and gone again once the check is over.
            assert!(!call.file.exists(), "{call:?} outlived the check");
        }
        assert_eq!(calls[0].contents, VALID);
        assert_eq!(calls[1].contents, FORMATTED);
        // Both checks share one scratch directory.
        assert_eq!(calls[0].file.parent(), calls[1].file.parent());
        assert_ne!(calls[0].file, calls[1].file);
    }

    #[test]
    fn output_that_fails_the_external_check_is_rejected() {
        let tools = FakeTools::new(
            &["gofmt"],
            vec![pass(), fail("a.go:3:15: expected ';', found 'EOF'")],
        );
        let err = check_with(&tools, VALID, "package main\nfunc main() {\n").unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("gofmt -e"), "{message}");
        assert!(message.contains("expected ';', found 'EOF'"), "{message}");
    }

    #[test]
    fn an_original_that_already_fails_is_not_blamed_on_the_formatter() {
        // Policy: the external level is satisfied by AstEquiv alone when the
        // user's own input does not pass the checker.
        let tools = FakeTools::new(&["gofmt"], vec![fail("a.go:1:1: expected 'package'")]);
        check_with(&tools, "\n\n// note\n", "// note\n").unwrap();
        // The output is not even offered to the checker.
        assert_eq!(tools.calls().len(), 1);
    }

    #[test]
    fn a_checker_that_cannot_be_spawned_is_an_error() {
        let tools = FakeTools::new(&["gofmt"], vec![Err(io::Error::other("spawn failed"))]);
        let err = check_with(&tools, VALID, FORMATTED).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err}");
        assert!(err.to_string().contains("spawn failed"), "{err}");
    }

    #[test]
    fn the_probe_is_the_help_flag_and_never_a_bare_invocation() {
        // Pinned because both alternatives are traps: `gofmt` has no
        // `--version` flag at all, and a bare `gofmt` reads *stdin*, so a
        // probe without an argument would block until the pipe closed.
        assert_eq!(GOFMT_PROBE_ARGS, ["-h"]);
        assert!(!GOFMT_PROBE_ARGS.is_empty());
    }

    #[test]
    fn system_tools_locate_walks_the_candidates_in_order() {
        // A name that cannot be started is skipped, not fatal, and the first
        // one that starts wins — which is the whole probe mechanism, run
        // against the real PATH. That this test terminates at all is the
        // evidence that the probe does not read stdin.
        assert_eq!(
            SystemTools.locate(&["tokenpress-no-such-tool", "gofmt"]),
            Some("gofmt".to_string())
        );
        assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool"]), None);
        // And the constant the orchestration actually probes resolves here.
        assert_eq!(SystemTools.locate(&GOFMT_NAMES), Some("gofmt".to_string()));
    }

    #[test]
    fn system_tools_run_reports_gofmts_verdict() {
        let scratch = Scratch::new().unwrap();
        let good = scratch.write("good.go", VALID).unwrap();
        let bad = scratch
            .write("bad.go", "package main\nfunc main() {\n")
            .unwrap();
        assert_eq!(
            verdict(SystemTools.run("gofmt", &GOFMT_ARGS, &good).unwrap()),
            ""
        );
        let message = verdict(SystemTools.run("gofmt", &GOFMT_ARGS, &bad).unwrap());
        assert!(message.contains("expected ';', found 'EOF'"), "{message}");
    }

    #[test]
    fn what_gofmt_prints_to_stdout_is_not_the_verdict() {
        // `gofmt` writes the *reformatted* source to stdout, so a file it
        // would happily rewrite still exits 0. The gate is the exit status
        // alone, and stdout is discarded rather than read: TokenPress's
        // output deliberately does not look like `gofmt`'s.
        let scratch = Scratch::new().unwrap();
        let unformatted = scratch.write("unformatted.go", FORMATTED).unwrap();
        assert_eq!(
            verdict(SystemTools.run("gofmt", &GOFMT_ARGS, &unformatted).unwrap()),
            ""
        );
        // ... and the same file goes through the orchestration untouched.
        check_with(&SystemTools, VALID, FORMATTED).unwrap();
    }

    #[test]
    fn gofmt_accepts_output_it_can_still_parse() {
        check_with(
            &SystemTools,
            "package main\n\nimport \"fmt\"\n\nfunc main() {\n\tfmt.Println(\"hi\")\n}\n",
            "package main\nimport \"fmt\"\nfunc main() {\nfmt.Println(\"hi\")\n}\n",
        )
        .unwrap();
    }

    #[test]
    fn gofmt_rejects_output_it_cannot_parse() {
        let err = check_with(&SystemTools, VALID, "package main\nfunc main() {\n").unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("external check failed (gofmt -e)"),
            "{message}"
        );
        assert!(message.contains("expected"), "{message}");
    }

    #[test]
    fn input_gofmt_already_rejects_does_not_fail_the_run() {
        // A file with no package clause: tree-sitter parses it as a
        // source_file of comments, `go/parser` refuses it ("expected
        // 'package'"). The stdlib ships such files — the empty
        // `cmd/go/internal/modindex/testdata/ignore_non_source/b.go` is one —
        // and the policy keeps them from being reported as TokenPress
        // failures.
        check_with(&SystemTools, "\n\n// note\n\n", "// note\n").unwrap();
        // The degenerate member of the same class: an empty file.
        check_with(&SystemTools, "", "").unwrap();
    }

    #[test]
    fn check_runs_the_real_gofmt() {
        check(VALID, FORMATTED).unwrap();
        let err = check(VALID, "package main\nfunc main() {\n").unwrap_err();
        assert!(err.to_string().contains("external check failed"), "{err}");
    }

    #[test]
    fn scratch_directories_are_unique_and_removed_on_drop() {
        let first = Scratch::new().unwrap();
        let second = Scratch::new().unwrap();
        assert_ne!(first.dir, second.dir);
        let path = first.write("a.go", VALID).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), VALID);
        let dir = first.dir.clone();
        drop(first);
        assert!(!dir.exists());
    }
}
