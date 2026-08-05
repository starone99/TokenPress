//! TokenPress for C# — the C#-specific half of the tree-sitter backend.
//!
//! At this stage the crate is the grammar boundary, the path set and the
//! comment policy: [`config`] names the grammar the shared engine drives,
//! [`paths`] says which files this backend claims, and [`policy`] holds the
//! three decisions the engine's comment stripper takes. The
//! [`tokenpress_core::Formatter`] implementation lands on top of them.
//!
//! # The crate split
//!
//! `tokenpress-treesitter` is the grammar-agnostic engine: one tree-sitter
//! runtime, the parse gate, the equivalence artifact, the protected-span
//! model, the whitespace rewriter and the comment stripper, none of which
//! knows which language it is looking at. **This** crate holds what cannot be
//! generic. A grammar reaches the engine as configuration, not as a
//! dependency, so `tree-sitter-c-sharp` is named in exactly one place —
//! [`config`] — and this crate never names the `tree-sitter` runtime at all.
//!
//! # What C# brings that Go and Java did not
//!
//! Two differences shape everything above this layer, and both are already
//! visible in [`config`]. **One comment kind**, not Java's two: `//`,
//! `/* … */` and `///` XML documentation comments are all the node kind
//! `comment`, so a policy that wants to treat XML doc specially cannot key on
//! the kind and has to read the comment's leading bytes. And **five**
//! protected literal kinds, not Java's two, because C# spells a string five
//! ways — ordinary, verbatim, raw, interpolated, and the character literal —
//! and the grammar gives four of them a node kind of their own.
//!
//! The one constraint that has no analogue in either earlier backend is the
//! **preprocessor line rule**: `#if`, `#region`, `#nullable` and their
//! relatives must each begin a line, so a directive dragged onto the line
//! before it stops being a directive. That is one of the two hazards behind
//! `newline_sensitive = true` — see [`config`] for the measurement — and
//! [`policy`] keeps it that way, as well as carrying the two constructs where
//! the grammar and a real C# compiler disagree about where a comment ends.

pub mod config;
pub mod paths;
pub mod policy;
