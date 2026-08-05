//! External verification: the output is handed to Roslyn's `csc` itself, on
//! top of the built-in checks in `tokenpress_treesitter::verify`.
//!
//! This is what [`tokenpress_core::VerifyLevel::External`] adds for this
//! backend. It never replaces the equivalence check; it runs after it.
//!
//! # The checker
//!
//! `dotnet <sdk>/Roslyn/bincore/csc.dll /nologo /noconfig /nostdlib+
//! /t:library /langversion:latest <file>` — the C# compiler, run over one file
//! with **no reference assemblies at all**. `/nostdlib+` is what makes that
//! possible: no ref pack, no project file, no restore and no network, so an
//! arbitrary user file can be handed to it in isolation.
//!
//! It is not Java's design, because it cannot be. Every other backend's
//! external gate is a parse-only mode of the real tool — `gofmt -e`,
//! `ruby -c`, `javac -XDshould-stop.ifNoError=PARSE`. **Roslyn has no
//! parse-only switch**, so a file that references types it cannot resolve —
//! the normal case for any file of a real project checked on its own — comes
//! back with a pile of semantic errors. Measured on .NET SDK 8.0.129 over
//! [`SELF_TEST_SOURCE`], a nine-line file: `CS0246` (unknown type), `CS0518`
//! (`System.Object` is not defined) and `CS0656` (a compiler-required member
//! is missing), twelve diagnostics in all. There is no phase to stop at.
//!
//! So the noise is not removed — it is made to **cancel**. The checker runs
//! over the original *and* over the candidate, the `error CS####` codes of
//! each run are collected into a multiset with their positions discarded
//! (formatting moves positions, so a position is not part of the verdict), and
//! the output is accepted only when the two multisets are **equal**.
//! Unresolvable references produce the same complaints on both sides and
//! cancel; a syntax error introduced by TokenPress appears on one side only.
//! That is this backend's analogue of javac's `PARSE` flag.
//!
//! # The verdict is the diagnostics, never the exit status
//!
//! `csc` exits non-zero for exactly the semantic noise this design
//! deliberately tolerates: the fixture above is exit 1 and always will be. An
//! exit-status gate — the rule every other backend here uses — would refuse
//! **every** real-world file. So the verdict is parsed out of the checker's
//! human-readable output, which makes this the first backend in the project
//! coupled to what a tool *prints*.
//!
//! That coupling is stated here rather than left implicit, because
//! `tokenpress-java`'s module docs record what happens when it is not: javac's
//! gate reads the exit status and **not** stderr, because the JVM launcher
//! prints `Picked up JAVA_TOOL_OPTIONS: …` to stderr on every invocation
//! wherever that variable is set, so a stderr-based gate would reject every
//! file on those machines. The same question has to be answered here, in the
//! other direction, and it was measured rather than assumed: on .NET SDK
//! 8.0.129 `csc` writes its `error CS####` diagnostics to **stdout**, leaves
//! **stderr empty**, and exits 1. This module therefore reads stdout and
//! nothing else — see `csc_writes_its_diagnostics_to_stdout`, which pins all
//! three halves of that measurement against the real binary.
//!
//! # No code-range filter
//!
//! The obvious shortcut — "only fail on `CS1xxx`, the syntactic range" — is
//! **unsound**, and was measured to be. A missing parenthesis really is
//! `CS1026: ) expected`. But a top-level statement placed after a type
//! declaration is `CS8803`, which is outside that range and is just as much a
//! syntax error; newer language features carry their syntax diagnostics in the
//! `CS8xxx` range generally. A range filter would silently pass real
//! breakage. `csc_rejects_a_top_level_statement_after_a_type_declaration` is
//! the test that fails if anyone reintroduces the idea.
//!
//! # The self-test
//!
//! Because the whole verdict is parsed text, the gate is tested before it is
//! trusted — and here that is load-bearing in a way it was not for Java. If a
//! future SDK reworded its diagnostics past [`diagnostic_codes`], every run
//! would extract an empty multiset, two empty multisets would compare equal,
//! and this gate would degenerate into a check that passes everything while
//! reporting success. Nothing about the exit status or the process would look
//! wrong.
//!
//! So [`check`] first runs the gate over [`SELF_TEST_SOURCE`] and
//! [`SELF_TEST_MINIMIZED`] — a built-in valid-but-unresolvable fixture and
//! this backend's own output for it — and requires **both** halves:
//!
//! 1. every code in [`SELF_TEST_CODES`] was actually extracted, so a
//!    diagnostic format this module can no longer read fails loudly instead of
//!    comparing two empty multisets; and
//! 2. the two multisets are equal, so an SDK whose noise stopped cancelling
//!    fails loudly instead of blaming the user's file.
//!
//! The exact *counts* are deliberately not pinned in the gate — they are an
//! SDK-version detail, and pinning them would turn a patch release into a
//! failure for every user. The distinct codes are pinned, which is what
//! catches a format change. The self-test runs once per [`check`] rather than
//! once per process on purpose: a cached verdict would be global mutable state
//! whose value depended on which file happened to be checked first.
//!
//! # Already-broken input
//!
//! The checker is run over the **original** first, and its multiset is the
//! baseline the candidate is compared against. A file the toolchain already
//! rejects is therefore never blamed on TokenPress — not by a special case,
//! but because the identical complaint appears on both sides and cancels like
//! any other noise. `class Big { long x = 99999999999999999999999; }` is a
//! clean tree-sitter parse and `error CS1021: Integral constant is too large`
//! to `csc`, before and after formatting alike, and the run succeeds.
//!
//! # Where the candidate goes
//!
//! The self-test pair, the original and the candidate are written to a
//! **private temp directory** (removed when the private `Scratch` guard
//! drops), each under a fixed `.cs` name, and `csc` is run with that directory
//! as its working directory. The user's own path is never written to here —
//! output that fails is discarded by the caller and never reaches the
//! destination file, which is the project's core invariant. The working
//! directory matters for two reasons beyond tidiness: any artifact `csc` might
//! emit lands inside the scratch directory and goes with it, and the file is
//! named to the compiler *relatively*, so the diagnostics quoted back read
//! `formatted.cs(2,19): …` instead of leaking a temp path.
//!
//! Fixed names are safe: C# has no rule tying a type's name to its file's, and
//! `.cs` is the only extension this backend claims (see [`crate::paths`]), so
//! one fixed name serves every path. Neither is the directory a parameter —
//! nothing outside the file is resolved under `/nostdlib+`, so a file checked
//! outside its project checks no differently inside it.
//!
//! # Finding the compiler
//!
//! `csc` is not a program on PATH: the SDK ships it as a managed assembly run
//! by the `dotnet` host. The SDK's version is part of that path and is
//! **discovered, never hardcoded** — `dotnet --list-sdks` prints
//! `8.0.129 [/usr/lib/dotnet/sdk]` per installed SDK, newest last, and
//! [`sdk_compilers`] turns each line into a `csc.dll` path with [`PathBuf`]
//! rather than string concatenation, because the directory shape differs on
//! Windows (`C:\Program Files\dotnet\sdk`) and CI runs both.
//!
//! When no `dotnet` with a usable `csc.dll` is found, verification **fails**,
//! naming it: silently degrading to the built-in level would turn an explicit
//! `--verify external` into a weaker guarantee than the user asked for.
//!
//! # Text, not bytes
//!
//! C# source has no fixed encoding — `csc /codepage` exists — but this module
//! takes `&str`: it is handed exactly what [`crate::CSharpFormatter::format`]
//! was handed and what it produced, and that signature is `&str` all the way
//! up from [`tokenpress_core::Formatter::format`]. A non-UTF-8 file never
//! reaches this crate at all; see the crate-level docs. What is written here
//! is therefore UTF-8, which is what `csc` reads by default.
//!
//! # Cost
//!
//! One `dotnet --list-sdks` probe (measured at 4 ms) plus **four** `csc`
//! invocations per file — the two self-test halves, the original, the
//! candidate — each measured at about 0.36 s. `--verify external` is
//! therefore far slower here than `--verify ast`. That is the price of an
//! independent opinion from the real compiler, and of knowing the gate is
//! still the gate.
//!
//! # Windows
//!
//! The bare name is enough. `tokenpress-js` has to probe `tsc.cmd` as well
//! because an npm-installed `tsc` is a batch shim that `CreateProcess` will
//! not start from the extensionless name; the .NET SDK installs a real
//! `dotnet.exe` and `CreateProcess` appends `.exe` itself. The candidate list
//! is kept as a list anyway, because the probe is what decides and a second
//! name can be added without reshaping anything.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use tokenpress_core::{Error, Result};

