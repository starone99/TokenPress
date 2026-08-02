//! TokenPress for Ruby — the Ruby backend, built on `ruby-prism`.
//!
//! This crate is being assembled in stages; today it holds the parser
//! boundary ([`parser`]), the path-support decision ([`paths`]), the
//! AST-equivalence artifact ([`comparable`]), the whole emitter — its
//! protected-span machinery, its whitespace policy and its comment-stripping
//! policy ([`emit`]) — and the verifier ([`verify`]). The
//! `tokenpress_core::Formatter` implementation lands in a later step, so
//! nothing here is wired into the CLI yet. Modules
//! are declared `pub` from the moment they land — the same arrangement
//! `tokenpress-js` uses — so an as-yet-unwired module is covered by its own
//! unit tests instead of tripping dead-code warnings.
//!
//! # Parser boundary
//!
//! `ruby-prism` is pinned **exactly** (`=1.9.0`): it is pre-1.0 with no
//! semver guarantees and declares no `rust-version`, so there is not even an
//! MSRV signal to read and any upgrade is a toolchain decision. Every
//! `ruby_prism` type and function this crate uses is therefore used inside
//! [`parser`] or re-exported from it — no other module may name `ruby_prism`,
//! exactly as CLAUDE.md requires for the ruff pin in `tokenpress-python` and
//! the oxc pin in `tokenpress-js`.
//!
//! A parsed result borrows the source bytes it was handed and cannot be
//! wrapped in an owning struct, so the eventual parse → emit → verify
//! pipeline has to run inside a single function scope; see [`parser`].
//!
//! # Build prerequisite
//!
//! `ruby-prism-sys` compiles the vendored prism C sources and generates its
//! bindings with bindgen, so building this crate needs a C compiler **and**
//! libclang. Ruby itself is not needed at build time.

pub mod comparable;
pub mod emit;
pub mod parser;
pub mod paths;
pub mod verify;

pub use tokenpress_core::{Error, Result};
