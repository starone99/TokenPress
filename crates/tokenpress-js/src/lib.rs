//! TokenPress for JavaScript/TypeScript — **under construction**.
//!
//! JS/TS/JSX/TSX support is experimental and is **not yet wired into the
//! CLI**: this crate currently exposes only the parser front end
//! (`parser`), with emit, verification and the `Formatter` implementation
//! still to come. Nothing here should be treated as a supported language
//! backend yet.

pub mod parser;

pub use tokenpress_core::{Error, Result};
