//! A stand-in for `claude -p --input-format stream-json --output-format stream-json --verbose
//! --replay-user-messages` in the runner's execution tests.
//! The real CLI was never observed doing a model turn (it was not signed in during the probe), so
//! apart from the sign-in failure, which replays real output
//! (`crates/core/tests/fixtures/claude/not-signed-in-clean-env.jsonl`), the lines it prints are
//! synthetic, shaped like the events the probe recorded. It reads its input as Claude Code 2.1.286 did
//! (`fake_claude_input`): the prompt is the first user message; a message that arrives during a tool call
//! is taken after that call's result, one after its last tool call as the next turn, after the result.
//! What it does depends on the prompt:
//!
//! - "Please sign in": answers like a CLI that is not signed in and exits with 1;
//! - "Report plan limits": first prints a `rate_limit_event`, then works as the rest of the prompt says;
//! - "Ask for the web": tries `curl`, which the permissions deny, then works as below;
//! - "Work in turns": appends `draft` to `README.md`, finishes a message that used 9,739 counted tokens, starts another
//!   one and waits until it is ended;
//! - "Keep working": starts a message and waits until it is ended;
//! - "Ask first": appends `draft` to `README.md` and ends with the step result `{"outcome":"question","summary":"Keep
//!   email sign-in?"}` (a new session only: a resumed one works as below); "Ask, then fail" writes the same question,
//!   then its result reports an API error and it exits with 1 (`fake_claude_ask`);
//! - "Ask before publishing": wants to run `pnpm publish --dry-run` through Bash and asks its permission prompt
//!   (`fake_permission_prompt`); approved, it appends `approved` to `README.md` and asks about the same call once more;
//!   then works as below. "Ask before writing docs" does the same for the `write` tool of the `docs` MCP server, with
//!   the secret `docs-token-value-7f3a` in the file path, and "Try an edit outside" for a `Write` outside the worktree.
//!   "Run `<command>`" runs that command through Bash: decided by the rules (`fake_claude_rules`) it appends `ran` to
//!   `README.md` when they run it, and when they ask it asks its permission prompt as above. Each decision is reported
//!   on standard error as `permission: <call> <behavior>` (`again` for the repeated call);
//! - "Answer with structured output": writes prose and ends with the result's `structured_output` asking
//!   `{"outcome":"question","summary":"Which database should I use?"}`, as the CLI does when given `--json-schema`;
//!   "Answer in prose, then JSON" ends with prose followed by `{"outcome":"failed","summary":"The build breaks."}` and
//!   no `structured_output`; "Answer without JSON" ends with prose only;
//! - "Ask with options": calls Orchester's `ask_question` with three options, one to pick (several with "several
//!   allowed"), and reports the answer it got (`fake_claude_tools`), then works as below;
//! - "Find a stopped background task": first ends a turn of its own, before it takes the prompt, then answers with
//!   prose and the step result through `StructuredOutput` (`fake_claude_background`);
//! - "Refuse the model": answers as Claude Code 2.1.89 did when the API refused Opus 5.5 to it ("version 2.1.280 or
//!   newer is required") and exits with 1;
//! - "Leave the prompt unstarted": never starts the permission prompt's server, which Claude Code starts with the other
//!   MCP servers as it starts;
//! - "Knock first" (with "Ask before publishing"): first connects to the prompt's runner end with a wrong token, and
//!   after the decision once more with the right one, reporting `knock: ` and `second: ` (`answered` or `closed`);
//!   "Leave while waiting" sends its call, then ends the prompt server once `fake-release` exists and waits; "Give up
//!   waiting" sends its call, gives up on it once `fake-release` exists, ends with a `question` result and a denied
//!   `Grep`, and ends the prompt server only after its input ended (`fake_claude_ask`);
//! - "Wait for release": runs `pnpm test` through Bash, which ends once the test writes `fake-release` next to the
//!   worktrees (`<repo>/.orchester/`), appending `draft` to `README.md` first when the prompt also says "Draft first";
//!   then it waits until it is ended when the prompt also says "Then keep working", else works as below;
//! - "Leave a handoff": works as below, and its step result hands off like `orchester-fake-codex`'s;
//! - "Wait for a message": runs `pnpm test` through Bash until a message arrives on its input (or the test writes
//!   `fake-release`), takes the message after the call's result, writes "Noted: <message>", then works as below;
//! - "Finish after release": once the test writes `fake-release`, ends its turn with the step result and no tool call,
//!   so a message that arrived meanwhile is taken as the next turn; with "Ignore messages" it reads nothing after its
//!   prompt;
//! - "Work slowly" waits a moment before it works as below, and "Write own file" writes `<step>.txt` instead of
//!   appending to `README.md`, as `orchester-fake-codex` does;
//! - anything else: edits `README.md` in its working directory (appending `hello`), runs `git status`, and ends with
//!   the step result `{"outcome":"done","summary":"Finished <step>"}` and exit 0.
//!
//! `--resume <session>` continues that session: once no `fake-hold` lies next to the worktrees (a test holds a
//! continued session at work with it), it appends `continued` to `README.md` and ends the same way. A new
//! session takes its id from `--session-id`.
//!
//! Started with `--system-prompt-file`, it is auto mode's reviewer instead (`fake_claude_review`), or with an
//! `--mcp-config` too a turn of the Orchestrator chat (`fake_claude_chat`).
//!
//! On standard error it reports its arguments, the `--json-schema` value on its own line (`json-schema: `), the names
//! of its environment variables and its `PATH`, the prompt (its first line, then each line), its process id and its MCP
//! configuration (`fake_mcp`), like `orchester-fake-codex`.

