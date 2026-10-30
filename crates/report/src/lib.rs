//! Orchester's error reports, in the Sentry protocol that GlitchTip speaks: [`ReportScrubber`], the one
//! `before_send` every report of the desktop app and the runner passes through, which keeps only what
//! `docs/error-reports.md` lists and removes the rest, (feature `client`) the client options both programs report
//! with, and (feature `switch`) the transport that turns a running client's reports on and off. A crate of its own
//! because both the app and the runner report, and none of the crates they share fits: `orchester-core` stays free of
//! a telemetry SDK, and the others hold the IPC protocol, processes, the database and the keychain. Nothing here sends
//! a report: each program starts its own client.

#[cfg(feature = "client")]
pub mod client;
mod event;
mod images;
mod paths;
mod secrets;
mod stack;
#[cfg(feature = "switch")]
pub mod switch;

pub use event::{ORCHESTER_TAGS, ReportScrubber};
