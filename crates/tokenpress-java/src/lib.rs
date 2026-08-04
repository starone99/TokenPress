//! TokenPress for Java — the Java-specific half of the tree-sitter backend.
//!
//! At this stage the crate is the grammar boundary, the path set and the
//! comment policy: [`config`] names the grammar the shared engine drives,
//! [`paths`] says which files this backend claims, and [`policy`] is the three
//! decisions the engine's comment stripper takes. The
//! [`tokenpress_core::Formatter`] implementation lands on top of them.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic. A grammar reaches the engine as configuration, not as a
//! dependency, so `tree-sitter-java` is named in exactly one place —
//! [`config`] — and this crate never names the `tree-sitter` runtime at all.
//!
//! Java needs *less* of the engine than Go does: `javac` reads nothing out of
//! a comment, so there is no directive keep-list required for correctness, no
//! column-0 promotion rule and no verbatim prologue.

pub mod config;
pub mod paths;
pub mod policy;