/// The names probed for the .NET host, in order (see the module docs on
/// Windows for why there is only one).
const DOTNET_NAMES: [&str; 1] = ["dotnet"];

/// What the probe passes. `dotnet --list-sdks` both proves the host starts and
/// reports where the compiler lives, which is why it is one step and not two.
const LIST_SDKS_ARG: &str = "--list-sdks";

/// Where `csc.dll` sits inside one SDK directory. Joined with [`PathBuf`], not
/// concatenated, because CI runs on Windows as well as Linux.
const CSC_RELATIVE: [&str; 3] = ["Roslyn", "bincore", "csc.dll"];

/// The invocation. `/nostdlib+` is the load-bearing one: no reference
/// assemblies, so no ref pack, project file, restore or network is needed to
/// check an arbitrary file. `/langversion:latest` is passed rather than left
/// to the SDK's default so that today's syntax is not reported as tomorrow's
/// syntax error; `langversion_latest_accepts_current_syntax` pins it.
const CSC_ARGS: [&str; 5] = [
    "/nologo",
    "/noconfig",
    "/nostdlib+",
    "/t:library",
    "/langversion:latest",
];

/// The file names the fixtures are checked under; all four are private to a
/// `Scratch` directory.
const FORMATTED_NAME: &str = "formatted.cs";
const ORIGINAL_NAME: &str = "original.cs";
const SELF_TEST_NAME: &str = "selftest.cs";
const SELF_TEST_MINIMIZED_NAME: &str = "selftest-minimized.cs";

