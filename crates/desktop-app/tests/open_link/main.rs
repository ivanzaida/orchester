//! The `orchester://` link (`docs/architecture/server.md`, Invites › *Accepting outside Orchester*, row 77): the parser
//! with every form it refuses ([`parse`]), the hand-over of a link to the app that holds the data folder over its pipe
//! ([`pipe`], Windows), the installer's registration of the scheme ([`installer`]), and the window that opens a link:
//! the invite's row in Settings › Orchester, or *This link isn't for anything here* ([`window`]).
//!
//! `cargo test -p orchester-app --test open_link open_link_`

mod installer;
mod parse;
#[cfg(windows)]
mod pipe;
#[path = "../support/mod.rs"]
mod support;
mod window;

/// An invite id in the form a link carries it.
const INVITE: &str = "0192f3a4-5b6c-7d8e-9f01-23456789abcd";
