//! External verification: the output is handed to the JavaScript/TypeScript
//! toolchain itself, on top of the built-in checks in [`crate::verify`].
//!
//! This is what [`tokenpress_core::VerifyLevel::External`] adds for this
//! backend. It never replaces [`crate::verify::equivalent`]; it runs after it.
//!
//! # The checkers
//!
//! - `tsc --noEmit --noCheck --skipLibCheck --allowJs --jsx preserve <file>` —
//!   preferred, and the only one that covers all eight extensions. `--noCheck`
//!   makes it a *syntax* check: a pure type error (`const x: number = "s"`)
//!   exits 0, a syntax error exits non-zero. That is exactly the property
//!   TokenPress needs — it rewrites whitespace, so it can only ever break
//!   syntax, and it must not fail a file whose types were already wrong.
//! - `node --check <file>` — fallback, used only for `.js`, `.mjs` and `.cjs`.
//!   node refuses any other extension outright
//!   (`ERR_UNKNOWN_FILE_EXTENSION`), so for a `.jsx`/`.ts`/`.mts`/`.cts`/`.tsx`
//!   target with no `tsc` on PATH this module reports the missing tool rather
//!   than pretending the file was checked.
//!
//! When neither tool is on PATH, verification **fails**, naming both: silently
//! degrading to the built-in level would turn an explicit `--verify external`
//! into a weaker guarantee than the user asked for.
//!
//! # Already-broken input
//!
//! The checker is run over the **original** source first. If the original does
//! not pass, the formatted output is not checked at all and the level counts as
//! satisfied by [`crate::verify::equivalent`] alone: TokenPress must not be
//! blamed for a file that the toolchain already rejected (a dialect oxc accepts
//! and tsc does not, a `tsc` older than a syntax the file uses, and so on). The
//! run is not failed either — the user's input is not TokenPress's error.
//!
//! # Where the candidate goes
//!
//! Both the original and the candidate output are written to a **private temp
//! directory** (removed when [`Scratch`] drops), each under a file name
//! carrying the same extension as the target path, because the extension is
//! what selects the dialect for both checkers. The user's own path is never
//! written to here — output that fails is discarded by the caller and never
//! reaches the destination file, which is the project's core invariant.
//!
//! # Cost
//!
//! One probe plus two checker processes per file, so `--verify external` is
//! substantially slower than `--verify ast`. That is the price of an
//! independent opinion from the real toolchain.
//!
//! # Windows
//!
//! An npm-installed `tsc` is a `tsc.cmd` shim; `Command::new("tsc")` does not
//! resolve it (`CreateProcess` only appends `.exe`, and the extensionless
//! `tsc` script npm installs next to it is not an image Windows can start). The
//! probe therefore tries the bare name first and `tsc.cmd` second, which
//! resolves on Windows and is simply absent elsewhere. `node` needs no shim
//! name: `node.exe` is found from the bare name.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokenpress_core::{Error, Result};

/// `tsc` first, then the Windows npm shim (see the module docs).
const TSC_NAMES: [&str; 2] = ["tsc", "tsc.cmd"];

/// Syntax-only invocation: `--noCheck` drops type checking, `--allowJs` lets
/// plain JavaScript through, `--jsx preserve` accepts JSX without needing a
/// tsconfig, `--skipLibCheck` keeps it away from `lib.d.ts`.
const TSC_ARGS: [&str; 6] = [
    "--noEmit",
    "--noCheck",
    "--skipLibCheck",
    "--allowJs",
    "--jsx",
    "preserve",
];

const NODE_NAMES: [&str; 1] = ["node"];
const NODE_ARGS: [&str; 1] = ["--check"];

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
                // executability and the Windows shim question in one step,
                // which no amount of path guessing does portably.
                Command::new(name)
                    .arg("--version")
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
            .output()?;
        if output.status.success() {
            return Ok(Outcome::Pass);
        }
        // tsc reports on stdout, node on stderr; keep both.
        let mut message = String::from_utf8_lossy(&output.stdout).into_owned();
        message.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(Outcome::Fail(message.trim().to_string()))
    }
}