/// What a diagnostic line says just before its code. Only errors are read:
/// `csc` also emits `warning CS####` lines for perfectly good files (a
/// `warning CS8021` about `RuntimeMetadataVersion` is routine under
/// `/nostdlib+`), and a warning is not a verdict.
const ERROR_MARKER: &str = "error CS";

/// Valid C# in which nothing resolves: there is no `Example.Nope`, no
/// `Widget`, `Gadget` or `Thing`, and under `/nostdlib+` not even
/// `System.Object`.
///
/// It is the fixture the gate is tested with before it is trusted. Measured on
/// .NET SDK 8.0.129: 4 × `CS0246`, 6 × `CS0518` and 2 × `CS0656` — twelve
/// diagnostics, byte-identical before and after minimization, which is the
/// row the whole design turns on.
const SELF_TEST_SOURCE: &str = "\
using Example.Nope;

public class Probe
{
    private Widget widget;

    public void First(params Gadget[] parts) { }

    public void Second(params Thing[] parts) { }
}
";

/// [`SELF_TEST_SOURCE`] after this backend's own minimization.
///
/// Written out rather than computed so the self-test does not depend on the
/// formatter it is defending; `the_self_test_pair_is_this_backends_own_output`
/// pins that the two cannot drift apart.
const SELF_TEST_MINIMIZED: &str = "\
using Example.Nope;
public class Probe
{
private Widget widget;
public void First(params Gadget[] parts) { }
public void Second(params Thing[] parts) { }
}
";

/// The distinct codes [`SELF_TEST_SOURCE`] is known to provoke, sorted.
///
/// The gate requires every one of them to come back out of
/// [`diagnostic_codes`]. Their *counts* are deliberately not part of the
/// contract — those are an SDK-version detail — but their presence is what
/// proves the diagnostics are still being read at all.
const SELF_TEST_CODES: [&str; 3] = ["CS0246", "CS0518", "CS0656"];

/// How the compiler is invoked: the host program, and the `csc.dll` an SDK
/// on this machine provides.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Compiler {
    program: String,
    csc: PathBuf,
}

impl Compiler {
    /// The command line, for quoting back in a diagnostic.
    fn invocation(&self) -> String {
        format!(
            "{} {} {}",
            self.program,
            self.csc.display(),
            CSC_ARGS.join(" ")
        )
    }
}

/// What one checker run reported.
#[derive(Clone, Debug)]
struct Report {
    /// The `error CS####` codes, sorted, so two multisets compare by equality.
    codes: Vec<String>,
    /// What the checker printed, kept only so a rejection can quote it.
    text: String,
}

/// The seam between the orchestration and the processes it drives, so the
/// orchestration can be tested without depending on what is installed on the
/// machine running the tests.
trait Tools {
    /// Returns the first of `candidates` that can actually be started and has
    /// an SDK with a `csc.dll`, or `None` when none of them can.
    fn locate(&self, candidates: &[&str]) -> Option<Compiler>;

    /// Runs the compiler over `name`, inside `dir`, and reports what it said.
    /// `Err` means the process could not be run at all.
    fn run(&self, compiler: &Compiler, dir: &Path, name: &str) -> io::Result<Report>;
}

/// The real compiler: SDK discovery by spawning, checks by spawning.
struct SystemTools;

impl Tools for SystemTools {
    fn locate(&self, candidates: &[&str]) -> Option<Compiler> {
        candidates.iter().find_map(|name| {
            // Starting the process *is* the probe: it settles PATH lookup,
            // executability and the Windows executable-suffix question in one
            // step, and its output is what says where `csc.dll` lives.
            let listing = Command::new(name)
                .arg(LIST_SDKS_ARG)
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&listing.stdout);
            // Newest last, so the newest usable one wins.
            let csc = sdk_compilers(&text)
                .into_iter()
                .rev()
                .find(|p| p.is_file())?;
            Some(Compiler {
                program: (*name).to_string(),
                csc,
            })
        })
    }

    fn run(&self, compiler: &Compiler, dir: &Path, name: &str) -> io::Result<Report> {
        let output = Command::new(&compiler.program)
            .arg(&compiler.csc)
            .args(CSC_ARGS)
            .arg(name)
            // See the module docs: it keeps any artifact inside the scratch
            // directory and keeps the temp path out of the quoted diagnostics.
            .current_dir(dir)
            .stdin(Stdio::null())
            .output()?;
        // Stdout, and only stdout: that is where `csc` writes its diagnostics,
        // and the exit status is not the verdict. See the module docs.
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Report {
            codes: diagnostic_codes(&text),
            text,
        })
    }
}

/// The `csc.dll` path of every SDK a `dotnet --list-sdks` listing reports, in
/// the order it reported them — ascending by version, so the newest is last.
///
/// Each line is `<version> [<directory>]`. A line that is not shaped that way
/// is skipped rather than guessed at.
fn sdk_compilers(listing: &str) -> Vec<PathBuf> {
    listing
        .lines()
        .filter_map(|line| {
            // `trim` before anything else: a Windows child process ends its
            // lines with `\r`, which would otherwise land inside the path.
            let (version, rest) = line.trim().split_once(" [")?;
            let mut path = PathBuf::from(rest.strip_suffix(']')?);
            path.push(version);
            path.extend(CSC_RELATIVE);
            Some(path)
        })
        .collect()
}

