//! Verification for Rust output: re-parse with `syn` and compare token
//! streams structurally. Canonicalization goes through `syn::File`'s
//! `ToTokens` so lexing differences like `>>` vs `> >` are normalized for
//! ordinary code; macro bodies are compared token-by-token ignoring the
//! Joint/Alone spacing metadata, which whitespace changes legitimately alter
//! (`vec![1, -2]` vs `vec![1,-2]` carry identical tokens).

use proc_macro2::{TokenStream, TokenTree};
use quote::ToTokens;
use tokenpress_core::{Error, Result};

pub fn reparse(code: &str) -> Result<syn::File> {
    syn::parse_file(code)
        .map_err(|e| Error::Verification(format!("output failed to re-parse: {e}")))
}

fn stream_eq(a: TokenStream, b: TokenStream) -> bool {
    let a: Vec<TokenTree> = a.into_iter().collect();
    let b: Vec<TokenTree> = b.into_iter().collect();
    a.len() == b.len()
        && a.into_iter().zip(b).all(|(x, y)| match (x, y) {
            (TokenTree::Ident(x), TokenTree::Ident(y)) => x == y,
            (TokenTree::Literal(x), TokenTree::Literal(y)) => x.to_string() == y.to_string(),
            (TokenTree::Punct(x), TokenTree::Punct(y)) => x.as_char() == y.as_char(),
            (TokenTree::Group(x), TokenTree::Group(y)) => {
                x.delimiter() == y.delimiter() && stream_eq(x.stream(), y.stream())
            }
            _ => false,
        })
}

pub fn equivalent(original: &syn::File, code: &str) -> Result<()> {
    let reparsed = reparse(code)?;
    if !stream_eq(original.to_token_stream(), reparsed.to_token_stream()) {
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
    fn macro_body_spacing_changes_are_equivalent() {
        // Removing the space after `,` flips the comma's Joint/Alone spacing
        // metadata inside the macro body; the tokens themselves are equal.
        let file = syn::parse_file("fn f() { let v = vec![1, -2, |x| x]; }").unwrap();
        assert!(equivalent(&file, "fn f(){let v=vec![1,-2,|x|x];}").is_ok());
    }

    #[test]
    fn macro_body_token_changes_are_still_rejected() {
        let file = syn::parse_file("fn f() { m!(1, 2); }").unwrap();
        let err = equivalent(&file, "fn f(){m!(1,3);}").unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
        let err = equivalent(&file, "fn f(){m!(1);}").unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
        // Same length but a different kind of token at the same position.
        let err = equivalent(&file, "fn f(){m!(x,2);}").unwrap_err();
        assert!(err.to_string().contains("token stream differs"));
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
