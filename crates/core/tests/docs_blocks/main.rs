//! A doc's Markdown body as blocks with inline marks (`orchester_core::docs::blocks`): every block kind, marks and
//! their nesting, the normalized Markdown written back, the round trip on real docs, and block ids across edits.
//! `cargo test -p orchester-core --test docs_blocks docs_blocks`.

mod ids;
mod kinds;
mod lists;
mod marks;
mod round_trip;
mod support;
