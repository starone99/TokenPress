//! External verification: the output is handed to `javac` itself, on top of
//! the built-in checks in `tokenpress_treesitter::verify`.
//!
//! This is what [`tokenpress_core::VerifyLevel::External`] adds for this
//! backend. It never replaces the equivalence check; it runs after it.
//!
//! # The checker
//!
//! `javac -XDshould-stop.ifNoError=PARSE <file>` — the compiler's own front
//! end, stopped after the parse phase. Nothing is resolved, attributed,
//! generated or run, no class file is written and no type outside the file is
//! loaded, so an arbitrary user file can be handed to it without side effects
//! and without a classpath, a module path or a build.
//!
//! That flag is what makes the checker usable at all. A plain `javac` is a
//! *compiler*: a file whose imports name types that are not on the classpath
//! is `error: package … does not exist`, which is true of virtually every file
//! of a real project checked on its own. Stopped at `PARSE`, the same file
//! exits **0** — measured on javac 21.0.10, a file importing
//! `com.example.nope.Missing` and declaring a field of that type is exit 0
//! under the flag and exit 1 without it. The whole design turns on that row.
//!
//! The verdict is the **exit status and nothing else**: 0 for a file that
//! parses, 1 for one that does not. Reading stderr instead would be actively
//! wrong, not merely redundant — the JVM launcher prints
//! `Picked up JAVA_TOOL_OPTIONS: …` to stderr on **every** invocation,
//! successful ones included, wherever that variable is set (this project's own
//! development container is one such machine). A stderr-based gate would
//! reject every file there. Only what a file that failed to parse has to say
//! about *why* is read, and only to quote it back.
//!
//! When `javac` is not on PATH, verification **fails**, naming it: silently
//! degrading to the built-in level would turn an explicit `--verify external`
//! into a weaker guarantee than the user asked for.
//!
//! # The self-test
//!
//! `-XD` options are javac's internal, undocumented namespace, and an
//! unrecognised key is **silently ignored** rather than rejected: measured on
//! javac 21.0.10, `-XDbogus.key.here=PARSE` over the unresolvable-import file
//! above exits **1**, because javac quietly ran a full compile. A JDK that
//! renamed the option would therefore not break loudly — it would downgrade
//! this gate to a whole-program compile and start refusing perfectly good
//! output, blaming TokenPress for a missing classpath.
//!
//! So the gate is tested before it is trusted. [`SELF_TEST_SOURCE`] is valid
//! Java whose single import cannot be resolved; it is run through the gate
//! first, and anything but a pass fails the run with a message naming
//! `-XDshould-stop.ifNoError=PARSE` explicitly. The self-test runs once per
//! [`check`] rather than once per process on purpose: a cached verdict would
//! be global mutable state whose value depended on which file happened to be
//! checked first, which is not a trade this module is willing to make for one
//! process spawn.
//!
//! # Already-broken input
//!
//! The checker is run over the **original** source first. If the original does
//! not pass, the formatted output is not checked at all and the level counts
//! as satisfied by the built-in equivalence check alone: TokenPress must not
//! be blamed for a file the toolchain already rejected. The run is not failed
//! either — the user's input is not TokenPress's error.
//!
//! tree-sitter-java and javac are two front ends that disagree in both
//! directions, and the disagreements that matter here are the ones the grammar
//! is *more* permissive about: `class A { long x = 99999999999; }` is a clean
//! tree-sitter parse and `error: integer number too large` to javac, and the
//! version-skew family (preview syntax the grammar knows and the installed JDK
//! does not) behaves the same way. Neither is TokenPress's doing.
//!
//! # Where the candidate goes
//!
//! The self-test fixture, the original and the candidate output are written to
//! a **private temp directory** (removed when the private `Scratch` guard
//! drops), each under a fixed `.java` name. The user's own path is never
//! written to here — output that fails is discarded by the caller and never
//! reaches the destination file, which is the project's core invariant.
//!
//! Fixed names are safe despite Java's public-class/filename rule. That rule
//! (JLS 7.6) is a **semantic** check, and this gate stops before the phase
//! that performs it: measured, a `formatted.java` containing
//! `public class Hello { … }` exits 0 under the flag. Nothing has to be
//! parsed out of the source to name the file after its public class, which is
//! just as well — that would mean reimplementing a Java parser to drive a Java
//! parser.
//!
//! Unlike `tokenpress-js`, no extension has to be carried over from the target
//! path: `.java` is the only extension this backend claims (see
//! [`crate::paths`]) and the gate has no dialect selector, so one fixed name
//! serves every path. Neither is the *directory* a parameter: nothing is
//! resolved at this phase, so a file checked outside its source tree parses no
//! less successfully than one checked inside it.
//!
//! # Text, not bytes
//!
//! Java source has no fixed encoding, but this module takes `&str`: it is
//! handed exactly what [`crate::JavaFormatter::format`] was handed and what it
//! produced, and that signature is `&str` all the way up from
//! [`tokenpress_core::Formatter::format`]. A non-UTF-8 file never reaches this
//! crate at all; see the crate-level docs. What is written here is therefore
//! UTF-8, which is also what javac reads under a UTF-8 platform charset — and
//! the gate never reaches a phase where a `\uXXXX` escape's meaning could
//! differ, so the encoding question is the read-time one and no more.
//!
//! # Cost
//!
//! One probe plus **three** `javac` processes per file — the self-test, the
//! original, the candidate — each paying JVM startup. `--verify external` is
//! therefore far slower here than `--verify ast`, more so than for any other
//! backend. That is the price of an independent opinion from the real
//! compiler, and of knowing the gate is still the gate.
//!
//! # Windows
//!
//! The bare name is enough. `tokenpress-js` has to probe `tsc.cmd` as well
//! because an npm-installed `tsc` is a batch shim that `CreateProcess` will
//! not start from the extensionless name; `javac` has no such problem, since
//! every Windows JDK (the installer, the zip, `actions/setup-java`) ships a
//! real `javac.exe` in `JAVA_HOME/bin` and `CreateProcess` appends `.exe`
//! itself. The candidate list is kept as a list anyway, because the probe is
//! what decides and a second name can be added without reshaping anything.
use std::fs;use std::io;use std::path::{Path,PathBuf};use std::process::{Command,Stdio};use std::sync::atomic::{AtomicU64,Ordering};use std::time::{SystemTime,UNIX_EPOCH};use tokenpress_core::{Error,Result};
/// The names probed for the compiler, in order (see the module docs on Windows
/// for why there is only one).
const JAVAC_NAMES:[&str;1]=["javac"];
/// What the probe passes. `javac -version` prints the version and exits 0; see
/// the module docs, and `the_probe_is_the_version_flag`, for why neither `-h`
/// nor a bare invocation can be used — both exit 2.
const JAVAC_PROBE_ARGS:[&str;1]=["-version"];
/// Parse-only invocation: stop after the parse phase, before anything is
/// resolved. The self-test below is what keeps this from silently becoming a
/// full compile.
const JAVAC_ARGS:[&str;1]=["-XDshould-stop.ifNoError=PARSE"];
/// The file names the fixtures are checked under; all three are private to a
/// `Scratch` directory.
const FORMATTED_NAME:&str="formatted.java";const ORIGINAL_NAME:&str="original.java";const SELF_TEST_NAME:&str="selftest.java";
/// Valid Java whose single import cannot be resolved by any classpath.
///
/// It is exit 0 under [`JAVAC_ARGS`] and exit 1 under a full compile, so it
/// separates the parse-only gate from the compiler underneath it. See the
/// module docs on the self-test for why that has to be checked at run time.
const SELF_TEST_SOURCE:&str="\
import com.example.tokenpress.absent.Missing;

