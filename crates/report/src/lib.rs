//! Orchester's error reports, in the Sentry protocol that GlitchTip speaks: [`ReportScrubber`], the one
//! `before_send` every report of the desktop app and the runner passes through, which keeps only what
//! `docs/error-reports.md` lists and removes the rest. A crate of its own because both the app and the runner
//! report, and none of the crates they share fits: `orchester-core` stays free of a telemetry SDK, and the others
//! hold the IPC protocol, processes, the database and the keychain. Nothing here sends a report.

mod event;
mod paths;
mod secrets;
mod stack;

pub use event::{ORCHESTER_TAGS, ReportScrubber};