/// The `error CS####` codes in a `csc` run's standard output, sorted so two
/// multisets compare by equality.
///
/// Positions are discarded on purpose: formatting moves them, so a position
/// cannot be part of the verdict. Warnings are not codes here — see
/// [`ERROR_MARKER`].
fn diagnostic_codes(output: &str) -> Vec<String> {
    let mut codes: Vec<String> = output
        .lines()
        .filter_map(|line| {
            let digits: String = line
                .split_once(ERROR_MARKER)?
                .1
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            (!digits.is_empty()).then(|| format!("CS{digits}"))
        })
        .collect();
    codes.sort();
    codes
}

/// The multiset difference between two sorted code lists: what the candidate
/// gained (`+`) and what it lost (`-`), for quoting back.
fn difference(before: &[String], after: &[String]) -> Vec<String> {
    let mut counts: BTreeMap<&str, i64> = BTreeMap::new();
    for code in after {
        *counts.entry(code.as_str()).or_default() += 1;
    }
    for code in before {
        *counts.entry(code.as_str()).or_default() -= 1;
    }
    let mut gained: Vec<String> = Vec::new();
    let mut lost: Vec<String> = Vec::new();
    for (code, count) in counts {
        if count > 0 {
            gained.push(format!("+{count} {code}"));
        }
        if count < 0 {
            lost.push(format!("-{} {code}", -count));
        }
    }
    gained.extend(lost);
    gained
}

/// Runs `csc`'s diagnostic-multiset gate over `code`, the formatted output.
///
/// `original` is the source it was produced from: it is checked first and its
/// diagnostics are the baseline (see the module docs).
pub fn check(original: &str, code: &str) -> Result<()> {
    check_with(&SystemTools, original, code)
}

fn check_with(tools: &dyn Tools, original: &str, code: &str) -> Result<()> {
    let compiler = tools.locate(&DOTNET_NAMES).ok_or_else(|| {
        Error::Verification(
            "external verification needs the .NET SDK: no `dotnet` on PATH reporting an SDK \
             with a Roslyn `csc.dll` was found"
                .to_string(),
        )
    })?;
    let scratch = Scratch::new()?;
    self_test(tools, &compiler, &scratch)?;

    // The original first: its diagnostics are the baseline, which is what
    // keeps a file the toolchain already rejects from being blamed on
    // TokenPress. See the module docs.
    scratch.write(ORIGINAL_NAME, original)?;
    let before = tools.run(&compiler, scratch.dir(), ORIGINAL_NAME)?;
    scratch.write(FORMATTED_NAME, code)?;
    let after = tools.run(&compiler, scratch.dir(), FORMATTED_NAME)?;
    if before.codes == after.codes {
        return Ok(());
    }
    Err(Error::Verification(format!(
        "external check failed ({}): `csc` reports different diagnostics for the output than \
         for the input ({}), so formatting changed what the compiler sees: {}",
        compiler.invocation(),
        difference(&before.codes, &after.codes).join(", "),
        after.text
    )))
}

