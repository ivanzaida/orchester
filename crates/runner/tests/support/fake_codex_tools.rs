//! How `orchester-fake-codex` starts and uses Orchester's server, as Codex 0.159.2 does: it starts the server its
//! `-c mcp_servers.Orchester.…` overrides configure, handing it the variables `env_vars` names from its own
//! environment, and lists its tools ([`start`]) once for the thread and once more for each
//! `mcpServerStatus/list`, which then closes it; a list that has no answer within [`LIST_WAIT`] is Codex's
//! *timed out awaiting tools/list*. With "Use Orchester's tools" the thread's server lists the tools
//! (`orchester-tools: <names>`), then makes the calls outside the worktree of [`fake_tool_calls::external`]
//! (`orchester-call: <tool> <ok|error>: <first line>`). Codex sends no approval request for a tool whose approval
//! mode is `approve`, so neither does the fake: the runner asks about a call that needs it before it answers. With
//! "Use the skill tools" it makes the calls of [`fake_skill_calls`], with "Use the file tools" those of a read-only
//! step ([`fake_tool_calls::files`]), with "Use the browser tools" those of [`fake_browser_tools`].

use std::{path::Path, process::Command, sync::mpsc, thread, time::Duration};

use serde_json::{Value, json};

use super::fake_permission_prompt::Prompt;

#[path = "fake_app_tools.rs"]
mod fake_app_tools;

#[path = "fake_browser_tools.rs"]
mod fake_browser_tools;

#[path = "fake_native_tools.rs"]
mod fake_native_tools;

#[path = "fake_process_tools.rs"]
mod fake_process_tools;

#[path = "fake_skill_calls.rs"]
mod fake_skill_calls;
#[path = "fake_tool_calls.rs"]
mod fake_tool_calls;

const KEY: &str = "mcp_servers.Orchester.";
/// How long the fake waits for the tools of Orchester's server; Codex waits 30 seconds. A relay the runner
/// refuses lists no tools only after its connection failed, which takes Windows about two seconds.
const LIST_WAIT: Duration = Duration::from_secs(6);

/// The calls of "Use Orchester's tools", "Use the file tools" (in a read-only step: no edit) and "Use the
/// index tools" (after `src/lib.rs` changed in the worktree, with a file the index does not know).
fn calls(prompt: &str, worktree: &Path) -> Vec<(&'static str, Value)> {
  let named =
    |calls: Vec<(&'static str, &'static str, Value)>| calls.into_iter().map(|(_, tool, input)| (tool, input)).collect();
  if prompt.contains("Use the skill tools") {
    return named(fake_skill_calls::calls(worktree));
  }
  if prompt.contains("Use the index tools") {
    let changed = "// moved down\n// moved down\npub fn parse_config() {}\n";
    if std::fs::write(worktree.join("src").join("lib.rs"), changed).is_err()
      || std::fs::write(worktree.join("src").join("fresh.rs"), "pub fn fresh() {}\n").is_err()
    {
      eprintln!("cannot change the sources");
    }
    return vec![
      ("find_symbol", json!({"name": "parse_config"})),
      ("find_symbol", json!({"name": "config", "kind": "struct"})),
      ("search_file", json!({"term": "parse_config"})),
      ("list_files", json!({"pattern": "*.rs", "path": null})),
    ];
  }
  if prompt.contains("Use the file tools") {
    return named(fake_tool_calls::files(true));
  }
  if prompt.contains("Use the run tools") {
    return named(fake_tool_calls::run_tools());
  }
  if prompt.contains("Run the check twice") {
    return named(fake_tool_calls::check_twice());
  }
  named(fake_tool_calls::external(worktree))
}

/// Orchester's server started by Codex.
pub struct Started {
  /// The server, once its tools were listed.
  pub server: Option<Prompt>,
  /// Its tools, or why Codex has none.
  pub tools: Result<Vec<String>, String>,
}