/// A checker resolved against the current machine.
struct Checker {
    program: String,
    args: &'static [&'static str],
}

/// A dialect as far as the external checkers care: the extension that selects
/// it, and whether `node --check` can read it.
struct Dialect {
    extension: &'static str,
    node_checkable: bool,
}

/// The eight extensions the backend claims, and which of them node can check.
const DIALECTS: [Dialect; 8] = [
    Dialect {
        extension: "js",
        node_checkable: true,
    },
    Dialect {
        extension: "mjs",
        node_checkable: true,
    },
    Dialect {
        extension: "cjs",
        node_checkable: true,
    },
    Dialect {
        extension: "jsx",
        node_checkable: false,
    },
    Dialect {
        extension: "ts",
        node_checkable: false,
    },
    Dialect {
        extension: "mts",
        node_checkable: false,
    },
    Dialect {
        extension: "cts",
        node_checkable: false,
    },
    Dialect {
        extension: "tsx",
        node_checkable: false,
    },
];

/// The dialect `path`'s extension selects. A `.d.ts` arrives here as `ts`,
/// which is what both checkers want anyway.
fn dialect(path: &Path) -> Result<&'static Dialect> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| DIALECTS.iter().find(|d| d.extension == ext))
        .ok_or_else(|| Error::UnsupportedLanguage(path.display().to_string()))
}

/// Picks the checker to use for `dialect`, or explains which tool is missing.
fn checker(tools: &dyn Tools, dialect: &Dialect) -> Result<Checker> {
    if let Some(program) = tools.locate(&TSC_NAMES) {
        return Ok(Checker {
            program,
            args: &TSC_ARGS,
        });
    }
    if dialect.node_checkable {
        if let Some(program) = tools.locate(&NODE_NAMES) {
            return Ok(Checker {
                program,
                args: &NODE_ARGS,
            });
        }
        return Err(Error::Verification(
            "external verification needs `tsc` or `node` on PATH: neither was found".to_string(),
        ));
    }
    Err(Error::Verification(format!(
        "external verification of a .{} file needs `tsc` on PATH: it was not found, \
         and the `node --check` fallback reads only .js, .mjs and .cjs",
        dialect.extension
    )))
}

/// Runs the external checker over `code`, the formatted output for `path`.
///
/// `original` is the source it was produced from: it is checked first, and a
/// `code` check only happens when the original passed (see the module docs).
pub fn check(path: &Path, original: &str, code: &str) -> Result<()> {
    check_with(&SystemTools, path, original, code)
}

