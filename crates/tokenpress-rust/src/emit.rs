//! Token-stream renderer for Rust. Emits the parsed token stream with the
//! minimum whitespace that still lexes identically (RS01, RS02), and handles
//! doc-comment attributes (RSO1).

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::ToTokens;

/// Character pairs that would lex as (the start of) one operator if glued.
const GLUE_PAIRS: &[(char, char)] = &[
    ('&', '&'),
    ('|', '|'),
    (':', ':'),
    ('=', '='),
    ('=', '>'),
    ('!', '='),
    ('<', '='),
    ('>', '='),
    ('-', '>'),
    ('<', '<'),
    ('.', '.'),
    // `128.. =>` glued would lex as `..=` + `>`.
    ('.', '='),
    ('+', '='),
    ('-', '='),
    ('*', '='),
    ('/', '='),
    ('%', '='),
    ('^', '='),
    ('&', '='),
    ('|', '='),
    // A `/` glued onto `/` or `*` would start a comment.
    ('/', '/'),
    ('/', '*'),
];

#[derive(Clone, Copy, PartialEq)]
enum Prev {
    /// Start of output or just after a newline.
    Start,
    Ident(char),
    Literal(char),
    Punct {
        ch: char,
        joint: bool,
    },
    Close(char),
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn needs_space(prev: Prev, first: char, prev_is_ident: bool) -> bool {
    let last = match prev {
        Prev::Start => return false,
        Prev::Punct { joint: true, .. } => return false,
        Prev::Ident(c) | Prev::Literal(c) | Prev::Punct { ch: c, .. } | Prev::Close(c) => c,
    };
    if is_word(last) && is_word(first) {
        return true;
    }
    if GLUE_PAIRS.contains(&(last, first)) {
        return true;
    }
    // A literal ending in a quote or raw-string `#` glued onto a word would
    // lex as a suffixed literal: `extern "system"fn` is one token.
    if matches!(prev, Prev::Literal(_)) && matches!(last, '"' | '\'' | '#') && is_word(first) {
        return true;
    }
    // An identifier glued onto a quote could form a prefixed literal
    // (`b'x'`, `r"x"`) or swallow a lifetime/label (`break'a`).
    prev_is_ident && matches!(first, '"' | '\'')
}

/// `#[doc = "..."]` / `#![doc = "..."]` recognized at `trees[0]`:
/// returns (string literal text, is_inner, trees consumed).
fn match_doc_attr(trees: &[TokenTree]) -> Option<(String, bool, usize)> {
    let TokenTree::Punct(hash) = trees.first()? else {
        return None;
    };
    if hash.as_char() != '#' {
        return None;
    }
    let (inner, group_idx) = match trees.get(1)? {
        TokenTree::Punct(bang) if bang.as_char() == '!' => (true, 2),
        _ => (false, 1),
    };
    let TokenTree::Group(group) = trees.get(group_idx)? else {
        return None;
    };
    if group.delimiter() != Delimiter::Bracket {
        return None;
    }
    let parts: Vec<TokenTree> = group.stream().into_iter().collect();
    match parts.as_slice() {
        [TokenTree::Ident(name), TokenTree::Punct(eq), TokenTree::Literal(lit)]
            if name == "doc" && eq.as_char() == '=' =>
        {
            Some((lit.to_string(), inner, group_idx + 1))
        }
        _ => None,
    }
}

/// A line qualifies for the `///` form only when it round-trips exactly: a
/// plain `"..."` literal with no escapes and no interior quote handling.
/// Qualifying is per line, but the choice is made per block (`doc_block`).
fn doc_comment_line(lit: &str, inner: bool) -> Option<String> {
    let body = lit.strip_prefix('"')?.strip_suffix('"')?;
    if body.contains('\\') || body.contains('"') {
        return None;
    }
    let marker = if inner { "//!" } else { "///" };
    Some(format!("{marker}{body}\n"))
}

/// The contiguous run of doc attributes of the same kind (all outer or all
/// inner — an inner/outer boundary ends the run) starting at `trees[0]`:
/// returns (trees consumed, sugared lines). The lines are `Some` only when
/// *every* line of the block qualifies for the `///` / `//!` form; one raw
/// line forces the whole block raw, because rustdoc strips the conventional
/// leading space from sugared fragments but keeps it on raw ones, so a mixed
/// block would misindent the doc example it reconstructs.
fn doc_block(trees: &[TokenTree]) -> Option<(usize, Option<Vec<String>>)> {
    let (lit, inner, consumed) = match_doc_attr(trees)?;
    let mut used = consumed;
    let mut lines = doc_comment_line(&lit, inner).map(|line| vec![line]);
    while let Some((lit, next_inner, consumed)) = match_doc_attr(&trees[used..]) {
        if next_inner != inner {
            break;
        }
        used += consumed;
        match (lines.as_mut(), doc_comment_line(&lit, inner)) {
            (Some(lines), Some(line)) => lines.push(line),
            _ => lines = None,
        }
    }
    Some((used, lines))
}

/// Removes every `#[doc = <lit>]` / `#![doc = <lit>]` attribute (RSO1).
/// Other doc attributes such as `#[doc(hidden)]` are kept.
pub fn strip_doc_attrs(ts: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = ts.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::new();
    let mut i = 0;
    while i < trees.len() {
        if let Some((_, _, consumed)) = match_doc_attr(&trees[i..]) {
            i += consumed;
            continue;
        }
        match &trees[i] {
            TokenTree::Group(g) => {
                let mut rebuilt =
                    proc_macro2::Group::new(g.delimiter(), strip_doc_attrs(g.stream()));
                rebuilt.set_span(g.span());
                out.push(TokenTree::Group(rebuilt));
            }
            other => out.push(other.clone()),
        }
        i += 1;
    }
    out.into_iter().collect()
}

pub fn render(file: &syn::File) -> String {
    let mut out = String::new();
    let mut prev = Prev::Start;
    render_seq(
        &file.to_token_stream().into_iter().collect::<Vec<_>>(),
        &mut out,
        &mut prev,
    );
    out
}

fn push_text(out: &mut String, prev: &mut Prev, text: &str, kind: fn(char) -> Prev) {
    let first = text.chars().next().unwrap_or('\0');
    if needs_space(*prev, first, matches!(prev, Prev::Ident(_))) {
        out.push(' ');
    }
    out.push_str(text);
    *prev = kind(text.chars().next_back().unwrap_or('\0'));
}

fn render_tree(tree: &TokenTree, out: &mut String, prev: &mut Prev) {
    match tree {
        TokenTree::Ident(id) => push_text(out, prev, &id.to_string(), Prev::Ident),
        TokenTree::Literal(lit) => push_text(out, prev, &lit.to_string(), Prev::Literal),
        TokenTree::Punct(p) => {
            let ch = p.as_char();
            if needs_space(*prev, ch, matches!(prev, Prev::Ident(_))) {
                out.push(' ');
            }
            out.push(ch);
            *prev = Prev::Punct {
                ch,
                joint: p.spacing() == Spacing::Joint,
            };
        }
        TokenTree::Group(g) => {
            let inner: Vec<TokenTree> = g.stream().into_iter().collect();
            match g.delimiter() {
                Delimiter::None => render_seq(&inner, out, prev),
                d => {
                    let (open, close) = match d {
                        Delimiter::Parenthesis => ('(', ')'),
                        Delimiter::Brace => ('{', '}'),
                        _ => ('[', ']'),
                    };
                    // No spacing rule can require a space before an open
                    // delimiter: it is neither a word char, a quote, nor
                    // the second half of any glueable operator pair.
                    out.push(open);
                    *prev = Prev::Punct {
                        ch: open,
                        joint: false,
                    };
                    render_seq(&inner, out, prev);
                    out.push(close);
                    *prev = Prev::Close(close);
                }
            }
        }
    }
}

fn render_seq(trees: &[TokenTree], out: &mut String, prev: &mut Prev) {
    let mut i = 0;
    while i < trees.len() {
        if let Some((consumed, lines)) = doc_block(&trees[i..]) {
            match lines {
                Some(lines) => {
                    for line in lines {
                        if !out.is_empty() && !out.ends_with('\n') {
                            out.push('\n');
                        }
                        out.push_str(&line);
                    }
                    *prev = Prev::Start;
                }
                // The whole block keeps its original attribute form.
                None => {
                    for tree in &trees[i..i + consumed] {
                        render_tree(tree, out, prev);
                    }
                }
            }
            i += consumed;
            continue;
        }
        render_tree(&trees[i], out, prev);
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_src(src: &str) -> String {
        render(&syn::parse_file(src).unwrap())
    }

    #[test]
    fn words_are_separated_operators_are_not() {
        assert_eq!(
            render_src("fn add(a: i32, b: i32) -> i32 { a + b }"),
            "fn add(a:i32,b:i32)->i32{a+b}"
        );
    }

    #[test]
    fn nested_generics_glue_but_gt_eq_does_not() {
        assert_eq!(
            render_src("fn f() { let v: Vec<Vec<u8>> = Vec::new(); }"),
            "fn f(){let v:Vec<Vec<u8>> =Vec::new();}"
        );
    }

    #[test]
    fn and_deref_glues_but_double_ref_keeps_a_space() {
        // `&*` is not an operator, so gluing is safe...
        assert_eq!(
            render_src("fn f(a: u8, b: &u8) -> u8 { a & *b }"),
            "fn f(a:u8,b:&u8)->u8{a&*b}"
        );
        // ...but `& &` glued could become the `&&` operator, so a space is
        // kept — even for `&&u8`, which syn re-emits as two `&` tokens.
        assert_eq!(
            render_src("fn f(a: &u8, b: &&u8) -> bool { a == *b }"),
            "fn f(a:&u8,b:& &u8)->bool{a==*b}"
        );
        assert!(needs_space(
            Prev::Punct {
                ch: '&',
                joint: false
            },
            '&',
            false
        ));
    }

    #[test]
    fn joint_operators_stay_glued() {
        assert_eq!(
            render_src("fn f(a: u32) -> u32 { a << 2 }"),
            "fn f(a:u32)->u32{a<<2}"
        );
        assert_eq!(
            render_src("fn f() { for _ in 0..=3 {} }"),
            "fn f(){for _ in 0..=3{}}"
        );
    }

    #[test]
    fn open_range_pattern_before_fat_arrow_keeps_a_space() {
        assert_eq!(
            render_src("fn f(x: u8) -> u8 { match x { 128.. => 1, _ => 2 } }"),
            "fn f(x:u8)->u8{match x{128.. =>1,_=>2}}"
        );
    }

    #[test]
    fn lifetimes_survive() {
        assert_eq!(
            render_src("fn f<'a>(x: &'a str) -> &'a str { x }"),
            "fn f<'a>(x:&'a str)->&'a str{x}"
        );
    }

    #[test]
    fn macro_call_tokens_are_preserved() {
        assert_eq!(
            render_src("fn main() { println!(\"{} x\", 1); }"),
            "fn main(){println!(\"{} x\",1);}"
        );
    }

    #[test]
    fn doc_comments_are_reemitted_as_line_comments() {
        assert_eq!(
            render_src("/// Adds one.\nfn f() {}"),
            "/// Adds one.\nfn f(){}"
        );
    }

    #[test]
    fn inner_doc_comments_use_bang_form() {
        assert_eq!(
            render_src("//! Module.\nfn f() {}"),
            "//! Module.\nfn f(){}"
        );
    }

    #[test]
    fn escaped_doc_content_falls_back_to_attribute_form() {
        let out = render_src("#[doc = \"say \\\"hi\\\"\"]\nfn f() {}");
        assert!(out.starts_with("#[doc="));
    }

    #[test]
    fn a_doc_block_with_one_escaped_line_is_emitted_entirely_raw() {
        // rustdoc unindents sugared and raw fragments differently, so the
        // whole contiguous block has to agree on one form.
        let out = render_src(
            "/// Read patterns.\n/// ```\n#[doc = \" let s = \\\"a\\\\nb\\\";\"]\n/// ```\nfn f() {}",
        );
        assert!(
            !out.contains("///"),
            "sugared line inside a raw block: {out}"
        );
        assert_eq!(out.matches("#[doc=").count(), 4, "{out}");
    }

    #[test]
    fn a_fully_plain_doc_block_stays_sugared() {
        assert_eq!(
            render_src("/// One.\n/// Two.\n/// Three.\nfn f() {}"),
            "/// One.\n/// Two.\n/// Three.\nfn f(){}"
        );
    }

    #[test]
    fn an_inner_doc_block_with_one_escaped_line_is_emitted_entirely_raw() {
        let out = render_src("//! Module.\n#![doc = \"a \\\"b\\\"\"]\nfn f() {}");
        assert!(
            !out.contains("//!"),
            "sugared line inside a raw block: {out}"
        );
        assert_eq!(out.matches("#![doc=").count(), 2, "{out}");
    }

    #[test]
    fn inner_and_outer_doc_runs_are_separate_blocks() {
        // The kind change ends the block: the plain inner line stays sugared
        // even though the outer run that follows it has to go raw.
        let out = render_src("//! Module.\n#[doc = \"a \\\"b\\\"\"]\n/// Plain.\nfn f() {}");
        assert!(out.starts_with("//! Module.\n"), "{out}");
        assert!(
            !out.contains("///"),
            "sugared line inside a raw block: {out}"
        );
        assert_eq!(out.matches("#[doc=").count(), 2, "{out}");
    }

    #[test]
    fn doc_blocks_inside_groups_are_grouped_too() {
        let out = render_src("mod m {\n/// Plain.\n#[doc = \"a \\\"b\\\"\"]\nfn f() {}\n}");
        assert!(
            !out.contains("///"),
            "sugared line inside a raw block: {out}"
        );
        assert_eq!(out.matches("#[doc=").count(), 2, "{out}");
    }

    #[test]
    fn strip_doc_attrs_removes_doc_comments_but_keeps_doc_hidden() {
        let file =
            syn::parse_file("/// gone\n#[doc(hidden)]\npub fn f() {}\nmod m { //! inner\n }")
                .unwrap();
        let stripped: syn::File = syn::parse2(strip_doc_attrs(file.to_token_stream())).unwrap();
        let out = render(&stripped);
        assert_eq!(out, "#[doc(hidden)]pub fn f(){}mod m{}");
    }

    #[test]
    fn string_literal_before_a_word_keeps_a_space() {
        // `extern "system"fn` would lex as one suffixed literal token.
        assert_eq!(
            render_src("unsafe extern \"system\" fn cb(x: u32) -> u32 { x }"),
            "unsafe extern \"system\" fn cb(x:u32)->u32{x}"
        );
        assert!(needs_space(Prev::Literal('#'), 'f', false));
        assert!(!needs_space(Prev::Literal('"'), '.', false));
    }

    #[test]
    fn ident_before_quote_needs_space() {
        assert!(needs_space(Prev::Ident('b'), '\'', true));
        assert!(needs_space(Prev::Ident('r'), '"', true));
        assert!(!needs_space(
            Prev::Punct {
                ch: '&',
                joint: false
            },
            '\'',
            false
        ));
    }

    #[test]
    fn start_of_output_never_needs_space() {
        assert!(!needs_space(Prev::Start, 'f', false));
    }

    #[test]
    fn empty_file_renders_empty() {
        assert_eq!(render_src(""), "");
    }

    #[test]
    fn doc_comment_after_an_item_starts_on_its_own_line() {
        assert_eq!(
            render_src("fn a() {}\n/// d\nfn b() {}"),
            "fn a(){}\n/// d\nfn b(){}"
        );
    }

    #[test]
    fn hash_tokens_in_macro_bodies_are_not_doc_attrs() {
        // `#` followed by an ident, a non-bracket group, and a non-doc
        // bracket group must all pass through untouched.
        assert_eq!(
            render_src("fn f() { m!(# a); m!(# (b)); m!(# [c]); }"),
            "fn f(){m!(#a);m!(#(b));m!(#[c]);}"
        );
    }

    #[test]
    fn none_delimiter_groups_are_transparent() {
        let g = TokenTree::Group(proc_macro2::Group::new(
            Delimiter::None,
            "1 + 1".parse().unwrap(),
        ));
        let file: syn::File = syn::parse2(quote::quote! { fn f() { m!(#g); } }).unwrap();
        assert_eq!(render(&file), "fn f(){m!(1+1);}");
    }
}
