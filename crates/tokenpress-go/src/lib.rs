//! TokenPress for Go — the Go-specific half of the tree-sitter backend.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic. At this stage that is the grammar configuration ([`config`]), the
//! path set the backend claims ([`paths`]) and the comment hazard surface
//! ([`policy`]); the `Formatter` implementation and the external checker are
//! not written yet.
//!
//! # Where the language knowledge lives
//!
//! [`config`] answers *what the grammar is*: which node kinds are comments,
//! which are literals whose bytes may never be touched, whether a newline can
//! change meaning. [`policy`] answers *what the language does with a comment*:
//! Go has no pragma syntax, so build constraints, compiler and linker
//! directives, `go generate` commands, `go:embed` bindings and the whole cgo
//! preamble all ride in comments, and the engine's equivalence artifact is
//! comment-blind by construction. Every rule that keeps a comment, protects
//! the head of a file, refuses to move a comment to column 0, or refuses to
//! touch a file at all is in that one module.
//!
//! A grammar reaches the engine as configuration, not as a dependency, so
//! `tree-sitter-go` is named in exactly one place — [`config`] — and this
//! crate never names the `tree-sitter` runtime at all.

pub mod config;
pub mod paths;
pub mod policy;
