//! TokenPress for JavaScript/TypeScript — **under construction**.
//!
//! JS/TS/JSX/TSX support is experimental and is **not yet wired into the
//! CLI**: this crate currently exposes only the parser front end (`parser`),
//! the whitespace-minimal renderer (`emit`) and the verifier (`verify`), with
//! the `Formatter` implementation still to come. Nothing here should be
//! treated as a supported language backend yet.
//!
//! Emitted output is **not comment-preserving**: `oxc_codegen` keeps only
//! leading statement-level comments (plus jsdoc, annotation and legal
//! comments) and drops trailing and expression-position comments. See
//! [`emit`] for the full statement.

pub mod emit;
pub mod parser;
pub mod verify;

pub use tokenpress_core::{Error, Result};