fn check_with(tools: &dyn Tools, path: &Path, original: &str, code: &str) -> Result<()> {
    let dialect = dialect(path)?;
    let checker = checker(tools, dialect)?;
    let scratch = Scratch::new()?;

    let before = scratch.write(&format!("original.{}", dialect.extension), original)?;
    if let Outcome::Fail(_) = tools.run(&checker.program, checker.args, &before)? {
        // Already-broken input: not TokenPress's error, and not a reason to
        // fail the run. The built-in equivalence check already passed.
        return Ok(());
    }

    let after = scratch.write(&format!("formatted.{}", dialect.extension), code)?;
    match tools.run(&checker.program, checker.args, &after)? {
        Outcome::Pass => Ok(()),
        Outcome::Fail(message) => Err(Error::Verification(format!(
            "external check failed ({}): {message}",
            checker.program
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
            "tokenpress-verify-{}-{nanos}-{}",
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

    #[test]
    fn no_checker_on_path_is_an_error_naming_both_tools() {
        let tools = FakeTools::new(&[], vec![]);
        let err =
            check_with(&tools, Path::new("a.js"), "const a = 1;\n", "const a=1;").unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("tsc"), "{message}");
        assert!(message.contains("node"), "{message}");
        assert!(tools.calls().is_empty());
    }

    #[test]
    fn tsc_is_preferred_and_sees_the_original_then_the_output() {
        let tools = FakeTools::new(&["tsc", "node"], vec![pass(), pass()]);
        check_with(
            &tools,
            Path::new("src/a.ts"),
            "const a: number = 1;\n",
            "const a:number=1;",
        )
        .unwrap();
        let calls = tools.calls();
        assert_eq!(calls.len(), 2);
        for call in &calls {
            assert_eq!(call.program, "tsc");
            assert_eq!(
                call.args,
                [
                    "--noEmit",
                    "--noCheck",
                    "--skipLibCheck",
                    "--allowJs",
                    "--jsx",
                    "preserve"
                ]
            );
            // Same dialect as the target, never the target itself.
            assert_eq!(call.file.extension().unwrap(), "ts");
            assert_ne!(call.file, Path::new("src/a.ts"));
            // ... and gone again once the check is over.
            assert!(!call.file.exists(), "{call:?} outlived the check");
        }
        assert_eq!(calls[0].contents, "const a: number = 1;\n");
        assert_eq!(calls[1].contents, "const a:number=1;");
    }

    #[test]
    fn node_is_the_fallback_for_the_dialects_it_understands() {
        for name in ["a.js", "a.mjs", "a.cjs"] {
            let tools = FakeTools::new(&["node"], vec![pass(), pass()]);
            check_with(&tools, Path::new(name), "const a = 1;\n", "const a=1;").unwrap();
            let calls = tools.calls();
            assert_eq!(calls.len(), 2, "{name}");
            assert_eq!(calls[0].program, "node");
            assert_eq!(calls[0].args, ["--check"]);
            let ext = name.rsplit('.').next().unwrap();
            assert_eq!(calls[0].file.extension().unwrap(), ext, "{name}");
        }
    }

    #[test]
    fn node_alone_cannot_check_the_typescript_and_jsx_dialects() {
        for name in ["a.jsx", "a.ts", "a.mts", "a.cts", "a.tsx"] {
            let tools = FakeTools::new(&["node"], vec![]);
            let err =
                check_with(&tools, Path::new(name), "const a = 1;\n", "const a=1;").unwrap_err();
            let message = err.to_string();
            assert!(message.contains("tsc"), "{name}: {message}");
            assert!(
                message.contains(name.rsplit('.').next().unwrap()),
                "{message}"
            );
            assert!(tools.calls().is_empty(), "{name}");
        }
    }

    #[test]
    fn the_windows_npm_shim_is_probed_after_the_bare_name() {
        let tools = FakeTools::new(&["tsc.cmd"], vec![pass(), pass()]);
        check_with(
            &tools,
            Path::new("a.tsx"),
            "const a = <p/>;\n",
            "const a=<p/>;",
        )
        .unwrap();
        assert_eq!(tools.calls()[0].program, "tsc.cmd");
    }

    #[test]
    fn output_that_fails_the_external_check_is_rejected() {
        let tools = FakeTools::new(
            &["node"],
            vec![pass(), fail("SyntaxError: Unexpected token")],
        );
        let err = check_with(&tools, Path::new("a.js"), "const a = 1;\n", "const a=;").unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("node"), "{message}");
        assert!(
            message.contains("SyntaxError: Unexpected token"),
            "{message}"
        );
    }

    #[test]
    fn an_original_that_already_fails_is_not_blamed_on_the_formatter() {
        // Policy: the external level is satisfied by AstEquiv alone when the
        // user's own input does not pass the checker.
        let tools = FakeTools::new(&["node"], vec![fail("SyntaxError: in the input")]);
        check_with(&tools, Path::new("a.js"), "const a = 1;\n", "const a=1;").unwrap();
        // The output is not even offered to the checker.
        assert_eq!(tools.calls().len(), 1);
    }

    #[test]
    fn a_checker_that_cannot_be_spawned_is_an_error() {
        let tools = FakeTools::new(&["node"], vec![Err(io::Error::other("spawn failed"))]);
        let err =
            check_with(&tools, Path::new("a.js"), "const a = 1;\n", "const a=1;").unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err}");
        assert!(err.to_string().contains("spawn failed"), "{err}");
    }

    #[test]
    fn an_extension_that_maps_to_no_dialect_is_refused() {
        let tools = FakeTools::new(&["tsc"], vec![]);
        for name in ["notes.txt", "noextension"] {
            let err =
                check_with(&tools, Path::new(name), "const a = 1;\n", "const a=1;").unwrap_err();
            assert_eq!(
                err.to_string(),
                format!("unsupported language for path: {name}")
            );
        }
    }

    #[test]
    fn system_tools_locate_finds_node_and_misses_a_bogus_name() {
        assert_eq!(
            SystemTools.locate(&["tokenpress-no-such-tool", "node"]),
            Some("node".to_string())
        );
        assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool"]), None);
    }

    #[test]
    fn system_tools_run_reports_the_checkers_verdict() {
        // `node --check` is the one checker both CI runners have.
        let scratch = Scratch::new().unwrap();
        let good = scratch.write("good.js", "const a=1;").unwrap();
        let bad = scratch.write("bad.js", "const a=;").unwrap();
        assert_eq!(
            verdict(SystemTools.run("node", &["--check"], &good).unwrap()),
            ""
        );
        let message = verdict(SystemTools.run("node", &["--check"], &bad).unwrap());
        assert!(message.contains("SyntaxError"), "{message}");
    }

    /// Real processes, but with `tsc` hidden: the fallback is the one checker
    /// every machine this project supports is guaranteed to have, so the
    /// policy tests below behave the same everywhere.
    struct NodeOnly;

    impl Tools for NodeOnly {
        fn locate(&self, candidates: &[&str]) -> Option<String> {
            candidates.contains(&"node").then(|| "node".to_string())
        }

        fn run(&self, program: &str, args: &[&str], file: &Path) -> io::Result<Outcome> {
            SystemTools.run(program, args, file)
        }
    }

    #[test]
    fn node_accepts_output_it_can_still_parse() {
        check_with(
            &NodeOnly,
            Path::new("a.js"),
            "function add( a , b ) {\n    return a + b;\n}\n",
            "function add(a,b){return a+b}",
        )
        .unwrap();
    }

    #[test]
    fn node_rejects_output_it_cannot_parse() {
        let err =
            check_with(&NodeOnly, Path::new("a.js"), "const a = 1;\n", "const a=;").unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("external check failed (node)"),
            "{message}"
        );
        assert!(message.contains("SyntaxError"), "{message}");
    }

    #[test]
    fn input_node_already_rejects_does_not_fail_the_run() {
        // ESM syntax in a `.cjs` file: oxc parses it, node refuses it. The
        // formatted output would be refused for exactly the same reason, so
        // the policy is what keeps this from being reported as a TokenPress
        // failure.
        check_with(
            &NodeOnly,
            Path::new("legacy.cjs"),
            "export const a = 1;\n",
            "export const a=1;",
        )
        .unwrap();
    }

    #[test]
    fn check_runs_the_real_toolchain() {
        // Whichever checker the machine has: `tsc` when installed, otherwise
        // `node`. Both accept `.js`.
        check(Path::new("a.js"), "const a = 1;\n", "const a=1;").unwrap();
        let err = check(Path::new("a.js"), "const a = 1;\n", "const a=;").unwrap_err();
        assert!(err.to_string().contains("external check failed"), "{err}");
    }

    #[test]
    fn scratch_directories_are_unique_and_removed_on_drop() {
        let first = Scratch::new().unwrap();
        let second = Scratch::new().unwrap();
        assert_ne!(first.dir, second.dir);
        let path = first.write("a.js", "const a=1;").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "const a=1;");
        let dir = first.dir.clone();
        drop(first);
        assert!(!dir.exists());
    }
}