/// Tests the gate before it is trusted; see the module docs on the self-test.
fn self_test(tools: &dyn Tools, compiler: &Compiler, scratch: &Scratch) -> Result<()> {
    scratch.write(SELF_TEST_NAME, SELF_TEST_SOURCE)?;
    let before = tools.run(compiler, scratch.dir(), SELF_TEST_NAME)?;
    if !SELF_TEST_CODES
        .iter()
        .all(|code| before.codes.iter().any(|seen| seen == code))
    {
        return Err(Error::Verification(format!(
            "external verification cannot trust `{}`: its self-test file — valid C# in which \
             nothing resolves — was expected to report {} and reported {} instead, so this \
             SDK's diagnostics are no longer being read and every file would compare equal to \
             every other: {}",
            compiler.invocation(),
            SELF_TEST_CODES.join(" "),
            before.codes.join(" "),
            before.text
        )));
    }
    scratch.write(SELF_TEST_MINIMIZED_NAME, SELF_TEST_MINIMIZED)?;
    let after = tools.run(compiler, scratch.dir(), SELF_TEST_MINIMIZED_NAME)?;
    if before.codes != after.codes {
        return Err(Error::Verification(format!(
            "external verification cannot trust `{}`: its self-test pair — the same file before \
             and after this backend's own minimization — reported {} and {}, so the semantic \
             noise this gate relies on cancelling no longer cancels: {}",
            compiler.invocation(),
            before.codes.join(" "),
            after.codes.join(" "),
            after.text
        )));
    }
    Ok(())
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
            "tokenpress-verify-csharp-{}-{nanos}-{}",
            std::process::id(),
            SCRATCH_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    fn dir(&self) -> &Path {
        &self.dir
    }

    fn write(&self, name: &str, contents: &str) -> io::Result<()> {
        fs::write(self.dir.join(name), contents)
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
        dir: PathBuf,
        name: String,
        contents: String,
    }

    /// [`Tools`] with a scripted SDK and scripted verdicts, so the
    /// orchestration can be tested without depending on what is installed on
    /// the machine running the tests.
    struct FakeTools {
        available: &'static [&'static str],
        reports: RefCell<VecDeque<io::Result<Report>>>,
        calls: RefCell<Vec<Call>>,
    }

    impl FakeTools {
        fn new(available: &'static [&'static str], reports: Vec<io::Result<Report>>) -> Self {
            Self {
                available,
                reports: RefCell::new(reports.into()),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Call> {
            self.calls.borrow().clone()
        }
    }

    impl Tools for FakeTools {
        fn locate(&self, candidates: &[&str]) -> Option<Compiler> {
            candidates
                .iter()
                .find(|name| self.available.contains(*name))
                .map(|name| Compiler {
                    program: (*name).to_string(),
                    csc: PathBuf::from("sdk").join("csc.dll"),
                })
        }

        fn run(&self, _compiler: &Compiler, dir: &Path, name: &str) -> io::Result<Report> {
            self.calls.borrow_mut().push(Call {
                dir: dir.to_path_buf(),
                name: name.to_string(),
                contents: std::fs::read_to_string(dir.join(name))
                    .expect("the checker's input must exist"),
            });
            self.reports
                .borrow_mut()
                .pop_front()
                .expect("more checker runs than the test scripted")
        }
    }

    /// A scripted report, from the codes alone.
    fn reports(codes: &[&str]) -> io::Result<Report> {
        Ok(Report {
            codes: codes.iter().map(|c| (*c).to_string()).collect(),
            text: codes
                .iter()
                .map(|c| format!("a.cs(1,1): error {c}: something"))
                .collect::<Vec<_>>()
                .join("\n"),
        })
    }

    /// What the self-test expects to see, as a scripted report.
    fn self_test_passes() -> io::Result<Report> {
        reports(&SELF_TEST_CODES)
    }

    /// A plain class: one `CS0518` on both sides, because `System.Object` is
    /// not defined under `/nostdlib+`. The gate's ordinary accepted row.
    const VALID: &str =
        "public class A\n{\n\n    int F(int a)\n    {\n        return a;\n    }\n}\n";
    const FORMATTED: &str = "public class A\n{\nint F(int a)\n{\nreturn a;\n}\n}\n";
    /// tree-sitter parses it; `csc` calls it `CS1026: ) expected`.
    const BROKEN: &str = "public class A\n{\nint F( { return 1; }\n}\n";

    #[test]
    fn no_dotnet_on_path_is_an_error_naming_the_tool() {
        let tools = FakeTools::new(&[], vec![]);
        let err = check_with(&tools, VALID, FORMATTED).unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("dotnet"), "{message}");
        assert!(message.contains("PATH"), "{message}");
        // Never a silent downgrade: nothing was checked.
        assert!(tools.calls().is_empty());
    }

    #[test]
    fn csc_sees_the_self_test_pair_then_the_original_then_the_output() {
        let tools = FakeTools::new(
            &["dotnet"],
            vec![
                self_test_passes(),
                self_test_passes(),
                reports(&["CS0518"]),
                reports(&["CS0518"]),
            ],
        );
        check_with(&tools, VALID, FORMATTED).unwrap();
        let calls = tools.calls();
        assert_eq!(calls.len(), 4);
        for call in &calls {
            // A `.cs` file in a private directory, never the user's own path.
            assert!(call.name.ends_with(".cs"), "{call:?}");
            assert!(call.dir.starts_with(std::env::temp_dir()), "{call:?}");
            // ... and gone again once the check is over.
            assert!(!call.dir.exists(), "{call:?} outlived the check");
            // All four share one scratch directory.
            assert_eq!(call.dir, calls[0].dir);
        }
        assert_eq!(calls[0].name, SELF_TEST_NAME);
        assert_eq!(calls[0].contents, SELF_TEST_SOURCE);
        assert_eq!(calls[1].name, SELF_TEST_MINIMIZED_NAME);
        assert_eq!(calls[1].contents, SELF_TEST_MINIMIZED);
        assert_eq!(calls[2].name, ORIGINAL_NAME);
        assert_eq!(calls[2].contents, VALID);
        assert_eq!(calls[3].name, FORMATTED_NAME);
        assert_eq!(calls[3].contents, FORMATTED);
    }

    #[test]
    fn a_self_test_whose_codes_were_not_extracted_stops_the_gate() {
        // The failure this exists for, and the reason the self-test is not
        // optional here: an SDK that reworded its diagnostics past
        // `diagnostic_codes` would yield an empty multiset for every file,
        // two empty multisets would compare equal, and the gate would pass
        // everything while looking healthy.
        let tools = FakeTools::new(&["dotnet"], vec![reports(&[])]);
        let err = check_with(&tools, VALID, FORMATTED).unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("self-test"), "{message}");
        assert!(message.contains("CS0246"), "{message}");
        assert!(message.contains("csc.dll"), "{message}");
        // Neither the input nor the output was offered to a gate that cannot
        // be trusted.
        assert_eq!(tools.calls().len(), 1);
    }

    #[test]
    fn a_self_test_whose_noise_stopped_cancelling_stops_the_gate() {
        // The other half: the codes are still readable, but the same file
        // before and after minimization no longer produces the same
        // complaints, so the cancellation the whole design rests on is gone.
        let tools = FakeTools::new(
            &["dotnet"],
            vec![self_test_passes(), reports(&["CS0246", "CS0518"])],
        );
        let err = check_with(&tools, VALID, FORMATTED).unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("self-test"), "{message}");
        assert!(message.contains("cancel"), "{message}");
        assert_eq!(tools.calls().len(), 2);
    }

    #[test]
    fn output_whose_diagnostics_differ_is_rejected() {
        let tools = FakeTools::new(
            &["dotnet"],
            vec![
                self_test_passes(),
                self_test_passes(),
                reports(&["CS0518"]),
                reports(&["CS1026"]),
            ],
        );
        let err = check_with(&tools, VALID, BROKEN).unwrap_err();
        assert!(matches!(err, Error::Verification(_)), "{err}");
        let message = err.to_string();
        assert!(message.contains("external check failed"), "{message}");
        assert!(message.contains("+1 CS1026"), "{message}");
        assert!(message.contains("-1 CS0518"), "{message}");
        // The checker's own words are quoted back.
        assert!(message.contains("error CS1026"), "{message}");
    }

    #[test]
    fn an_original_the_checker_already_rejects_is_not_blamed_on_the_formatter() {
        // Policy, and here it is not a special case: the same complaint on
        // both sides cancels like any other noise.
        let tools = FakeTools::new(
            &["dotnet"],
            vec![
                self_test_passes(),
                self_test_passes(),
                reports(&["CS1021"]),
                reports(&["CS1021"]),
            ],
        );
        check_with(
            &tools,
            "class Big\n{\n    long  x  =  99999999999999999999999;\n}\n",
            "class Big\n{\nlong x = 99999999999999999999999;\n}\n",
        )
        .unwrap();
        assert_eq!(tools.calls().len(), 4);
    }

    #[test]
    fn a_checker_that_cannot_be_spawned_is_an_error() {
        let tools = FakeTools::new(&["dotnet"], vec![Err(io::Error::other("spawn failed"))]);
        let err = check_with(&tools, VALID, FORMATTED).unwrap_err();
        assert!(matches!(err, Error::Io(_)), "{err}");
        assert!(err.to_string().contains("spawn failed"), "{err}");
    }

    #[test]
    fn diagnostic_codes_are_a_multiset_with_positions_discarded() {
        // The same two codes at different positions and in a different order
        // are the same verdict, which is what lets a reformatted file compare
        // equal to the file it came from.
        let before = "a.cs(1,7): error CS0246: nope\na.cs(9,3): error CS0518: nope\n";
        let after = "a.cs(4,1): error CS0518: nope\na.cs(1,7): error CS0246: nope\n";
        assert_eq!(diagnostic_codes(before), ["CS0246", "CS0518"]);
        assert_eq!(diagnostic_codes(before), diagnostic_codes(after));
        // Repeats are kept: it is a multiset, not a set.
        let twice = "a.cs(1,1): error CS0518: nope\na.cs(2,2): error CS0518: nope\n";
        assert_eq!(diagnostic_codes(twice), ["CS0518", "CS0518"]);
    }

    #[test]
    fn diagnostic_codes_read_errors_only_and_skip_what_they_cannot_parse() {
        // A `warning CS####` line is routine under `/nostdlib+` and is not a
        // verdict; a diagnostic with no position prefix is what `csc` emits
        // for a whole-compilation complaint and does count; and a line that
        // says `error CS` without a number is not guessed at.
        let output = "warning CS8021: No value for RuntimeMetadataVersion found.\n\
                      error CS0518: Predefined type 'System.Object' is not defined\n\
                      note: error CSnope\n\
                      \n";
        assert_eq!(diagnostic_codes(output), ["CS0518"]);
        assert!(diagnostic_codes("").is_empty());
    }

    #[test]
    fn sdk_compilers_reads_the_list_sdks_listing() {
        // Both platform shapes, because CI runs both, and a `\r` because that
        // is what a Windows child process writes.
        let listing = "8.0.129 [/usr/lib/dotnet/sdk]\n9.0.100 [/usr/lib/dotnet/sdk]\r\n";
        let found = sdk_compilers(listing);
        assert_eq!(found.len(), 2);
        // Ascending, so the newest is last -- which is the one `locate` takes.
        assert!(found[0].ends_with(
            PathBuf::from("8.0.129")
                .join("Roslyn")
                .join("bincore")
                .join("csc.dll")
        ));
        assert!(found[1].starts_with("/usr/lib/dotnet/sdk"));
        assert!(found[1].ends_with(
            PathBuf::from("9.0.100")
                .join("Roslyn")
                .join("bincore")
                .join("csc.dll")
        ));
        // The Windows shape: a directory with spaces and backslashes, joined
        // as a path rather than concatenated as a string.
        let windows = sdk_compilers("8.0.404 [C:\\Program Files\\dotnet\\sdk]\n");
        assert_eq!(windows.len(), 1);
        assert!(
            windows[0]
                .to_string_lossy()
                .contains("C:\\Program Files\\dotnet\\sdk"),
            "{windows:?}"
        );
        // Anything not shaped `<version> [<directory>]` is skipped, not
        // guessed at: `dotnet` prints a paragraph of prose when no SDK is
        // installed at all.
        assert!(sdk_compilers("").is_empty());
        assert!(sdk_compilers("No SDKs were found.\n").is_empty());
        assert!(sdk_compilers("8.0.129 [/usr/lib/dotnet/sdk\n").is_empty());
    }

    #[test]
    fn difference_names_what_the_candidate_gained_and_lost() {
        let one = ["CS0246".to_string()];
        let other = ["CS1026".to_string()];
        assert_eq!(difference(&one, &other), ["+1 CS1026", "-1 CS0246"]);
        assert_eq!(difference(&[], &other), ["+1 CS1026"]);
        assert_eq!(difference(&one, &[]), ["-1 CS0246"]);
        // Counts, not just presence: losing one of two is a difference.
        let twice = ["CS0518".to_string(), "CS0518".to_string()];
        let once = ["CS0518".to_string()];
        assert_eq!(difference(&twice, &once), ["-1 CS0518"]);
        // Equal multisets have no difference at all, which is the accepted
        // case and the reason this is only ever rendered on rejection.
        assert!(difference(&twice, &twice).is_empty());
    }

    #[test]
    fn system_tools_locate_walks_the_candidates_in_order() {
        // A name that cannot be started is skipped, not fatal, and the first
        // one that starts wins -- which is the whole probe mechanism, run
        // against the real PATH.
        assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool"]), None);
        let found = SystemTools
            .locate(&["tokenpress-no-such-tool", "dotnet"])
            .expect("this machine must have a .NET SDK to run the suite");
        assert_eq!(found.program, "dotnet");
        // Discovered, never hardcoded: the version is whatever this machine
        // installed, and the path exists.
        assert!(found.csc.is_file(), "{found:?}");
        assert!(found.csc.ends_with("csc.dll"), "{found:?}");
        // ... and the constant the orchestration actually probes resolves.
        assert_eq!(SystemTools.locate(&DOTNET_NAMES), Some(found));
    }

    /// The real compiler, located once per test that needs it.
    fn system_compiler() -> Compiler {
        SystemTools
            .locate(&DOTNET_NAMES)
            .expect("this machine must have a .NET SDK to run the suite")
    }

    #[test]
    fn csc_writes_its_diagnostics_to_stdout() {
        // The measurement the whole design turns on, pinned against the real
        // binary: the diagnostics are on **stdout**, stderr is empty, and the
        // process exits non-zero for a file this gate deliberately accepts.
        // An exit-status gate -- the rule every other backend here uses --
        // would therefore refuse every real-world file, and a stderr-based one
        // would read nothing at all. This is the C# counterpart of the
        // `JAVA_TOOL_OPTIONS` measurement in `tokenpress-java`'s `external`.
        let compiler = system_compiler();
        let scratch = Scratch::new().unwrap();
        scratch.write(SELF_TEST_NAME, SELF_TEST_SOURCE).unwrap();
        let output = Command::new(&compiler.program)
            .arg(&compiler.csc)
            .args(CSC_ARGS)
            .arg(SELF_TEST_NAME)
            .current_dir(scratch.dir())
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr), "");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("error CS0246"), "{stdout}");
    }

    #[test]
    fn the_self_test_fixture_provokes_the_codes_the_gate_looks_for() {
        // Both halves of the self-test, against the real SDK. The distinct
        // codes are asserted and their counts are not: the counts are an
        // SDK-version detail (4 x CS0246, 6 x CS0518, 2 x CS0656 on 8.0.129),
        // while the codes are what proves `diagnostic_codes` still reads this
        // SDK's output -- and the equality is the cancellation the design
        // rests on.
        let compiler = system_compiler();
        let scratch = Scratch::new().unwrap();
        scratch.write(SELF_TEST_NAME, SELF_TEST_SOURCE).unwrap();
        scratch
            .write(SELF_TEST_MINIMIZED_NAME, SELF_TEST_MINIMIZED)
            .unwrap();
        let before = SystemTools
            .run(&compiler, scratch.dir(), SELF_TEST_NAME)
            .unwrap();
        let after = SystemTools
            .run(&compiler, scratch.dir(), SELF_TEST_MINIMIZED_NAME)
            .unwrap();
        let mut distinct = before.codes.clone();
        distinct.dedup();
        assert_eq!(distinct, SELF_TEST_CODES);
        assert_eq!(before.codes, after.codes);
        assert!(before.codes.len() > SELF_TEST_CODES.len());
    }

    #[test]
    fn the_self_test_pair_is_this_backends_own_output() {
        // The fixture and its minimization are two constants, so they could
        // drift apart from the emitter they are meant to represent. They
        // cannot: this runs the real formatter over the first and requires the
        // second.
        use std::path::Path;
        use tokenpress_core::{FormatOptions, Formatter};
        let produced = crate::CSharpFormatter::default()
            .format(
                Path::new("Probe.cs"),
                SELF_TEST_SOURCE,
                &FormatOptions::default(),
            )
            .unwrap();
        assert_eq!(produced.code, SELF_TEST_MINIMIZED);
    }

    #[test]
    fn system_tools_run_reports_the_diagnostic_multiset() {
        let compiler = system_compiler();
        let scratch = Scratch::new().unwrap();
        scratch.write("good.cs", VALID).unwrap();
        scratch.write("bad.cs", BROKEN).unwrap();
        // Even a perfectly good file has diagnostics under `/nostdlib+`, which
        // is the noise that cancels: three `CS0518`s here, one for the class's
        // `System.Object` base and one for each `System.Int32` in `F`'s
        // signature. Three, not one -- corrected against the real compiler.
        let good = SystemTools
            .run(&compiler, scratch.dir(), "good.cs")
            .unwrap();
        assert_eq!(good.codes, ["CS0518", "CS0518", "CS0518"]);
        let bad = SystemTools.run(&compiler, scratch.dir(), "bad.cs").unwrap();
        assert_eq!(bad.codes, ["CS1026"]);
        // Relative naming keeps the temp path out of what gets quoted back.
        assert!(bad.text.starts_with("bad.cs("), "{}", bad.text);
    }

    #[test]
    fn langversion_latest_accepts_current_syntax() {
        // `/langversion:latest` is passed rather than left to the SDK's
        // default, so that syntax this SDK understands is never reported as a
        // syntax error on one side of the comparison. A collection expression
        // is C# 12 syntax; under a language version that did not know it, it
        // would come back as a syntax diagnostic rather than as the ordinary
        // `/nostdlib+` noise.
        let compiler = system_compiler();
        let scratch = Scratch::new().unwrap();
        scratch
            .write("modern.cs", "class A\n{\n    int[] xs = [1, 2];\n}\n")
            .unwrap();
        let report = SystemTools
            .run(&compiler, scratch.dir(), "modern.cs")
            .unwrap();
        assert!(CSC_ARGS.contains(&"/langversion:latest"));
        for code in &report.codes {
            assert_eq!(code, "CS0518", "{}", report.text);
        }
    }

    #[test]
    fn csc_accepts_output_it_still_reads_the_same_way() {
        check_with(&SystemTools, VALID, FORMATTED).unwrap();
    }

    #[test]
    fn csc_accepts_output_whose_references_cannot_be_resolved() {
        // The row the whole design turns on, run through the orchestration
        // rather than through `SystemTools` alone: a file naming types nothing
        // provides is exactly what a real project's file looks like when it is
        // checked on its own, and its twelve diagnostics cancel exactly.
        check_with(&SystemTools, SELF_TEST_SOURCE, SELF_TEST_MINIMIZED).unwrap();
    }

    #[test]
    fn csc_rejects_output_with_an_introduced_syntax_error() {
        let err = check_with(&SystemTools, VALID, BROKEN).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("external check failed"), "{message}");
        assert!(message.contains("csc.dll"), "{message}");
        assert!(message.contains("+1 CS1026"), "{message}");
        assert!(message.contains(") expected"), "{message}");
    }

    #[test]
    fn csc_rejects_a_top_level_statement_after_a_type_declaration() {
        // The row that fails if anyone reintroduces a `CS1xxx` code-range
        // filter: this is as much a syntax error as a missing parenthesis, and
        // `csc` reports it as **CS8803**, outside that range.
        let err =
            check_with(&SystemTools, "class A { }\n", "class A { }\nint x = 1;\n").unwrap_err();
        let message = err.to_string();
        assert!(message.contains("+1 CS8803"), "{message}");
        assert!(
            message.contains("Top-level statements must precede"),
            "{message}"
        );
    }

    #[test]
    fn input_csc_already_rejects_does_not_fail_the_run() {
        // `99999999999999999999999` -- an integer literal no C# type can hold.
        // The grammar parses it happily; `csc` calls it
        // `CS1021: Integral constant is too large`, at every release, so this
        // stays a disagreement rather than ageing into agreement the way a
        // version-fragile preview-syntax fixture would. The complaint is
        // identical on both sides, so it cancels and the run succeeds.
        check_with(
            &SystemTools,
            "class Big\n{\n    long  x  =  99999999999999999999999;\n}\n",
            "class Big\n{\nlong x = 99999999999999999999999;\n}\n",
        )
        .unwrap();
    }

    #[test]
    fn check_runs_the_real_csc() {
        check(VALID, FORMATTED).unwrap();
        let err = check(VALID, BROKEN).unwrap_err();
        assert!(err.to_string().contains("external check failed"), "{err}");
    }

    #[test]
    fn scratch_directories_are_unique_and_removed_on_drop() {
        let first = Scratch::new().unwrap();
        let second = Scratch::new().unwrap();
        assert_ne!(first.dir, second.dir);
        first.write("A.cs", VALID).unwrap();
        assert_eq!(
            std::fs::read_to_string(first.dir().join("A.cs")).unwrap(),
            VALID
        );
        let dir = first.dir.clone();
        drop(first);
        assert!(!dir.exists());
    }
}
