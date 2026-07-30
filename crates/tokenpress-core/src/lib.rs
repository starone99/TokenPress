//! Language-agnostic core for TokenPress.
//!
//! This crate defines the interfaces every language formatter implements
//! ([`Formatter`]), the tokenizer abstraction used for token counting
//! ([`Tokenizer`]), and the shared option/result/error types.

mod error;
mod formatter;
mod options;
mod tokenizer;

pub use error::{Error, Result};
pub use formatter::{FormatResult, Formatter};
pub use options::{FormatOptions, VerifyLevel};
pub use tokenizer::{Tokenizer, TokenizerKind};
