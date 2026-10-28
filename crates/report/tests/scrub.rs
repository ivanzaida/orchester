use std::{borrow::Cow, path::Path};

use orchester_report::ReportScrubber;
use sentry::protocol::{
  Breadcrumb, Context, DeviceContext, Event, Exception, Frame, Level, Map, OsContext, Request, RuntimeContext,
  Stacktrace, Url, User, Value,
};

const HOME: &str = "/Users/ivan";
const SOURCE_ROOT: &str = "/build/orchester";
const PROJECT: &str = "D:/work/acme-secret";
const API_KEY: &str = "sk-ant-api03-AbCdEf123456GhIjKl";
const BEARER_TOKEN: &str = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJpdmFuIn0.c2lnbmF0dXJl";

fn scrubber() -> ReportScrubber {
  ReportScrubber::new(Path::new(SOURCE_ROOT), Some(Path::new(HOME)))
}

fn private_event() -> Event<'static> {
  let mut event = Event {
    message: Some(format!(
      "failed to read {HOME}/notes.txt in {PROJECT} with {API_KEY} and Bearer {BEARER_TOKEN}"
    )),
    server_name: Some(Cow::Borrowed("IVAN-PC")),
    user: Some(User {
      id: Some("user-42".to_owned()),
      email: Some("ivan@example.com".to_owned()),
      username: Some("ivanz".to_owned()),
      ..User::default()
    }),
    request: Some(Request {
      url: Some(Url::parse("https://intranet.example/private").expect("the URL is valid")),
      cookies: Some("session=cookie-value".to_owned()),
      ..Request::default()
    }),
    breadcrumbs: vec![Breadcrumb {
      message: Some("opened the crumb-project".to_owned()),
      ..Breadcrumb::default()
    }]
    .into(),
    ..Event::default()
  };
  event.extra.insert("extra-key".to_owned(), Value::from("extra-value"));
  event.tags.insert("process".to_owned(), "runner".to_owned());
  event.tags.insert("foreign-tag".to_owned(), "foreign-value".to_owned());
  event
}

#[test]
fn scrub_removes_every_private_value() {
  let scrubbed = format!("{:?}", scrubber().scrub(private_event()));

  for private in [
    "/Users",
    "ivan",
    "acme-secret",
    "D:/work",
    API_KEY,
    BEARER_TOKEN,
    "IVAN-PC",
    "user-42",
    "intranet.example",
    "cookie-value",
    "crumb-project",
    "extra-key",
    "extra-value",
    "foreign-tag",
    "foreign-value",
  ] {
    assert!(!scrubbed.contains(private), "{private} is left in {scrubbed}");
  }
}

#[test]
fn scrub_drops_user_request_extra_breadcrumbs_and_foreign_tags() {
  let scrubbed = scrubber().scrub(private_event());

  assert_eq!(scrubbed.server_name, None);
  assert_eq!(scrubbed.user, None);
  assert_eq!(scrubbed.request, None);
  assert!(scrubbed.extra.is_empty());
  assert!(scrubbed.breadcrumbs.values.is_empty());
  assert_eq!(scrubbed.tags, Map::from([("process".to_owned(), "runner".to_owned())]));
}

#[test]
fn scrub_shortens_home_hides_other_paths_and_redacts_secrets() {
  let scrubbed = scrubber().scrub(private_event());

  assert_eq!(
    scrubbed.message.as_deref(),
    Some("failed to read ~/<path> in <path> with <redacted> and Bearer <redacted>")
  );
}

#[test]
fn scrub_hides_a_unix_project_under_the_home_folder() {
  let event = Event {
    message: Some(format!(
      "failed to open {HOME}/projects/acme-secret/.orchester/worktrees/run-1/src/lib.rs at {SOURCE_ROOT}/crates/x.rs"
    )),
    ..Event::default()
  };

  let scrubbed = scrubber().scrub(event);

  assert_eq!(
    scrubbed.message.as_deref(),
    Some("failed to open ~/<path> at crates/x.rs")
  );
}