class TokenPressParseGateSelfTest {
    Missing field;
}
";
/// What one checker run concluded.
#[derive(Debug)]enum Outcome{Pass,
/// The checker rejected the file; carries its own diagnostics.
Fail(String),}
/// The seam between the orchestration and the processes it drives, so the
/// orchestration can be tested without depending on what is installed on the
/// machine running the tests.
trait Tools{
/// Returns the first of `candidates` that can actually be started, or
/// `None` when none of them can.
fn locate(&self,candidates:&[&str])->Option<String>;
/// Runs `program args... file` and reports its verdict. `Err` means the
/// process could not be run at all.
fn run(&self,program:&str,args:&[&str],file:&Path)->io::Result<Outcome>;}
/// The real compiler: PATH lookup by spawning, checks by spawning.
struct SystemTools;impl Tools for SystemTools{fn locate(&self,candidates:&[&str])->Option<String>{candidates.iter().find(|name|{Command::new(name).args(JAVAC_PROBE_ARGS).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()}).map(|name|(*name).to_string())}fn run(&self,program:&str,args:&[&str],file:&Path)->io::Result<Outcome>{let output=Command::new(program).args(args).arg(file).stdin(Stdio::null()).output()?;if output.status.success(){return Ok(Outcome::Pass);}Ok(Outcome::Fail(String::from_utf8_lossy(&output.stderr).trim().to_string(),))}}
/// Runs javac's parse-only gate over `code`, the formatted output.
///
/// `original` is the source it was produced from: it is checked first, and a
/// `code` check only happens when the original passed (see the module docs).
pub fn check(original:&str,code:&str)->Result<()>{check_with(&SystemTools,original,code)}fn check_with(tools:&dyn Tools,original:&str,code:&str)->Result<()>{let program=tools.locate(&JAVAC_NAMES).ok_or_else(| |{Error::Verification("external verification needs `javac` on PATH: it was not found".to_string(),)})?;let scratch=Scratch::new()?;let fixture=scratch.write(SELF_TEST_NAME,SELF_TEST_SOURCE)?;if let Outcome::Fail(message)=tools.run(&program,&JAVAC_ARGS,&fixture)?{return Err(Error::Verification(format!("external verification cannot trust `{program} -XDshould-stop.ifNoError=PARSE`: \
             its self-test file — valid Java whose one import no classpath can resolve — was \
             rejected, so this JDK is no longer stopping after the parse phase and the check \
             would be a whole-program compile: {message}")));}let before=scratch.write(ORIGINAL_NAME,original)?;if let Outcome::Fail(_)=tools.run(&program,&JAVAC_ARGS,&before)?{return Ok(());}let after=scratch.write(FORMATTED_NAME,code)?;match tools.run(&program,&JAVAC_ARGS,&after)?{Outcome::Pass=>Ok(()),Outcome::Fail(message)=>Err(Error::Verification(format!("external check failed ({program} -XDshould-stop.ifNoError=PARSE): {message}"))),}}
