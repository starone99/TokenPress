//! The grammar-agnostic tree-sitter engine shared by every tree-sitter
//! backend (Go first; Java, C# and PHP are meant to reuse it verbatim).
//!
//! # The crate split
//!
//! tree-sitter is one runtime driving many generated grammars, so the work
//! divides cleanly and the split is deliberate:
//!
//! - **this crate** holds everything that does not know which language it is
//!   looking at: the parse gate, the equivalence artifact, the protected-span
//!   model, the whitespace rewriter and the comment stripper — the last of
//!   which takes its language-specific decisions as callbacks
//!   ([`emit::CommentPolicy`]). A grammar is not a dependency here,
//!   it is *configuration* — a [`parser::LanguageConfig`] carrying the
//!   [`parser::Language`], the comment and protected node kinds, and a
//!   newline-sensitivity flag.
//! - **a per-language crate** (`tokenpress-go`, and later siblings) holds
//!   everything that cannot be generic: the kind lists themselves, the
//!   language's hazard rules, the external checker, the supported path set,
//!   and the [`tokenpress_core::Formatter`] implementation.
//!
//! The engine's leverage comes from that boundary being enforced by the
//! grammar itself: [`parser::LanguageConfig::new`] validates every kind name
//! against the grammar, so a typo in a per-language kind list is a
//! construction error rather than a silently unprotected string literal.
//!
//! # Why a grammar is a dev-dependency
//!
//! A generic engine cannot be tested without at least one real grammar, so
//! `tree-sitter-go` is a **dev-dependency**: the tests run against exactly
//! the grammar the first consumer ships, and no released artifact links it.
//! The alternative — a hand-rolled toy grammar checked in as generated
//! `parser.c` — would add a grammar generator to the build and still prove
//! less than a grammar a real backend depends on. That trade is not worth
//! it.
//!
//! # Build prerequisite, and why CI needs no change
//!
//! `cc` compiles tree-sitter's `src/lib.c` (and a grammar's `parser.c`), so
//! this crate needs a **C compiler** — but no C++, and no bindgen/libclang,
//! which is strictly less than `tokenpress-ruby` already requires. Every CI
//! job that builds the workspace already runs `.github/actions/libclang`,
//! which installs a C compiler along with libclang, so `ci.yml` and
//! `release.yml` need **no new setup step** for this crate. That is stated
//! here explicitly rather than left implied. The one job that deliberately
//! runs without it, `no-ruby`, builds `tokenpress-cli
//! --no-default-features`, which does not reach this crate.

pub mod comparable;
pub mod emit;
pub mod parser;

pub use tokenpress_core::{Error, Result};
