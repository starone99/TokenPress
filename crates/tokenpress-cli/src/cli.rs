//! Library surface of the `tokenpress` binary.

use std::ffi::OsString;
use std::io::Write;

/// Runs the CLI and returns the process exit code.
/// Exit codes: 0 = success, 1 = `check` found changes, 2 = error.
pub fn run<I, T>(_args: I, out: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let _ = writeln!(out, "tokenpress: not yet implemented");
    2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_reports_unimplemented_for_now() {
        let mut out = Vec::new();
        let code = run(["tokenpress"], &mut out);
        assert_eq!(code, 2);
        assert!(String::from_utf8(out).unwrap().contains("tokenpress"));
    }
}
