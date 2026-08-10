//! External verification: the output is handed to Ruby itself, on top of the
//! built-in checks in [`crate::verify`].
//!
//! This is what [`tokenpress_core::VerifyLevel::External`] adds for this
//! backend. It never replaces [`crate::verify::equivalent`]; it runs after it.
//!
//! # The checker
//!
//! `ruby -c <file>` — the interpreter's own syntax check. `-c` compiles the
//! file and stops: nothing in it runs, `BEGIN`/`END` blocks included, so an
//! arbitrary user file can be handed to it without side effects. That is
//! exactly the property TokenPress needs — it rewrites whitespace, so it can
//! only ever break syntax, and it must not *execute* the code it is checking.
//!
//! The verdict is the **exit status and nothing else**: 0 for a file that
//! compiles, 1 for one that does not. `ruby -c` also writes `Syntax OK` to
//! stdout on success, and it writes warnings (`key :a is duplicated ...`) to
//! stderr for files that compile perfectly well, so neither stream can be
//! used as the signal — only what a file that failed to compile has to say
//! about *why* is read, and only to quote it back.
//!
//! When `ruby` is not on PATH, verification **fails**, naming it: silently
//! degrading to the built-in level would turn an explicit `--verify external`
//! into a weaker guarantee than the user asked for.
//!
//! # Already-broken input
//!
//! The checker is run over the **original** source first. If the original does
//! not pass, the formatted output is not checked at all and the level counts as
//! satisfied by [`crate::verify::equivalent`] alone: TokenPress must not be
//! blamed for a file the interpreter already rejected. The pinned prism and
//! the installed MRI are two different front ends with two different release
//! cadences, so this is a routine disagreement rather than an exotic one — a
//! syntax newer than the installed `ruby`, or a literal MRI compiles and prism
//! only parses (`/[z-a]/` is a well-formed regexp literal to prism and an
//! "empty range in char class" `SyntaxError` to MRI, which builds the regexp).
//! The run is not failed either — the user's input is not TokenPress's error.
//!
//! # Where the candidate goes
//!
//! Both the original and the candidate output are written to a **private temp
//! directory** (removed when [`Scratch`] drops), each under a `.rb` name. The
//! user's own path is never written to here — output that fails is discarded
//! by the caller and never reaches the destination file, which is the
//! project's core invariant.
//!
//! Unlike `tokenpress-js`, no extension has to be carried over from the target
//! path: prism has no dialect selector and neither does `ruby -c`, so one
//! fixed `.rb` name serves every path this backend claims — `Gemfile` and
//! `Rakefile` included, which have no extension to carry. The path is
//! therefore not a parameter of this module at all.
//!
//! # Text, not bytes
//!
//! Ruby sources need not be valid UTF-8, but this module takes `&str`: it is
//! handed exactly what [`crate::RubyFormatter::format`] was handed and what it
//! produced, and that signature is `&str` all the way up from
//! [`tokenpress_core::Formatter::format`]. A non-UTF-8 file never reaches this
//! crate at all; see the crate-level docs.
//!
//! # Cost
//!
//! One probe plus two `ruby` processes per file, so `--verify external` is
//! substantially slower than `--verify ast`. That is the price of an
//! independent opinion from the real interpreter.
//!
//! # Windows
//!
//! The bare name is enough. `tokenpress-js` has to probe `tsc.cmd` as well
//! because an npm-installed `tsc` is a batch shim that `CreateProcess` will
//! not start from the extensionless name; `ruby` has no such problem, since
//! every Windows Ruby (RubyInstaller, the `ruby/setup-ruby` action) puts a
//! real `ruby.exe` on PATH and `CreateProcess` appends `.exe` itself. The
//! candidate list is kept as a list anyway, because the probe is what decides
//! and a second name can be added without reshaping anything.
use std::fs;use std::io;use std::path::{Path,PathBuf};use std::process::{Command,Stdio};use std::sync::atomic::{AtomicU64,Ordering};use std::time::{SystemTime,UNIX_EPOCH};use tokenpress_core::{Error,Result};
/// The names probed for the interpreter, in order (see the module docs on
/// Windows for why there is only one).
const RUBY_NAMES:[&str;1]=["ruby"];
/// Syntax-only invocation: `-c` compiles the file and stops.
const RUBY_ARGS:[&str;1]=["-c"];
/// The file name the candidate is checked under. `original.rb` is its
/// counterpart; both are private to a [`Scratch`] directory.
const FORMATTED_NAME:&str="formatted.rb";const ORIGINAL_NAME:&str="original.rb";
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
/// The real interpreter: PATH lookup by spawning, checks by spawning.
struct SystemTools;impl Tools for SystemTools{fn locate(&self,candidates:&[&str])->Option<String>{candidates.iter().find(|name|{Command::new(name).arg("--version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok()}).map(|name|(*name).to_string())}fn run(&self,program:&str,args:&[&str],file:&Path)->io::Result<Outcome>{let output=Command::new(program).args(args).arg(file).stdin(Stdio::null()).output()?;if output.status.success(){return Ok(Outcome::Pass);}Ok(Outcome::Fail(String::from_utf8_lossy(&output.stderr).trim().to_string(),))}}
/// Runs `ruby -c` over `code`, the formatted output.
///
/// `original` is the source it was produced from: it is checked first, and a
/// `code` check only happens when the original passed (see the module docs).
pub fn check(original:&str,code:&str)->Result<()>{check_with(&SystemTools,original,code)}fn check_with(tools:&dyn Tools,original:&str,code:&str)->Result<()>{let program=tools.locate(&RUBY_NAMES).ok_or_else(| |{Error::Verification("external verification needs `ruby` on PATH: it was not found".to_string(),)})?;let scratch=Scratch::new()?;let before=scratch.write(ORIGINAL_NAME,original)?;if let Outcome::Fail(_)=tools.run(&program,&RUBY_ARGS,&before)?{return Ok(());}let after=scratch.write(FORMATTED_NAME,code)?;match tools.run(&program,&RUBY_ARGS,&after)?{Outcome::Pass=>Ok(()),Outcome::Fail(message)=>Err(Error::Verification(format!("external check failed ({program} -c): {message}"))),}}
/// A private temp directory, removed when it drops.
struct Scratch{dir:PathBuf,}
/// Distinguishes concurrent scratch directories inside one process; the pid
/// and the clock distinguish them across processes.
static SCRATCH_COUNTER:AtomicU64=AtomicU64::new(0);impl Scratch{fn new()->io::Result<Self>{let nanos=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();let dir=std::env::temp_dir().join(format!("tokenpress-verify-rb-{}-{nanos}-{}",std::process::id(),SCRATCH_COUNTER.fetch_add(1,Ordering::Relaxed)));fs::create_dir_all(&dir)?;Ok(Self{dir})}fn write(&self,name:&str,contents:&str)->io::Result<PathBuf>{let path=self.dir.join(name);fs::write(&path,contents)?;Ok(path)}}impl Drop for Scratch{fn drop(&mut self){let _=fs::remove_dir_all(&self.dir);}}#[cfg(test)]mod tests{use super::*;use std::cell::RefCell;use std::collections::VecDeque;
/// One recorded [`Tools::run`] call, including what the file it was
/// pointed at contained at the time.
#[derive(Clone,Debug)]struct Call{program:String,args:Vec<String>,file:PathBuf,contents:String,}
/// [`Tools`] with a scripted PATH and scripted verdicts, so the
/// orchestration can be tested without depending on what is installed on
/// the machine running the tests.
struct FakeTools{available:&'static[&'static str],verdicts:RefCell<VecDeque<io::Result<Outcome>>>,calls:RefCell<Vec<Call>>,}impl FakeTools{fn new(available:&'static[&'static str],verdicts:Vec<io::Result<Outcome>>)->Self{Self{available,verdicts:RefCell::new(verdicts.into()),calls:RefCell::new(Vec::new()),}}fn calls(&self)->Vec<Call>{self.calls.borrow().clone()}}impl Tools for FakeTools{fn locate(&self,candidates:&[&str])->Option<String>{candidates.iter().find(|name|self.available.contains(*name)).map(|name|(*name).to_string())}fn run(&self,program:&str,args:&[&str],file:&Path)->io::Result<Outcome>{self.calls.borrow_mut().push(Call{program:program.to_string(),args:args.iter().map(|a|(*a).to_string()).collect(),file:file.to_path_buf(),contents:std::fs::read_to_string(file).expect("the checker's input must exist"),});self.verdicts.borrow_mut().pop_front().expect("more checker runs than the test scripted")}}fn pass()->io::Result<Outcome>{Ok(Outcome::Pass)}fn fail(message:&str)->io::Result<Outcome>{Ok(Outcome::Fail(message.to_string()))}
/// Flattens an outcome so both variants are inspected the same way: the
/// empty string is a pass, anything else the checker's complaint.
fn verdict(outcome:Outcome)->String{match outcome{Outcome::Pass=>String::new(),Outcome::Fail(message)=>message,}}#[test]fn no_ruby_on_path_is_an_error_naming_the_tool(){let tools=FakeTools::new(&[],vec![]);let err=check_with(&tools,"x  =  1\n","x = 1\n").unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");let message=err.to_string();assert!(message.contains("ruby"),"{message}");assert!(message.contains("PATH"),"{message}");assert!(tools.calls().is_empty());}#[test]fn ruby_sees_the_original_first_and_then_the_output(){let tools=FakeTools::new(&["ruby"],vec![pass(),pass()]);check_with(&tools,"x  =  1\n","x = 1\n").unwrap();let calls=tools.calls();assert_eq!(calls.len(),2);for call in&calls{assert_eq!(call.program,"ruby");assert_eq!(call.args,["-c"]);assert_eq!(call.file.extension().unwrap(),"rb");assert!(call.file.starts_with(std::env::temp_dir()),"{call:?}");assert!(!call.file.exists(),"{call:?} outlived the check");}assert_eq!(calls[0].contents,"x  =  1\n");assert_eq!(calls[1].contents,"x = 1\n");assert_eq!(calls[0].file.parent(),calls[1].file.parent());assert_ne!(calls[0].file,calls[1].file);}#[test]fn output_that_fails_the_external_check_is_rejected(){let tools=FakeTools::new(&["ruby"],vec![pass(),fail("a.rb:1: syntax error")]);let err=check_with(&tools,"x  =  1\n","x =\n").unwrap_err();assert!(matches!(err,Error::Verification(_)),"{err}");let message=err.to_string();assert!(message.contains("ruby -c"),"{message}");assert!(message.contains("a.rb:1: syntax error"),"{message}");}#[test]fn an_original_that_already_fails_is_not_blamed_on_the_formatter(){let tools=FakeTools::new(&["ruby"],vec![fail("a.rb:1: empty range in char class")]);check_with(&tools,"x  =  /[z-a]/\n","x = /[z-a]/\n").unwrap();assert_eq!(tools.calls().len(),1);}#[test]fn a_checker_that_cannot_be_spawned_is_an_error(){let tools=FakeTools::new(&["ruby"],vec![Err(io::Error::other("spawn failed"))]);let err=check_with(&tools,"x  =  1\n","x = 1\n").unwrap_err();assert!(matches!(err,Error::Io(_)),"{err}");assert!(err.to_string().contains("spawn failed"),"{err}");}#[test]fn system_tools_locate_walks_the_candidates_in_order(){assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool","ruby"]),Some("ruby".to_string()));assert_eq!(SystemTools.locate(&["tokenpress-no-such-tool"]),None);assert_eq!(SystemTools.locate(&RUBY_NAMES),Some("ruby".to_string()));}#[test]fn system_tools_run_reports_rubys_verdict(){let scratch=Scratch::new().unwrap();let good=scratch.write("good.rb","x = 1\n").unwrap();let bad=scratch.write("bad.rb","x =\n").unwrap();assert_eq!(verdict(SystemTools.run("ruby",&RUBY_ARGS,&good).unwrap()),"");let message=verdict(SystemTools.run("ruby",&RUBY_ARGS,&bad).unwrap());assert!(message.contains("syntax error"),"{message}");}#[test]fn warnings_on_stderr_do_not_fail_the_check(){let scratch=Scratch::new().unwrap();let warns=scratch.write("warns.rb","h = {a: 1, a: 2}\n").unwrap();assert_eq!(verdict(SystemTools.run("ruby",&RUBY_ARGS,&warns).unwrap()),"");check_with(&SystemTools,"h  =  {a: 1, a: 2}\n","h = {a: 1, a: 2}\n").unwrap();}#[test]fn ruby_accepts_output_it_can_still_parse(){check_with(&SystemTools,"def add(a, b)\n    a + b\nend\n","def add(a, b)\na + b\nend\n",).unwrap();}#[test]fn ruby_rejects_output_it_cannot_parse(){let err=check_with(&SystemTools,"x  =  1\n","x =\n").unwrap_err();let message=err.to_string();assert!(message.contains("external check failed (ruby -c)"),"{message}");assert!(message.contains("syntax error"),"{message}");}#[test]fn input_ruby_already_rejects_does_not_fail_the_run(){check_with(&SystemTools,"x  =  /[z-a]/\n","x = /[z-a]/\n").unwrap();}#[test]fn check_runs_the_real_interpreter(){check("x  =  1\n","x = 1\n").unwrap();let err=check("x  =  1\n","x =\n").unwrap_err();assert!(err.to_string().contains("external check failed"),"{err}");}#[test]fn scratch_directories_are_unique_and_removed_on_drop(){let first=Scratch::new().unwrap();let second=Scratch::new().unwrap();assert_ne!(first.dir,second.dir);let path=first.write("a.rb","x = 1\n").unwrap();assert_eq!(std::fs::read_to_string(&path).unwrap(),"x = 1\n");let dir=first.dir.clone();drop(first);assert!(!dir.exists());}}