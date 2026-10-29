//! The Sentry client's options that the app and the runner share (feature `client`): GlitchTip's `desktop` project, the
//! integrations, and [`ReportScrubber`] as `before_send`. Starting the client and its transport is each program's own.

use std::borrow::Cow;

use sentry::{
  ClientOptions,
  integrations::{
    backtrace::{AttachStacktraceIntegration, ProcessStacktraceIntegration},
    contexts::ContextIntegration,
    debug_images::DebugImagesIntegration,
    panic::PanicIntegration,
  },
  protocol::{Context, Event, Value},
};

use crate::ReportScrubber;

/// The `desktop` project's client key in GlitchTip: public, it only lets a program send reports there.
pub const DSN: &str = "https://13e0bcbf57a14c2597a67d9a57d53bfe@errors.orchester.cloud/1";

/// How long a program waits for its pending reports to leave when it exits, or after a panic.
pub const FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// The context in which sentry's tracing layer puts a log line's fields.
const LOG_FIELDS: &str = "Rust Tracing Fields";

/// The client's options for the reports of `process` (the `process` tag: `app` or `runner`) in `release` and
/// `environment`: GlitchTip's `desktop` project, and the scrubber as `before_send`, after a log line's fields are put
/// into its message ([`with_fields_in_message`]) and the tag is set. Sentry's defaults keep PII, traces and sessions
/// off. The integrations are named here rather than left to `sentry::init`, so a client made from these options alone
/// (a test's) reports the same way: the panic hook, the OS and CPU contexts, the stack where an ERROR line was logged,
/// the stack trace's frames marked as Orchester's or not, and the loaded modules with their debug IDs, by which
/// GlitchTip resolves a release build's frame addresses in its uploaded PDBs.
pub fn options(
  process: &'static str,
  release: impl Into<Cow<'static, str>>,
  environment: impl Into<Cow<'static, str>>,
) -> ClientOptions {
  let scrubber = ReportScrubber::for_this_build();
  ClientOptions::new()
    .dsn(DSN)
    .release(release)
    .environment(environment)
    .send_default_pii(false)
    .shutdown_timeout(FLUSH_TIMEOUT)
    .attach_stacktrace(true)
    .default_integrations(false)
    .add_integration(ContextIntegration::default())
    .add_integration(PanicIntegration::default())
    .add_integration(AttachStacktraceIntegration)
    .add_integration(ProcessStacktraceIntegration)
    .add_integration(DebugImagesIntegration::new())
    .before_send(move |mut event| {
      event.tags.insert("process".to_owned(), process.to_owned());
      Some(scrubber.scrub(with_fields_in_message(event)))
    })
}

/// `event` with the fields of the log line it came from appended to its message as `name=value`, the way the log files
/// write them, so the scrubber cleans them with the message; it drops the context that held them.
fn with_fields_in_message(mut event: Event<'static>) -> Event<'static> {
  let Some(Context::Other(fields)) = event.contexts.remove(LOG_FIELDS) else {
    return event;
  };
  let fields = fields.into_iter().map(|(name, value)| match value {
    Value::String(text) => format!("{name}={text}"),
    other => format!("{name}={other}"),
  });
  let words: Vec<String> = event.message.take().into_iter().chain(fields).collect();
  event.message = Some(words.join(" "));
  event
}
