//! Drives a started app through the real invocation relay, including refused discovery and outside executables.

use std::{
  fs,
  io::{BufRead, BufReader},
  path::{Path, PathBuf},
  process::{Child, Command, Stdio},
};

use serde_json::{Value, json};

struct App(Child);

impl Drop for App {
  fn drop(&mut self) {
    if self.0.try_wait().expect("status").is_none() {
      self.0.kill().expect("kill own stand-in");
    }
    self.0.wait().expect("reap own stand-in");
  }
}

fn start(worktree: &Path) -> (App, PathBuf) {
  let original = std::env::current_exe()
    .expect("CLI")
    .parent()
    .expect("bin")
    .join(if cfg!(windows) {
      "orchester-test-command.exe"
    } else {
      "orchester-test-command"
    });
  let folder = worktree.join(".tmp/qa/app");
  fs::create_dir_all(folder.join("bin")).expect("bin");
  let program = folder.join("bin").join(if cfg!(windows) {
    "orchester-app.exe"
  } else {
    "orchester-app"
  });
  fs::copy(original, &program).expect("copy");
  let discovery = folder.join("profile/orchester-app-mcp");
  (start_program(&program, &discovery), discovery)
}

fn start_program(program: &Path, discovery: &Path) -> App {
  let mut command = Command::new(program);
  command
    .arg("serve-app")
    .arg(discovery)
    .stdout(Stdio::piped())
    .stderr(Stdio::null());
  #[cfg(windows)]
  {
    use std::os::windows::process::CommandExt;
    command.creation_flags(0x0800_0000);
  }
  let mut app = App(command.spawn().expect("start"));
  let mut reader = BufReader::new(app.0.stdout.take().expect("stdout"));
  for _ in 0..2 {
    let mut line = String::new();
    reader.read_line(&mut line).expect("ready");
    assert!(!line.is_empty());
  }
  app
}

pub fn exercise(worktree: &Path, mut call: impl FnMut(&str, &Value) -> Value) {
  let (app, folder) = start(worktree);
  let pid = app.0.id();
  let discovery_file = folder.join(format!("{pid}.json"));
  let initial = fs::read(&discovery_file).expect("discovery");
  let navigate = json!({"pid":pid,"tool":"navigate","arguments":{"route":"tasks"}});
  let outside = call(
    "call_run_app",
    &json!({"pid":std::process::id(),"tool":"app_state","arguments":{}}),
  );
  assert_eq!(outside["isError"], true);
  assert!(
    outside["content"][0]["text"]
      .as_str()
      .expect("error")
      .contains("refused")
  );
  eprintln!("app-relay: outside refused");

  refuse_outside_apps(worktree, &folder, &mut call);

  for field in ["pid", "app", "url"] {
    let mut discovery: Value = serde_json::from_slice(&initial).expect("discovery");
    discovery[field] = match field {
      "pid" => json!(pid + 1),
      "app" => json!("other-app"),
      _ => json!("http://example.com/mcp"),
    };
    fs::write(&discovery_file, discovery.to_string()).expect("tamper");
    let refused = call("call_run_app", &navigate);
    assert_eq!(refused["isError"], true);
    assert!(!folder.join("calls").exists(), "refusal made no request");
    eprintln!("app-relay: {field} refused");
  }
  fs::write(&discovery_file, initial).expect("restore");
  let answer = call("call_run_app", &navigate);
  assert_eq!(answer["isError"], false);
  assert_eq!(answer["structuredContent"]["route"], "tasks");
  assert_eq!(answer["structuredContent"]["credential"], "<redacted>");
  assert!(!answer.to_string().contains("stand-in-app-private-token"));
  eprintln!("app-relay: navigate structured and redacted");

  let answer = call(
    "call_run_app",
    &json!({"pid":pid,"tool":"lurq_screenshot","arguments":{}}),
  );
  assert_eq!(answer["isError"], false);
  let path = answer["content"][0]["text"]
    .as_str()
    .expect("path")
    .strip_prefix("PNG saved: ")
    .expect("saved");
  assert!(path.starts_with(".tmp/run-app/") && path.ends_with(".png"));
  let saved = worktree.join(path);
  let image = image::open(saved).expect("PNG");
  assert_eq!((image.width(), image.height()), (2, 2));
  eprintln!("app-relay: screenshot saved");

  let error = call("call_run_app", &json!({"pid":pid,"tool":"tool_error","arguments":{}}));
  assert_eq!(error["isError"], true);
  assert!(!error.to_string().contains("stand-in-app-private-token"));
  let bad_image = call("call_run_app", &json!({"pid":pid,"tool":"bad_image","arguments":{}}));
  assert_eq!(bad_image["isError"], true);
  eprintln!("app-relay: errors redacted and invalid PNG refused");
  let redirected = call("call_run_app", &json!({"pid":pid,"tool":"redirect","arguments":{}}));
  assert_eq!(redirected["isError"], true);
  assert!(!redirected.to_string().contains("stand-in-app-private-token"));
  eprintln!("app-relay: redirect refused");
  let calls = fs::read_to_string(folder.join("calls")).expect("calls");
  assert_eq!(
    calls.lines().collect::<Vec<_>>(),
    ["navigate", "lurq_screenshot", "tool_error", "bad_image", "redirect"]
  );
}

fn refuse_outside_apps(worktree: &Path, folder: &Path, call: &mut impl FnMut(&str, &Value) -> Value) {
  // These stand-ins represent an installed executable and another run: neither is this run's.
  let original = std::env::current_exe()
    .expect("CLI")
    .parent()
    .expect("bin")
    .join(if cfg!(windows) {
      "orchester-test-command.exe"
    } else {
      "orchester-test-command"
    });
  let other_run = worktree.parent().expect("worktrees").join("other-run");
  fs::create_dir_all(&other_run).expect("other run");
  let other_program = other_run.join(if cfg!(windows) {
    "orchester-app.exe"
  } else {
    "orchester-app"
  });
  fs::copy(&original, &other_program).expect("copy");
  for (index, program) in [&original, &other_program].into_iter().enumerate() {
    let outside_folder = folder.join(format!("outside-{index}"));
    let outside_app = start_program(program, &outside_folder);
    let refused = call(
      "call_run_app",
      &json!({"pid":outside_app.0.id(),"tool":"app_state","arguments":{}}),
    );
    assert_eq!(refused["isError"], true);
    assert!(
      refused["content"][0]["text"]
        .as_str()
        .expect("error")
        .contains("outside this run")
    );
    assert!(!outside_folder.join("calls").exists());
  }
  eprintln!("app-relay: installed and other run refused");
}