#![allow(
  clippy::print_stdout,
  clippy::print_stderr,
  reason = "a stand-in for a CLI: printing is what it is for"
)]

use std::{fs::OpenOptions, io::Write, path::Path, process::ExitCode, thread, time::Duration};

#[path = "fake_chat_mcp.rs"]
mod fake_chat_mcp;
#[path = "fake_claude_ask.rs"]
mod fake_claude_ask;
#[path = "fake_claude_background.rs"]
mod fake_claude_background;
#[path = "fake_claude_chat.rs"]
mod fake_claude_chat;
#[path = "fake_claude_input.rs"]
mod fake_claude_input;
#[path = "fake_claude_review.rs"]
mod fake_claude_review;
#[path = "fake_claude_rules.rs"]
mod fake_claude_rules;
#[path = "fake_claude_tools.rs"]
mod fake_claude_tools;
#[path = "fake_mcp.rs"]
mod fake_mcp;
#[path = "fake_permission_prompt.rs"]
mod fake_permission_prompt;
#[path = "fake_skill_calls.rs"]
mod fake_skill_calls;
#[path = "fake_tool_calls.rs"]
mod fake_tool_calls;

const NOT_SIGNED_IN: &str = include_str!("../../../core/tests/fixtures/claude/not-signed-in-clean-env.jsonl");
/// Counted: 12 + 9,700 (cache writes) + 27 = 9,739; the 2,432 cache reads do not count.
const USAGE: &str =
  r#"{"input_tokens":12,"output_tokens":27,"cache_creation_input_tokens":9700,"cache_read_input_tokens":2432}"#;
/// A message that is still being written has not used anything yet.
const NO_USAGE: &str = r#"{"input_tokens":0,"output_tokens":0}"#;

fn main() -> ExitCode {
  let args: Vec<String> = std::env::args().skip(1).collect();
  if fake_claude_review::is_review(&args) {
    return fake_claude_review::review(&args);
  }
  let Some(prompt) = read_prompt(&args) else {
    return ExitCode::from(2);
  };
  if prompt.contains("Please sign in") {
    print!("{NOT_SIGNED_IN}");
    return ExitCode::from(1);
  }
  report_limits(&prompt);
  let after = |flag: &str| {
    args
      .iter()
      .position(|arg| arg == flag)
      .and_then(|index| args.get(index + 1))
      .cloned()
  };
  let resumed = after("--resume");
  let Some(session) = resumed.clone().or_else(|| after("--session-id")) else {
    eprintln!("no session");
    return ExitCode::from(2);
  };
  if fake_claude_chat::is_chat(&args) {
    return fake_claude_chat::answer(&args, &session, &prompt);
  }
  let Ok(worktree) = std::env::current_dir() else {
    eprintln!("no working directory");
    return ExitCode::from(2);
  };
  let fake = Fake {
    session,
    prompt: prompt.clone(),
    handoff: prompt.contains("Leave a handoff"),
  };
  let servers = if fake_mcp::report_claude(&args) {
    r#"[{"name":"broken","status":"failed"}]"#
  } else {
    "[]"
  };
  if prompt.contains(fake_claude_background::PROMPT) {
    fake_claude_background::stopped_task(&fake.session);
  }
  println!(
    r#"{{"type":"system","subtype":"init","cwd":"worktree","session_id":"{}","tools":[],"mcp_servers":{servers},"model":"claude-haiku-4-5","permissionMode":"acceptEdits","apiKeySource":"none","claude_code_version":"2.1.89"}}"#,
    fake.session
  );
  fake_claude_input::replay(&fake.session, &prompt);
  let step = prompt
    .lines()
    .find_map(|line| line.strip_prefix("# Current step: "))
    .unwrap_or("the step")
    .to_owned();
  let permission_prompt = started_prompt(&args, &prompt);
  if resumed.is_some() {
    return fake.resume(&worktree, &args, &step, permission_prompt);
  }
  fake.work(&worktree, (&args, permission_prompt), &step)
}