#[test]
fn scrub_hides_a_windows_project_under_the_home_folder_but_keeps_the_source_tree_in_it() {
  let scrubber = ReportScrubber::new(
    Path::new(r"C:\Users\Ivan\src\orchester"),
    Some(Path::new(r"C:\Users\Ivan")),
  );
  let event = Event {
    message: Some(
      r"cannot read C:\Users\Ivan\work\acme\notes.txt from C:\Users\Ivan\src\orchester\crates\runner\src\main.rs:3"
        .to_owned(),
    ),
    ..Event::default()
  };

  let scrubbed = scrubber.scrub(event);

  assert_eq!(
    scrubbed.message.as_deref(),
    Some(r"cannot read ~\<path> from crates\runner\src\main.rs:3")
  );
}

#[test]
fn scrub_hides_paths_after_a_colon_or_a_url_scheme() {
  let event = Event {
    message: Some(r"open file:///D:/work/acme and file:///srv/acme, path:D:\work\acme".to_owned()),
    ..Event::default()
  };

  let scrubbed = scrubber().scrub(event);

  assert_eq!(
    scrubbed.message.as_deref(),
    Some("open file://<path> and file://<path>, path:<path>")
  );
}

#[test]
fn scrub_keeps_stack_frames_message_level_release_and_environment() {
  let event = Event {
    level: Level::Fatal,
    release: Some(Cow::Borrowed("orchester@0.3.0")),
    environment: Some(Cow::Borrowed("production")),
    exception: vec![Exception {
      ty: "panic".to_owned(),
      value: Some("index out of bounds".to_owned()),
      stacktrace: Some(Stacktrace {
        frames: vec![Frame {
          function: Some("orchester_runner::main".to_owned()),
          module: Some("orchester_runner".to_owned()),
          filename: Some("main.rs".to_owned()),
          abs_path: Some(format!("{SOURCE_ROOT}/crates/runner/src/main.rs")),
          lineno: Some(12),
          colno: Some(5),
          in_app: Some(true),
          context_line: Some("let secret = read();".to_owned()),
          vars: Map::from([("secret".to_owned(), Value::from("hunter2"))]),
          ..Frame::default()
        }],
        ..Stacktrace::default()
      }),
      ..Exception::default()
    }]
    .into(),
    ..Event::default()
  };

  let scrubbed = scrubber().scrub(event);

  assert_eq!(scrubbed.level, Level::Fatal);
  assert_eq!(scrubbed.release.as_deref(), Some("orchester@0.3.0"));
  assert_eq!(scrubbed.environment.as_deref(), Some("production"));
  let exception = &scrubbed.exception.values[0];
  assert_eq!(exception.ty, "panic");
  assert_eq!(exception.value.as_deref(), Some("index out of bounds"));
  let frames = &exception.stacktrace.as_ref().expect("the stack trace stays").frames;
  assert_eq!(
    frames,
    &vec![Frame {
      function: Some("orchester_runner::main".to_owned()),
      module: Some("orchester_runner".to_owned()),
      filename: Some("crates/runner/src/main.rs".to_owned()),
      lineno: Some(12),
      colno: Some(5),
      in_app: Some(true),
      ..Frame::default()
    }]
  );
}

#[test]
fn scrub_keeps_the_message_of_a_log_entry() {
  let event = Event {
    logentry: Some(sentry::protocol::LogEntry {
      message: format!("cannot open {HOME}/x"),
      params: vec![Value::from("param-value")],
    }),
    ..Event::default()
  };

  let scrubbed = scrubber().scrub(event);

  assert_eq!(scrubbed.message.as_deref(), Some("cannot open ~/<path>"));
  assert_eq!(scrubbed.logentry, None);
}

