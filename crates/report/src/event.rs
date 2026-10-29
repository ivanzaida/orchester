use std::{borrow::Cow, path::Path};

use sentry::protocol::{Context, DeviceContext, Event, Map, OsContext};

use crate::{paths::PathRules, secrets};

/// The tags Orchester sets on its reports, the only ones a report keeps: `process`, which of its programs
/// reported (the app or the runner), and on the app's report of a frozen window `screen`, the kind of screen it
/// showed, and `stack`, whether the window's thread's stack was captured.
pub const ORCHESTER_TAGS: &[&str] = &["process", "screen", "stack"];

/// Removes everything private from an error report before it may leave the computer.
///
/// It keeps an allow-list of the event's fields and drops the rest; in every text it keeps, the Orchester
/// source tree's paths become relative to it, the home folder becomes `~`, any other absolute path becomes
/// `<path>`, and whatever is shaped like a secret becomes `<redacted>`.
pub struct ReportScrubber {
  paths: PathRules,
}

impl ReportScrubber {
  /// A scrubber for the source tree at `source_root` and the person's `home` folder.
  pub fn new(source_root: &Path, home: Option<&Path>) -> Self {
    Self {
      paths: PathRules::new(source_root, home),
    }
  }

  /// A scrubber for the source tree this binary was built from, whose paths its stack frames carry, and the
  /// signed-in person's home folder.
  pub fn for_this_build() -> Self {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
      .ancestors()
      .nth(2)
      .expect("crates/report lies two folders below the workspace root");
    Self::new(source_root, std::env::home_dir().as_deref())
  }

  /// The report `event` with only what may be sent; the hook `sentry::ClientOptions::before_send` calls.
  pub fn scrub(&self, event: Event<'static>) -> Event<'static> {
    let message = event.message.or(event.logentry.map(|entry| entry.message));
    Event {
      event_id: event.event_id,
      level: event.level,
      timestamp: event.timestamp,
      platform: event.platform,
      release: event.release,
      environment: event.environment,
      message: message.map(|message| self.text(&message)),
      exception: event
        .exception
        .values
        .into_iter()
        .map(|exception| self.exception(exception))
        .collect::<Vec<_>>()
        .into(),
      stacktrace: event.stacktrace.map(|stacktrace| self.stacktrace(stacktrace)),
      threads: event
        .threads
        .values
        .into_iter()
        .map(|thread| self.thread(thread))
        .collect::<Vec<_>>()
        .into(),
      debug_meta: Cow::Owned(self.debug_meta(event.debug_meta.into_owned())),
      contexts: system_contexts(event.contexts),
      tags: self.tags(event.tags),
      ..Event::default()
    }
  }

  pub(crate) fn text(&self, text: &str) -> String {
    secrets::redact(&self.paths.rewrite(text))
  }

  fn tags(&self, tags: Map<String, String>) -> Map<String, String> {
    tags
      .into_iter()
      .filter(|(name, _)| ORCHESTER_TAGS.contains(&name.as_str()))
      .map(|(name, value)| (name, self.text(&value)))
      .collect()
  }
}

/// The operating system's name and version and the CPU architecture, without the other contexts.
fn system_contexts(contexts: Map<String, Context>) -> Map<String, Context> {
  let mut kept = Map::new();
  for context in contexts.into_values() {
    match context {
      Context::Os(os) => {
        let os = OsContext {
          name: os.name,
          version: os.version,
          ..OsContext::default()
        };
        kept.insert("os".to_owned(), os.into());
      }
      Context::Device(device) => {
        let device = DeviceContext {
          arch: device.arch,
          ..DeviceContext::default()
        };
        kept.insert("device".to_owned(), device.into());
      }
      _ => {}
    }
  }
  kept
}
