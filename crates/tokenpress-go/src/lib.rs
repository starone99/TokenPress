//! TokenPress for Go — the Go-specific half of the tree-sitter backend.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic. At this stage that is the grammar configuration ([`config`]) and
//! the path set the backend claims ([`paths`]); the comment policy, the
//! `Formatter` implementation and the external checker are not written yet.
//!
//! A grammar reaches the engine as configuration, not as a dependency, so
//! `tree-sitter-go` is named in exactly one place — [`config`] — and this
//! crate never names the `tree-sitter` runtime at all.

pub mod config;
pub mod paths;