/// Orchester's server as the arguments configure it, started; `None` when the arguments configure no such
/// server.
pub fn start(args: &[String]) -> Option<Started> {
  let command = command(args)?;
  let (sender, answer) = mpsc::channel();
  // A relay the runner does not serve never answers: the list waits on its own thread.
  thread::spawn(move || {
    let started = Prompt::start(command, "approve").map(|mut server| {
      let tools = server.list();
      (server, tools)
    });
    sender.send(started).ok();
  });
  let (server, tools) = match answer.recv_timeout(LIST_WAIT) {
    Ok(Some((server, Some(tools)))) => (Some(server), Ok(tools)),
    Ok(Some((server, None))) => (Some(server), Err("tools/list failed".to_owned())),
    Ok(None) => (None, Err("MCP startup failed: the server did not start".to_owned())),
    Err(_) => (
      None,
      Err("MCP startup failed: timed out awaiting tools/list after 30s".to_owned()),
    ),
  };
  Some(Started { server, tools })
}

/// The setting `name` of Orchester's server in the arguments.
pub fn setting(args: &[String], name: &str) -> Option<Value> {
  let value = args
    .iter()
    .find_map(|arg| arg.strip_prefix(KEY)?.strip_prefix(name)?.strip_prefix('='))?;
  // The values the runner writes are TOML basic strings and arrays of them, which read as JSON.
  serde_json::from_str(value).ok()
}

/// The command of Orchester's server the arguments configure, with the variables Codex hands it.
fn command(args: &[String]) -> Option<Command> {
  let (program, arguments, address, handed) = (
    setting(args, "command")?,
    setting(args, "args")?,
    setting(args, "env.ORCHESTER_APPROVAL_ADDRESS")?,
    setting(args, "env_vars")?,
  );
  let mut command = Command::new(program.as_str().unwrap_or_default());
  for arg in arguments.as_array().into_iter().flatten() {
    command.arg(arg.as_str().unwrap_or_default());
  }
  command.env("ORCHESTER_APPROVAL_ADDRESS", address.as_str().unwrap_or_default());
  for name in handed.as_array().into_iter().flatten().filter_map(|name| name.as_str()) {
    if let Ok(value) = std::env::var(name) {
      command.env(name, value);
    }
  }
  Some(command)
}

/// Lists and calls Orchester's tools through the thread's server, `None` when the thread has none.
pub fn report(server: Option<&mut Prompt>, worktree: &Path, prompt: &str) {
  let Some(server) = server else {
    eprintln!("orchester-tools: no server");
    return;
  };
  eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
  if prompt.contains("Call the run app") {
    fake_app_tools::exercise(worktree, false, |tool, arguments| {
      server.call_result(tool, arguments).expect("relay")
    });
    return;
  }
  if prompt.contains("Use the process tools") {
    fake_process_tools::exercise(worktree, |tool, arguments| {
      report_call(server, tool, arguments);
    });
    return;
  }
  if prompt.contains("Use the browser tools") {
    fake_browser_tools::exercise(worktree, &mut |tool, arguments| server.call_result(tool, arguments));
    return;
  }
  if prompt.contains("Start the QA app") {
    let scenario = prompt
      .split("QA scenario ")
      .nth(1)
      .and_then(|rest| rest.split('.').next())
      .unwrap_or("tasks");
    report_call(server, "start_qa_app", &json!({"scenario": scenario}));
    report_call(server, "call_run_app", &json!({"tool": "app_state", "arguments": {}}));
    if prompt.contains("Then read outside") {
      read_outside(server, worktree);
      report_call(server, "call_run_app", &json!({"tool": "app_state", "arguments": {}}));
      if prompt.contains("Then start again") {
        report_call(server, "start_qa_app", &json!({"scenario": scenario}));
      }
    }
    return;
  }
  if prompt.contains("Start the service ") || prompt.contains("Start the stack ") {
    services(server, worktree, prompt);
    return;
  }
  if prompt.contains("Run the suite ") {
    suites(server, prompt);
    return;
  }
  for (tool, arguments) in calls(prompt, worktree) {
    report_call(server, tool, &arguments);
  }
}