#[test]
fn scrub_matches_windows_paths_with_spaces_and_backslashes() {
  let scrubber = ReportScrubber::new(Path::new(r"D:\a\orchester"), Some(Path::new(r"C:\Users\Ivan Zaida")));
  let event = Event {
    message: Some(
      r"panicked at D:\a\orchester\crates\runner\src\main.rs:12:5 reading c:/users/ivan zaida\AppData\x.log and C:\Program Files\Acme Tools\acme.exe"
        .to_owned(),
    ),
    ..Event::default()
  };

  let scrubbed = scrubber.scrub(event);

  assert_eq!(
    scrubbed.message.as_deref(),
    Some(r"panicked at crates\runner\src\main.rs:12:5 reading ~\<path> and <path>")
  );
}

#[test]
fn scrub_matches_only_whole_folder_names() {
  let event = Event {
    message: Some(format!(
      "{HOME}by/x and {HOME}.smith/x and {SOURCE_ROOT}.bak/x and {HOME}"
    )),
    ..Event::default()
  };

  let scrubbed = scrubber().scrub(event);

  assert_eq!(scrubbed.message.as_deref(), Some("<path> and <path> and <path> and ~"));
}

#[test]
fn for_this_build_reads_its_source_files_relative_to_the_workspace() {
  let event = Event {
    message: Some(format!("at {}/src/event.rs:40", env!("CARGO_MANIFEST_DIR"))),
    ..Event::default()
  };

  let scrubbed = ReportScrubber::for_this_build().scrub(event);

  let message = scrubbed.message.expect("the message stays").replace('\\', "/");
  assert_eq!(message, "at crates/report/src/event.rs:40");
}

#[test]
fn scrub_redacts_tokens_and_long_runs_but_keeps_words_and_ids() {
  let event = Event {
    message: Some(
      "ghp_abcdefghijklmnopqrstuvwxyz0123456789 gho_abcdefgh1234 github_pat_11ABCDEFG0abcdefghij \
       3f577c7f2eee1aa708554897ef051007dc2bede9 QWxhZGRpbjpvcGVuIHNlc2FtZQ9xYz1234567== \
       sk-learn task-runner 01a18429-9a1b-72b7-bc70-69a04efc241f Bearer"
        .to_owned(),
    ),
    ..Event::default()
  };

  let scrubbed = scrubber().scrub(event);

  assert_eq!(
    scrubbed.message.as_deref(),
    Some(
      "<redacted> <redacted> <redacted> <redacted> <redacted> sk-learn task-runner \
       01a18429-9a1b-72b7-bc70-69a04efc241f Bearer"
    )
  );
}

#[test]
fn scrub_keeps_only_the_os_name_and_version_and_the_cpu_architecture() {
  let mut event = Event::default();
  event.contexts.insert(
    "os".to_owned(),
    OsContext {
      name: Some("Windows".to_owned()),
      version: Some("10.0.26200".to_owned()),
      build: Some("26200".to_owned()),
      kernel_version: Some("kernel".to_owned()),
      ..OsContext::default()
    }
    .into(),
  );
  event.contexts.insert(
    "device".to_owned(),
    DeviceContext {
      name: Some("IVAN-PC".to_owned()),
      arch: Some("x86_64".to_owned()),
      memory_size: Some(32),
      ..DeviceContext::default()
    }
    .into(),
  );
  event.contexts.insert(
    "rust".to_owned(),
    RuntimeContext {
      name: Some("rustc".to_owned()),
      ..RuntimeContext::default()
    }
    .into(),
  );

  let scrubbed = scrubber().scrub(event);

  let expected: Map<String, Context> = Map::from([
    (
      "os".to_owned(),
      OsContext {
        name: Some("Windows".to_owned()),
        version: Some("10.0.26200".to_owned()),
        ..OsContext::default()
      }
      .into(),
    ),
    (
      "device".to_owned(),
      DeviceContext {
        arch: Some("x86_64".to_owned()),
        ..DeviceContext::default()
      }
      .into(),
    ),
  ]);
  assert_eq!(scrubbed.contexts, expected);
}
