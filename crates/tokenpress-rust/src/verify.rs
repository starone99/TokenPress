//! Verification for Rust output: re-parse with `syn` and compare canonical
//! token streams. Canonicalization goes through `syn::File`'s `ToTokens` so
//! lexing differences like `>>` vs `> >` are normalized on both sides.

use quote::ToTokens;
use tokenpress_core::{Error, Result};

pub fn reparse(code: &str) -> Result<syn::File> {
    syn::parse_file(code)
        .map_err(|e| Error::Verification(format!("output failed to re-parse: {e}")))
}

fn canonical(file: &syn::File) -> String {
    file.to_token_stream().to_string()
}

pub fn equivalent(original: &syn::File, code: &str) -> Result<()> {
    let reparsed = reparse(code)?;
    if canonical(original) != canonical(&reparsed) {
        return Err(Error::Verification(
            "output token stream differs from input".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reparse_accepts_valid_and_rejects_invalid_output() {
        assert!(reparse("fn f() {}").is_ok());
        let err = reparse("fn f( {").err().unwrap();
        assert!(err.to_string().contains("failed to re-parse"));
    }

    #[test]
    fn identical_token_streams_are_equivalent() {
        let file = syn::parse_file("fn f() -> u8 { 1 + 2 }").unwrap();
        assert!(equivalent(&file, "fn f()->u8{1+2}").is_ok());
    }

    #[test]
    fn glued_generics_normalize_to_the_same_stream() {
        let file = syn::parse_file("fn f() { let v: Vec<Vec<u8>> = Vec::new(); }").unwrap();
        assert!(equivalent(&file, "fn f(){let v:Vec<Vec<u8>> =Vec::new();}").is_ok());
    }

    #[test]
    fn changed_tokens_are_rejected() {
        let file = syn::parse_file("fn f() -> u8 { 1 }").unwrap();
        let err = equivalent(&file, "fn f()->u8{2}").unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
    }

    #[test]
    fn unparsable_output_is_rejected() {
        let file = syn::parse_file("fn f() {}").unwrap();
        assert!(equivalent(&file, "fn f( {").is_err());
    }
}