/// The name after `lead` in `prompt`, up to the next `.`.
fn named_after<'a>(prompt: &'a str, lead: &str) -> Option<&'a str> {
  prompt.split(lead).nth(1).and_then(|rest| rest.split('.').next())
}

/// "Start the service <name>." calls `start_service`, "Start the stack <name>." `start_stack`; with "Then start the
/// service <other>." it then calls `start_service` of that one; with "Then drive its UI" it then calls the `ui_*` tools
/// on it ([`drive_ui`]); with "Copy the native fixture.", "Then drive the native window" and "Then open its UI" it puts
/// the fixture window in the worktree before the start and drives or opens it after ([`fake_native_tools`]); with
/// "Then read outside" it then reads
/// `deps-a/notes.txt` of the repository, which asks; with "Then wait for it to end" it calls `service_status` until the
/// service is no longer ready; with "Then wait for a crash notice" it calls `service_status` until an answer carries a
/// notice ([`crash_notice`]); with "Then look at the service" it calls `service_status` and `service_logs`; and with
/// "Then stop the service" it calls `stop_service`, then `service_status` again when it also looks.
fn services(server: &mut Prompt, worktree: &Path, prompt: &str) {
  let stack = named_after(prompt, "Start the stack ");
  let name = named_after(prompt, "Start the service ").unwrap_or("web");
  let named = json!({"name": name});
  let look = prompt.contains("Then look at the service");
  if prompt.contains("Copy the native fixture.") {
    fake_native_tools::copy_fixture(worktree);
  }
  match stack {
    Some(stack) => report_call(server, "start_stack", &json!({"name": stack})),
    None => report_call(server, "start_service", &named),
  };
  if let Some(other) = named_after(prompt, "Then start the service ") {
    report_call(server, "start_service", &json!({"name": other}));
  }
  if prompt.contains("Then drive its UI") {
    drive_ui(server, name);
  }
  if prompt.contains("Then drive the native window") {
    fake_native_tools::drive(name, &mut |tool, arguments| report_call(server, tool, arguments));
  }
  if prompt.contains("Then open its UI") {
    report_call(server, "ui_open", &json!({"service": name}));
  }
  if prompt.contains("Then read outside") {
    read_outside(server, worktree);
  }
  if prompt.contains("Then wait for it to end") {
    for _ in 0..80 {
      let ready = server
        .call("service_status", &named)
        .is_some_and(|(text, _)| text.contains(r#""state":"ready""#));
      if !ready {
        break;
      }
      thread::sleep(Duration::from_millis(250));
    }
  }
  if prompt.contains("Then wait for a crash notice") {
    crash_notice(server, &named);
  }
  if look {
    report_call(server, "service_status", &named);
    report_call(server, "service_logs", &json!({"name": name, "tail": 50}));
  }
  if prompt.contains("Then stop the service") {
    report_call(server, "stop_service", &named);
    if look {
      report_call(server, "service_status", &named);
    }
  }
}

/// "Then drive its UI": a snapshot before any `ui_open`, which is refused, then `ui_open` of the service, a snapshot of
/// the whole tree and one of a query, each `ui_act` action, a screenshot, the log, a wait for it to render and one for
/// an element that shows only after a few looks, and the service's status and log through the service tools.
fn drive_ui(server: &mut Prompt, name: &str) {
  report_call(server, "ui_snapshot", &json!({}));
  report_call(server, "ui_open", &json!({"service": name}));
  report_call(server, "ui_snapshot", &json!({}));
  report_call(server, "ui_snapshot", &json!({"query": "Save"}));
  report_call(server, "ui_act", &json!({"ref": "ref_2", "action": "click"}));
  report_call(
    server,
    "ui_act",
    &json!({"ref": "ref_3", "action": "type", "text": "Fix login"}),
  );
  report_call(server, "ui_act", &json!({"action": "key", "text": "Enter"}));
  report_call(server, "ui_act", &json!({"ref": "ref_3", "action": "scroll"}));
  report_call(
    server,
    "ui_act",
    &json!({"ref": "ref_4", "action": "select", "text": "High"}),
  );
  report_call(server, "ui_screenshot", &json!({}));
  report_call(server, "ui_logs", &json!({"lines": 5}));
  report_call(server, "ui_wait", &json!({"condition": "idle"}));
  report_call(
    server,
    "ui_wait",
    &json!({"condition": "element", "query": "Saved", "timeout_ms": 10000}),
  );
  report_call(server, "service_status", &json!({"name": name}));
  report_call(server, "service_logs", &json!({"name": name}));
}

/// Calls `service_status` of `named` until its answer carries Orchester's notices after it, as text blocks after the
/// first, and reports each notice's lines as `orchester-notice| <line>`; `orchester-notice: none` when none came.
fn crash_notice(server: &mut Prompt, named: &Value) {
  for _ in 0..120 {
    let notices: Vec<String> = server
      .call_result("service_status", named)
      .map(|result| {
        result["content"]
          .as_array()
          .into_iter()
          .flatten()
          .skip(1)
          .filter_map(|block| block["text"].as_str().map(str::to_owned))
          .collect()
      })
      .unwrap_or_default();
    if !notices.is_empty() {
      for line in notices.iter().flat_map(|notice| notice.lines()) {
        eprintln!("orchester-notice| {line}");
      }
      return;
    }
    thread::sleep(Duration::from_millis(250));
  }
  eprintln!("orchester-notice: none");
}
/// Reads `deps-a/notes.txt` of the repository, outside the worktree, which asks.
fn read_outside(server: &mut Prompt, worktree: &Path) {
  let notes = fake_tool_calls::repository_folder(worktree, "deps-a").join("notes.txt");
  report_call(
    server,
    "read_external_file",
    &json!({"filename": notes.to_string_lossy()}),
  );
}

/// "Run the suite <name>." calls `run_tests` of that suite, "Run the suite <name> with filter <filter>." narrowed by
/// the filter, each such sentence once, in order; with "Then read the output of <test>." it then calls `test_output`
/// of that test of the last run `run_tests` answered.
fn suites(server: &mut Prompt, prompt: &str) {
  let mut last_run = None;
  for sentence in prompt
    .split("Run the suite ")
    .skip(1)
    .filter_map(|rest| rest.split('.').next())
  {
    let (suite, filter) = match sentence.split_once(" with filter ") {
      Some((suite, filter)) => (suite, Some(filter)),
      None => (sentence, None),
    };
    let answer = report_call(server, "run_tests", &json!({"suite": suite, "filter": filter}));
    let run = answer
      .and_then(|text| serde_json::from_str::<Value>(&text).ok())
      .and_then(|answer| answer["run"].as_str().map(str::to_owned));
    last_run = run.or(last_run);
  }
  if let (Some(test), Some(run)) = (named_after(prompt, "Then read the output of "), last_run) {
    report_call(server, "test_output", &json!({"run": run, "test": test}));
  }
}

/// Calls `tool` with `arguments` and reports its answer; the answer's text, when it came.
fn report_call(server: &mut Prompt, tool: &str, arguments: &Value) -> Option<String> {
  match server.call(tool, arguments) {
    Some((text, failed)) => {
      eprintln!(
        "orchester-call: {tool} {}: {}",
        if failed { "error" } else { "ok" },
        text.lines().next().unwrap_or_default()
      );
      for line in text.lines() {
        eprintln!("orchester-answer| {line}");
      }
      Some(text)
    }
    None => {
      eprintln!("orchester-call: {tool} no answer");
      None
    }
  }
}
