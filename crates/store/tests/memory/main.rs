//! Task and project memory in the project database: records and their changes, scope filters,
//! full-text search by word stems and trigram fragments, the Memory setting and what each
//! invocation was given. Run with `cargo test -p orchester-store --test memory memory`.

#[path = "../support/mod.rs"]
mod support;

mod changes;
mod fixture;
mod records;
mod search;