impl Fake {
  /// What a new session does, by its prompt.
  fn work(
    &self,
    worktree: &Path,
    (args, permission_prompt): (&[String], Option<fake_permission_prompt::Prompt>),
    step: &str,
  ) -> ExitCode {
    let prompt = &self.prompt;
    if prompt.contains("Keep working") {
      self.message("msg_wait", r#"{"type":"text","text":"Working."}"#, NO_USAGE);
      wait_to_be_ended();
    }
    if prompt.contains("Ask first") {
      return self.ask(worktree);
    }
    if prompt.contains("Ask, then fail") {
      return self.ask_then_fail();
    }
    if prompt.contains("Work in turns") {
      if let Err(error) = append(worktree, "draft") {
        eprintln!("cannot write README.md: {error}");
        return ExitCode::from(2);
      }
      self.assistant("msg_draft", r#"{"type":"text","text":"Drafted."}"#);
      self.message("msg_more", r#"{"type":"text","text":"Going on."}"#, NO_USAGE);
      wait_to_be_ended();
    }
    if prompt.contains("Refuse the model") {
      return self.refuse_the_model();
    }
    if prompt.contains("Hit the plan limit") && !account_has_room() {
      return self.hit_the_plan_limit();
    }
    if prompt.contains(fake_claude_background::PROMPT) {
      return self.structured_output(step);
    }
    if let Some((text, structured)) = answer(prompt) {
      return self.answer(&text, structured);
    }
    if prompt.contains("Use the file tools") {
      return self.use_file_tools(worktree, (args, permission_prompt), step);
    }
    if prompt.contains("Use the process tools") {
      return self.use_process_tools(worktree, (args, permission_prompt), step);
    }
    if prompt.contains("Use the run tools") {
      return self.use_run_tools(worktree, (args, permission_prompt), step);
    }
    if prompt.contains("Use the skill tools") {
      return self.use_skill_tools(worktree, (args, permission_prompt), step);
    }
    if prompt.contains("Use Orchester's tools") {
      return self.use_orchester_tools(worktree, (args, permission_prompt), step);
    }
    if prompt.contains("Ask with options") {
      return self.ask_with_options(worktree, (args, permission_prompt), step);
    }
    if let Some(ask) = fake_claude_ask::Ask::of(prompt) {
      return self.ask_prompt(worktree, (args, permission_prompt), &ask, step);
    }
    if prompt.contains("Wait for release") {
      self.wait_for_release(worktree, prompt);
    }
    if prompt.contains("Wait for a message") {
      self.wait_for_a_message(worktree);
    }
    if prompt.contains("Finish after release") {
      return self.finish_after_release(worktree, step);
    }
    if prompt.contains("Ask for the web") {
      return self.ask_for_the_web(worktree, step);
    }
    self.finish(worktree, "hello", step)
  }
}

/// The permission prompt's server: Claude Code starts its MCP servers, that one among them, as it
/// starts. A test that knocks first opens it only after the knock.
fn started_prompt(args: &[String], prompt: &str) -> Option<fake_permission_prompt::Prompt> {
  if prompt.contains("Knock first") || prompt.contains("Leave the prompt unstarted") {
    return None;
  }
  fake_permission_prompt::Prompt::open(args)
}

/// Whether the account the fake runs on has room left: its folder holds `plan-room`.
fn account_has_room() -> bool {
  std::env::var_os("CLAUDE_CONFIG_DIR").is_some_and(|folder| Path::new(&folder).join("plan-room").exists())
}

/// "Report plan limits": prints the `rate_limit_event` Claude Code 2.1.89 printed.
fn report_limits(prompt: &str) {
  if prompt.contains("Report plan limits") {
    print!(
      "{}",
      include_str!("../../../core/tests/fixtures/claude/plan-limits.jsonl")
    );
  }
}

/// Reads the prompt, the first user message on standard input, and reports on standard error how
/// the CLI was started.
fn read_prompt(args: &[String]) -> Option<String> {
  let mut names: Vec<String> = std::env::vars_os()
    .map(|(name, _)| name.to_string_lossy().into_owned())
    .collect();
  names.sort();
  let Some(prompt) = fake_claude_input::open() else {
    eprintln!("cannot read the prompt");
    return None;
  };
  eprintln!("argv: {}", args.join(" "));
  if let Some(schema) = args
    .iter()
    .position(|arg| arg == "--json-schema")
    .and_then(|at| args.get(at + 1))
  {
    eprintln!("json-schema: {schema}");
  }
  eprintln!("env: {}", names.join(","));
  if let Some(folder) = std::env::var_os("CLAUDE_CONFIG_DIR") {
    eprintln!("account folder: {}", folder.to_string_lossy());
  }
  eprintln!(
    "path: {}",
    std::env::var_os("PATH").unwrap_or_default().to_string_lossy()
  );
  eprintln!("prompt: {}", prompt.lines().next().unwrap_or_default());
  for line in prompt.lines() {
    eprintln!("prompt| {line}");
  }
  eprintln!("pid: {}", std::process::id());
  Some(prompt)
}

/// The final text and `structured_output` of an "Answer …" prompt.
fn answer(prompt: &str) -> Option<(String, Option<serde_json::Value>)> {
  if prompt.contains("Answer with structured output") {
    let question = serde_json::json!({"outcome": "question", "summary": "Which database should I use?"});
    return Some(("I need a decision before I go on.".to_owned(), Some(question)));
  }
  if prompt.contains("Answer in prose, then JSON") {
    let text = "I found a problem.\n\n{\"outcome\":\"failed\",\"summary\":\"The build breaks.\"}";
    return Some((text.to_owned(), None));
  }
  prompt
    .contains("Answer without JSON")
    .then(|| ("Everything looks fine to me.".to_owned(), None))
}

const DENIAL: &str =
  r#"[{"tool_name":"Bash","tool_use_id":"toolu_web","tool_input":{"command":"curl https://example.com"}}]"#;

struct Fake {
  session: String,
  prompt: String,
  /// Its step result hands off.
  handoff: bool,
}

impl Fake {
  fn assistant(&self, id: &str, block: &str) {
    self.message(id, block, USAGE);
  }

  fn message(&self, id: &str, block: &str, usage: &str) {
    println!(
      r#"{{"type":"assistant","message":{{"id":"{id}","role":"assistant","type":"message","content":[{block}],"usage":{usage}}},"parent_tool_use_id":null,"session_id":"{}"}}"#,
      self.session
    );
    flush();
  }

  fn tool_result(&self, id: &str, content: &str, is_error: bool) {
    println!(
      r#"{{"type":"user","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{id}","content":"{content}","is_error":{is_error}}}]}},"session_id":"{}"}}"#,
      self.session
    );
  }

  /// A command that runs until the test releases it.
  fn wait_for_release(&self, worktree: &Path, prompt: &str) {
    if prompt.contains("Draft first") && append(worktree, "draft").is_err() {
      eprintln!("cannot write README.md");
    }
    self.assistant(
      "msg_test",
      r#"{"type":"tool_use","id":"toolu_test","name":"Bash","input":{"command":"pnpm test"}}"#,
    );
    let release = worktree
      .parent()
      .and_then(Path::parent)
      .map(|orchester| orchester.join("fake-release"));
    while !release.as_ref().is_some_and(|release| release.exists()) {
      thread::sleep(Duration::from_millis(50));
    }
    self.tool_result("toolu_test", "ok", false);
    if prompt.contains("Then keep working") {
      self.message("msg_wait", r#"{"type":"text","text":"Working."}"#, NO_USAGE);
      wait_to_be_ended();
    }
  }

  /// "Wait for a message": a command that runs until a message arrives, which the agent takes after the
  /// command's result.
  fn wait_for_a_message(&self, worktree: &Path) {
    self.assistant(
      "msg_test",
      r#"{"type":"tool_use","id":"toolu_test","name":"Bash","input":{"command":"pnpm test"}}"#,
    );
    let release = worktree
      .parent()
      .and_then(Path::parent)
      .map(|orchester| orchester.join("fake-release"));
    let message = loop {
      if let Some(message) = fake_claude_input::next(Duration::from_millis(50)) {
        break Some(message);
      }
      if release.as_ref().is_some_and(|release| release.exists()) {
        break None;
      }
    };
    self.tool_result("toolu_test", "ok", false);
    if let Some(message) = message {
      fake_claude_input::replay(&self.session, &message);
      let noted = serde_json::json!({"type": "text", "text": format!("Noted: {message}")});
      self.assistant("msg_noted", &noted.to_string());
    }
  }

  /// "Finish after release": the step result without another tool call, once released.
  fn finish_after_release(&self, worktree: &Path, step: &str) -> ExitCode {
    let release = worktree
      .parent()
      .and_then(Path::parent)
      .map(|orchester| orchester.join("fake-release"));
    self.message("msg_wait", r#"{"type":"text","text":"Working."}"#, NO_USAGE);
    while !release.as_ref().is_some_and(|release| release.exists()) {
      thread::sleep(Duration::from_millis(50));
    }
    let result = serde_json::json!({"outcome": "done", "summary": format!("Finished {step}")}).to_string();
    self.answer(&result, None)
  }

  /// "Ask for the web": `curl`, which the permissions deny, then the usual work.
  fn ask_for_the_web(&self, worktree: &Path, step: &str) -> ExitCode {
    self.assistant(
      "msg_web",
      r#"{"type":"tool_use","id":"toolu_web","name":"Bash","input":{"command":"curl https://example.com"}}"#,
    );
    self.tool_result(
      "toolu_web",
      "Claude requested permissions to use Bash, but you haven't granted it yet.",
      true,
    );
    self.finish_with(worktree, "hello", step, DENIAL)
  }

  /// The API's refusal of the model to an old CLI, as 2.1.89 printed it.
  fn refuse_the_model(&self) -> ExitCode {
    let refusal = "API Error: 400 {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",\"message\":\"Claude \
                   Code 2.1.89 does not support this model; version 2.1.280 or newer is required. Run 'claude update', \
                   or update the Claude desktop app, then try again.\"}}";
    let message = serde_json::json!({
      "type": "assistant", "error": "invalid_request", "session_id": self.session,
      "message": {"id": "msg_refused", "role": "assistant", "type": "message",
        "content": [{"type": "text", "text": refusal}], "usage": {"input_tokens": 0, "output_tokens": 0}},
    });
    println!("{message}");
    let result = serde_json::json!({
      "type": "result", "subtype": "success", "is_error": true, "result": refusal, "session_id": self.session,
      "usage": {"input_tokens": 0, "output_tokens": 0}, "permission_denials": [],
    });
    println!("{result}");
    ExitCode::from(1)
  }

  /// "Hit the plan limit": reports its account's five-hour window reached, resetting in an hour (or in the seconds
  /// its account folder's `plan-reset` says), and refuses the turn as Claude Code does, unless its account folder
  /// holds `plan-room`.
  fn hit_the_plan_limit(&self) -> ExitCode {
    // Without a readable `plan-reset` the window resets in an hour.
    let after = std::env::var_os("CLAUDE_CONFIG_DIR")
      .and_then(|folder| std::fs::read_to_string(Path::new(&folder).join("plan-reset")).ok())
      .and_then(|seconds| seconds.trim().parse::<u64>().ok())
      .unwrap_or(3600);
    let resets = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .map_or(0, |now| now.as_secs() + after);
    println!(
      r#"{{"type":"rate_limit_event","rate_limit_info":{{"status":"rejected","resetsAt":{resets},"rateLimitType":"five_hour","utilization":1.0,"isUsingOverage":false}}}}"#
    );
    let refusal = "Claude AI usage limit reached";
    println!(
      "{}",
      serde_json::json!({
        "type": "result", "subtype": "success", "is_error": true, "result": refusal, "session_id": self.session,
        "usage": {"input_tokens": 0, "output_tokens": 0}, "permission_denials": [],
      })
    );
    ExitCode::from(1)
  }

  /// Ends with the message `text` and, when given, the result's `structured_output`.
  fn answer(&self, text: &str, structured: Option<serde_json::Value>) -> ExitCode {
    self.assistant(
      "msg_answer",
      &serde_json::json!({"type": "text", "text": text}).to_string(),
    );
    let mut result = serde_json::json!({
      "type": "result", "subtype": "success", "is_error": false, "num_turns": 1, "result": text,
      "session_id": self.session, "usage": {"input_tokens": 0, "output_tokens": 0}, "permission_denials": [],
    });
    if let Some(structured) = structured {
      result["structured_output"] = structured;
    }
    println!("{result}");
    flush();
    fake_claude_input::later_turns(&self.session);
    ExitCode::SUCCESS
  }

  /// "Ask first": a draft, then a question as the step result.
  fn ask(&self, worktree: &Path) -> ExitCode {
    if let Err(error) = append(worktree, "draft") {
      eprintln!("cannot write README.md: {error}");
      return ExitCode::from(2);
    }
    let question = r#"{\"outcome\":\"question\",\"summary\":\"Keep email sign-in?\"}"#;
    self.assistant("msg_ask", &format!(r#"{{"type":"text","text":"{question}"}}"#));
    println!(
      r#"{{"type":"result","subtype":"success","is_error":false,"num_turns":1,"result":"{question}","session_id":"{}","total_cost_usd":0.1,"usage":{{"input_tokens":0,"output_tokens":0}},"permission_denials":[]}}"#,
      self.session
    );
    flush();
    fake_claude_input::later_turns(&self.session);
    ExitCode::SUCCESS
  }

  fn finish(&self, worktree: &Path, line: &str, step: &str) -> ExitCode {
    self.finish_with(worktree, line, step, "[]")
  }

  /// Appends `line` to `README.md` through an edit, runs `git status`, then ends with the step result.
  fn finish_with(&self, worktree: &Path, line: &str, step: &str, denials: &str) -> ExitCode {
    if self.prompt.contains("Work slowly") {
      thread::sleep(Duration::from_millis(1_500));
    }
    let written = if self.prompt.contains("Write own file") {
      let file = format!("{}.txt", step.to_lowercase().replace(' ', "-"));
      std::fs::write(worktree.join(file), format!("{line}\n"))
    } else {
      append(worktree, line)
    };
    if let Err(error) = written {
      eprintln!("cannot write the file: {error}");
      return ExitCode::from(2);
    }
    self.assistant(
      "msg_edit",
      r#"{"type":"tool_use","id":"toolu_edit","name":"Edit","input":{"file_path":"README.md","old_string":"hi","new_string":"hi"}}"#,
    );
    self.tool_result("toolu_edit", "The file README.md has been updated.", false);
    self.assistant(
      "msg_git",
      r#"{"type":"tool_use","id":"toolu_git","name":"Bash","input":{"command":"git status --short"}}"#,
    );
    self.tool_result("toolu_git", " M README.md", false);
    println!("a line that is not JSON");
    let handoff = if self.handoff {
      format!(
        concat!(
          r#",\"completed\":[\"Appended {line}\"],\"decisions\":[\"Greet in README.md from {step}\"],"#,
          r#"\"open_questions\":null,\"next_agent_should_know\":[\"README.md ends with {line}\"]"#
        ),
        line = line,
        step = step
      )
    } else {
      String::new()
    };
    let result = format!(r#"{{\"outcome\":\"done\",\"summary\":\"Finished {step}\"{handoff}}}"#);
    self.assistant("msg_done", &format!(r#"{{"type":"text","text":"{result}"}}"#));
    println!(
      r#"{{"type":"result","subtype":"success","is_error":false,"num_turns":3,"result":"{result}","session_id":"{}","total_cost_usd":0.5,"usage":{{"input_tokens":0,"output_tokens":0}},"permission_denials":{denials}}}"#,
      self.session
    );
    flush();
    fake_claude_input::later_turns(&self.session);
    ExitCode::SUCCESS
  }
}

fn append(worktree: &Path, line: &str) -> std::io::Result<()> {
  OpenOptions::new()
    .append(true)
    .open(worktree.join("README.md"))
    .and_then(|mut readme| writeln!(readme, "{line}"))
}

/// Waits while the test holds continued sessions with `fake-hold` next to the worktrees.
fn wait_while_held(worktree: &Path) {
  let hold = worktree
    .parent()
    .and_then(Path::parent)
    .map(|orchester| orchester.join("fake-hold"));
  while hold.as_ref().is_some_and(|hold| hold.exists()) {
    thread::sleep(Duration::from_millis(50));
  }
}

fn wait_to_be_ended() -> ! {
  flush();
  thread::sleep(Duration::from_secs(600));
  std::process::exit(0)
}

fn flush() {
  if std::io::stdout().flush().is_err() {
    eprintln!("cannot flush standard output");
  }
}