/// A private temp directory, removed when it drops.
struct Scratch{dir:PathBuf,}
/// Distinguishes concurrent scratch directories inside one process; the pid
/// and the clock distinguish them across processes.
static SCRATCH_COUNTER:AtomicU64=AtomicU64::new(0);impl Scratch{fn new()->io::Result<Self>{let nanos=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();let dir=std::env::temp_dir().join(format!("tokenpress-verify-java-{}-{nanos}-{}",std::process::id(),SCRATCH_COUNTER.fetch_add(1,Ordering::Relaxed)));fs::create_dir_all(&dir)?;Ok(Self{dir})}fn write(&self,name:&str,contents:&str)->io::Result<PathBuf>{let path=self.dir.join(name);fs::write(&path,contents)?;Ok(path)}}impl Drop for Scratch{fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}#[cfg(test)]mod tests{use super::*;use std::cell::RefCell;use std::collections::VecDeque;
/// One recorded [`Tools::run`] call, including what the file it was
/// pointed at contained at the time.
#[derive(Clone,Debug)]struct Call{program:String,args:Vec<String>,file:PathBuf,contents:String,}
/// [`Tools`] with a scripted PATH and scripted verdicts, so the
/// orchestration can be tested without depending on what is installed on
/// the machine running the tests.
struct FakeTools{available:&'static[&'static str],verdicts:RefCell<VecDeque<io::Result<Outcome>>>,calls:RefCell<Vec<Call>>,}impl FakeTools{fn new(available:&'static[&'static str],verdicts:Vec<io::Result<Outcome>>)->Self{Self{available,verdicts:RefCell::new(verdicts.into()),calls:RefCell::new(Vec::new()),}}fn calls(&self)->Vec<Call>{self.calls.borrow().clone()}}impl Tools for FakeTools{fn locate(&self,candidates:&[&str])->Option<String>{candidates.iter().find(|name|self.available.contains(*name)).map(|name|(*name).to_string())}fn run(&self,program:&str,args:&[&str],file:&Path)->io::Result<Outcome>{self.calls.borrow_mut().push(Call{program:program.to_string(),args:args.iter().map(|a|(*a).to_string()).collect(),file:file.to_path_buf(),contents:std::fs::read_to_string(file).expect("the checker's input must exist"),});self.verdicts.borrow_mut().pop_front().expect("more checker runs than the test scripted")}}fn pass()->io::Result<Outcome>{Ok(Outcome::Pass)}fn fail(message:&str)->io::Result<Outcome>{Ok(Outcome::Fail(message.to_string()))}
/// Flattens an outcome so both variants are inspected the same way: the
/// empty string is a pass, anything else the checker's complaint.
fn verdict(outcome:Outcome)->String{match outcome{Outcome::Pass=>String::new(),Outcome::Fail(message)=>message,}}
/// A public class deliberately not named after the file it is written to,
/// because that is the arrangement the gate has to tolerate.
const VALID:&str="public class A {\n\n    int f(int a) {\n        return a;\n    }\n}\n";const FORMATTED:&str="public class A {\nint f(int a) {\nreturn a;\n}\n}\n";
/// tree-sitter parses it; javac calls it `error: reached end of file while
/// parsing`.
const BROKEN:&str="public class A {\n    int f(int a) {\n";#[test]fn no_javac_on_path_is_an_error_naming_the_tool(){let tools=FakeTools::new(&[],vec![]);let err=check_with(&tools,VALID,FORMATTED).unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");let message=err.to_string();assert!(message.contains("javac"),"{message}");assert!(message.contains("PATH"),"{message}");assert!(tools.calls().is_empty());}#[test]fn javac_sees_the_self_test_then_the_original_then_the_output(){let tools=FakeTools::new(&["javac"],vec![pass(),pass(),pass()]);check_with(&tools,VALID,FORMATTED).unwrap();let calls=tools.calls();assert_eq!(calls.len(),3);for call in&calls{assert_eq!(call.program,"javac");assert_eq!(call.args,["-XDshould-stop.ifNoError=PARSE"]);assert_eq!(call.file.extension().unwrap(),"java");assert!(call.file.starts_with(std::env::temp_dir()),"{call:?}");assert!(!call.file.exists(),"{call:?} outlived the check");assert_eq!(call.file.parent(),calls[0].file.parent());}assert_eq!(calls[0].contents,SELF_TEST_SOURCE);assert_eq!(calls[1].contents,VALID);assert_eq!(calls[2].contents,FORMATTED);assert_ne!(calls[0].file,calls[1].file);assert_ne!(calls[1].file,calls[2].file);}#[test]fn a_self_test_the_jdk_fails_stops_the_gate_naming_the_option(){let tools=FakeTools::new(&["javac"],vec![fail("selftest.java:1: error: package com.example does not exist",)],);let err=check_with(&tools,VALID,FORMATTED).unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");let message=err.to_string();assert!(message.contains("-XDshould-stop.ifNoError=PARSE"),"{message}");assert!(message.contains("self-test"),"{message}");assert!(message.contains("does not exist"),"{message}");assert_eq!(tools.calls().len(),1);}#[test]fn output_that_fails_the_external_check_is_rejected(){let tools=FakeTools::new(&["javac"],vec![pass(),pass(),fail("formatted.java:2: error: reached end of file while parsing"),],);let err=check_with(&tools,VALID,BROKEN).unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");let message=err.to_string();assert!(message.contains("javac -XDshould-stop.ifNoError=PARSE"),"{message}");assert!(message.contains("reached end of file while parsing"),"{message}");}#[test]fn an_original_that_already_fails_is_not_blamed_on_the_formatter(){let tools=FakeTools::new(&["javac"],vec![pass(),fail("original.java:1: error: integer number too large"),],);check_with(&tools,"class A {\n    long x = 99999999999;\n}\n","class A {\nlong x = 99999999999;\n}\n",).unwrap();assert_eq!(tools.calls().len(),2);}#[test]fn a_checker_that_cannot_be_spawned_is_an_error(){let tools=FakeTools::new(&["javac"],vec![Err(io::Error::other("spawn failed"))]);let err=check_with(&tools,VALID,FORMATTED).unwrap_err();assert!(matches!(err,Error::Io(_)),"{err}");assert!(err.to_string().contains("spawn failed"),"{err}");}#[test]fn the_probe_is_the_version_flag(){assert_eq!(JAVAC_PROBE_ARGS,["-version"]);assert!(!JAVAC_PROBE_ARGS.is_empty());}#[test]fn system_tools_locate_walks_the_candidates_in_order(){assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool","javac"]),Some("javac".to_string()));assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool"]),None);assert_eq!(SystemTools.locate(&JAVAC_NAMES),Some("javac".to_string()));}#[test]fn system_tools_run_reports_javacs_verdict(){let scratch=Scratch::new().unwrap();let good=scratch.write("good.java",VALID).unwrap();let bad=scratch.write("bad.java",BROKEN).unwrap();assert_eq!(verdict(SystemTools.run("javac",&JAVAC_ARGS,&good).unwrap()),"");let message=verdict(SystemTools.run("javac",&JAVAC_ARGS,&bad).unwrap());assert!(message.contains("reached end of file while parsing"),"{message}");}#[test]fn a_public_class_need_not_be_named_after_its_file(){let scratch=Scratch::new().unwrap();let mismatched=scratch.write(FORMATTED_NAME,FORMATTED).unwrap();assert_eq!(verdict(SystemTools.run("javac",&JAVAC_ARGS,&mismatched).unwrap()),"");}#[test]fn the_self_test_fixture_passes_the_gate_and_fails_a_full_compile(){let scratch=Scratch::new().unwrap();let fixture=scratch.write(SELF_TEST_NAME,SELF_TEST_SOURCE).unwrap();assert_eq!(verdict(SystemTools.run("javac",&JAVAC_ARGS,&fixture).unwrap()),"");let full=verdict(SystemTools.run("javac",&[],&fixture).unwrap());assert!(full.contains("does not exist"),"{full}");}#[test]fn what_javac_prints_to_stderr_is_not_the_verdict(){let scratch=Scratch::new().unwrap();let good=scratch.write("good.java",VALID).unwrap();let noisy=Command::new("javac").args(JAVAC_ARGS).arg(&good).env("JAVA_TOOL_OPTIONS","-Dtokenpress.stderr.probe=1").stdin(Stdio::null()).output().unwrap();assert!(noisy.status.success());assert!(!noisy.stderr.is_empty(),"the launcher was expected to announce JAVA_TOOL_OPTIONS on stderr");assert_eq!(verdict(SystemTools.run("javac",&JAVAC_ARGS,&good).unwrap()),"");}#[test]fn javac_accepts_output_it_can_still_parse(){check_with(&SystemTools,VALID,FORMATTED).unwrap();}#[test]fn javac_accepts_output_whose_imports_cannot_be_resolved(){check_with(&SystemTools,"import com.example.nope.Missing;\n\npublic class A {\n    Missing m;\n}\n","import com.example.nope.Missing;\npublic class A {\nMissing m;\n}\n",).unwrap();}#[test]fn javac_rejects_output_it_cannot_parse(){let err=check_with(&SystemTools,VALID,BROKEN).unwrap_err();let message=err.to_string();assert!(message.contains("external check failed (javac -XDshould-stop.ifNoError=PARSE)"),"{message}");assert!(message.contains("reached end of file while parsing"),"{message}");}#[test]fn input_javac_already_rejects_does_not_fail_the_run(){check_with(&SystemTools,"class A {\n    long x = 99999999999;\n}\n","class A {\nlong x = 99999999999;\n}\n",).unwrap();}#[test]fn check_runs_the_real_javac(){check(VALID,FORMATTED).unwrap();let err=check(VALID,BROKEN).unwrap_err();assert!(err.to_string().contains("external check failed"),"{err}");}#[test]fn scratch_directories_are_unique_and_removed_on_drop(){let first=Scratch::new().unwrap();let second=Scratch::new().unwrap();assert_ne!(first.dir,second.dir);let path=first.write("A.java",VALID).unwrap();assert_eq!(std::fs::read_to_string(&path).unwrap(),VALID);let dir=first.dir.clone();drop(first);assert!(!dir.exists());}}