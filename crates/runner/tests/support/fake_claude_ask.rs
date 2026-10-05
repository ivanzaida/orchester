//! How `orchester-fake-claude` asks its permission prompt about a tool call (`fake_permission_prompt`), and takes up
//! a call its session was ended while waiting on. A denied call is answered with the prompt's message, which the
//! fake reports (`told: <message>`), and the fake goes on (`went on` in `README.md`), as Claude Code's agent does.
//! A prompt with "twice" makes a second call of the same tool after the first is approved (`toolu_…_second`, its
//! command's last word replaced by `tokio` for a command), which appends `approved` again when it is approved.

use std::{fs, path::Path, process::ExitCode};

use super::{Fake, append, fake_claude_rules, fake_permission_prompt, flush, wait_to_be_ended, wait_while_held};

/// A tool call the fake asks its permission prompt about.
pub struct Ask {
  call: String,
  tool: String,
  input: serde_json::Value,
}

impl Ask {
  pub fn of(prompt: &str) -> Option<Self> {
    // "Run `<command>`.": that command.
    let run = prompt
      .split_once("Run `")
      .and_then(|(_, rest)| rest.split_once('`'))
      .map(|(command, _)| command);
    let (call, tool, input) = if let Some(command) = run {
      ("toolu_run", "Bash", serde_json::json!({"command": command}))
    } else if prompt.contains("Ask before publishing") {
      (
        "toolu_publish",
        "Bash",
        serde_json::json!({"command": "pnpm publish --dry-run", "description": "Publish a dry run"}),
      )
    } else if prompt.contains("Ask before writing docs") {
      (
        "toolu_docs",
        "mcp__docs__write",
        serde_json::json!({"filePath": "docs/docs-token-value-7f3a.md", "text": "notes"}),
      )
    } else if prompt.contains("Read the design") {
      (
        "toolu_design_read",
        "mcp__pencil__get_screenshot",
        serde_json::json!({"nodeId": "a1"}),
      )
    } else if prompt.contains("Change the design") {
      (
        "toolu_design_change",
        "mcp__pencil__batch_design",
        serde_json::json!({"operations": "I(a1)"}),
      )
    } else if prompt.contains("Make a folder") {
      (
        "toolu_mkdir",
        "Bash",
        serde_json::json!({"command": "mkdir build-output"}),
      )
    } else if prompt.contains("Remove the build") {
      ("toolu_rm", "Bash", serde_json::json!({"command": "rm -rf build"}))
    } else if prompt.contains("Try an edit outside") {
      (
        "toolu_outside",
        "Write",
        serde_json::json!({"file_path": "/elsewhere/notes.md", "content": "x"}),
      )
    } else {
      return None;
    };
    Some(Self {
      call: call.to_owned(),
      tool: tool.to_owned(),
      input,
    })
  }
}

impl Fake {
  /// Asks the permission prompt about `ask`, carries it out when approved, then finishes. `opened` is
  /// the prompt's server when it was started already.
  pub fn ask_prompt(
    &self,
    worktree: &Path,
    (args, opened): (&[String], Option<fake_permission_prompt::Prompt>),
    ask: &Ask,
    step: &str,
  ) -> ExitCode {
    let block = serde_json::json!({"type": "tool_use", "id": ask.call, "name": ask.tool, "input": ask.input});
    self.assistant("msg_ask", &block.to_string());
    let command = ask.input.get("command").and_then(serde_json::Value::as_str);
    let rule = match command {
      Some(command) => fake_claude_rules::bash(args, command),
      None if ask.tool.starts_with("mcp__") => fake_claude_rules::mcp(args, &ask.tool),
      None => fake_claude_rules::Decision::Ask,
    };
    if rule != fake_claude_rules::Decision::Ask {
      eprintln!("permission: {} {}", &ask.call, rule.as_str());
      let ran = rule.runs();
      if ran && append(worktree, "ran").is_err() {
        eprintln!("cannot write README.md");
      }
      self.tool_result(&ask.call, if ran { "ok" } else { "Denied." }, !ran);
      return self.finish(worktree, "hello", step);
    }
    let knocks = self.prompt.contains("Knock first");
    if knocks {
      eprintln!("knock: {}", fake_permission_prompt::knock(args, Some("not-the-token")));
    }
    let Some(mut prompt) = opened.or_else(|| fake_permission_prompt::Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    if self.prompt.contains("Leave while waiting") {
      prompt.leave_while_waiting(&ask.tool, &ask.input, &ask.call, &released(worktree));
      wait_to_be_ended();
    }
    if self.prompt.contains("Give up waiting") {
      return self.give_up_waiting(worktree, prompt, ask);
    }
    // As the real CLI's transcript would, the session keeps the call without a result while it
    // waits: a runner that ends the CLI now leaves it unanswered for `--resume`.
    let pending = pending_call(worktree, &self.session);
    let kept = serde_json::json!({"call": ask.call, "tool": ask.tool, "input": ask.input});
    if let Err(error) = fs::write(&pending, kept.to_string()) {
      eprintln!("cannot keep the pending call: {error}");
    }
    let (decision, told) = prompt
      .ask(&ask.tool, &ask.input, &ask.call)
      .unwrap_or_else(|| ("none".to_owned(), String::new()));
    if fs::remove_file(&pending).is_err() {
      eprintln!("the pending call was not kept");
    }
    eprintln!("permission: {} {decision}", &ask.call);
    if decision != "allow" {
      self.denied(worktree, &ask.call, &told);
      flush();
      return self.finish(worktree, "hello", step);
    }
    if let Err(error) = append(worktree, "approved") {
      eprintln!("cannot write README.md: {error}");
      return ExitCode::from(2);
    }
    self.tool_result(&ask.call, "ok", false);
    let (again, _) = prompt
      .ask(&ask.tool, &ask.input, &ask.call)
      .unwrap_or_else(|| ("none".to_owned(), String::new()));
    eprintln!("permission again: {} {again}", &ask.call);
    if knocks {
      eprintln!("second: {}", fake_permission_prompt::knock(args, None));
    }
    if self.prompt.to_lowercase().contains("twice") {
      self.ask_second(worktree, &mut prompt, ask);
    }
    flush();
    self.finish(worktree, "hello", step)
  }

  /// "Give up waiting" (run 26 of the user's ORC-21): the call waits for a person until the test writes
  /// `fake-release`, then fails as Claude Code fails a permission prompt call silent for 30 minutes, while the
  /// prompt's server stays blocked on it. The agent then ends its turn with a `question` result through its
  /// `StructuredOutput` tool, the result lists a denied `Grep`, and once its input ends the CLI ends the prompt's
  /// server before it exits, so the runner sees the prompt close after the result.
  fn give_up_waiting(&self, worktree: &Path, mut prompt: fake_permission_prompt::Prompt, ask: &Ask) -> ExitCode {
    prompt.send_call(&ask.tool, &ask.input, &ask.call);
    let release = released(worktree);
    while !release.exists() {
      std::thread::sleep(std::time::Duration::from_millis(50));
    }
    self.tool_result(&ask.call, GAVE_UP, true);
    let question = serde_json::json!({"outcome": "question", "summary": "May I run the pin helper?"});
    let block = serde_json::json!({"type": "tool_use", "id": "toolu_structured", "name": "StructuredOutput",
      "input": question});
    self.assistant("msg_structured", &block.to_string());
    self.tool_result("toolu_structured", "Structured output provided successfully", false);
    let denials = serde_json::json!([{"tool_name": "Grep", "tool_use_id": "toolu_grep",
      "tool_input": {"pattern": "lurq_[a-z_]+", "path": "/elsewhere/lurq"}}]);
    let result = serde_json::json!({
      "type": "result", "subtype": "success", "is_error": false, "num_turns": 4, "result": "",
      "session_id": self.session, "usage": {"input_tokens": 0, "output_tokens": 0},
      "permission_denials": denials, "structured_output": question,
    });
    println!("{result}");
    flush();
    super::fake_claude_input::later_turns(&self.session);
    drop(prompt);
    // The runner hears of the closed prompt before the CLI's exit, as it did in run 26.
    std::thread::sleep(std::time::Duration::from_millis(500));
    ExitCode::SUCCESS
  }

  /// "Ask, then fail": the question as the agent's last message, then a result that reports an API error
  /// for the turn, unrelated to any permission prompt.
  pub fn ask_then_fail(&self) -> ExitCode {
    let question = serde_json::json!({"outcome": "question", "summary": "Keep email sign-in?"}).to_string();
    self.assistant(
      "msg_ask",
      &serde_json::json!({"type": "text", "text": question}).to_string(),
    );
    let result = serde_json::json!({
      "type": "result", "subtype": "success", "is_error": true, "num_turns": 1,
      "result": "API Error: 500 Internal server error", "session_id": self.session,
      "usage": {"input_tokens": 0, "output_tokens": 0}, "permission_denials": [],
    });
    println!("{result}");
    flush();
    super::fake_claude_input::later_turns(&self.session);
    ExitCode::from(1)
  }

  /// A denied call: the agent reads what it was told and goes on without it.
  fn denied(&self, worktree: &Path, call: &str, told: &str) {
    eprintln!("told: {told}");
    self.tool_result(call, told, true);
    if append(worktree, "went on").is_err() {
      eprintln!("cannot write README.md");
    }
  }

  /// A second call of `first`'s tool, in the same session.
  fn ask_second(&self, worktree: &Path, prompt: &mut fake_permission_prompt::Prompt, first: &Ask) {
    let call = format!("{}_second", first.call);
    let mut input = first.input.clone();
    if let Some(command) = first.input.get("command").and_then(serde_json::Value::as_str) {
      let head = command.rsplit_once(' ').map_or(command, |(head, _)| head);
      input["command"] = serde_json::json!(format!("{head} tokio"));
    }
    let block = serde_json::json!({"type": "tool_use", "id": call, "name": first.tool, "input": input});
    self.assistant("msg_second", &block.to_string());
    let (decision, told) = prompt
      .ask(&first.tool, &input, &call)
      .unwrap_or_else(|| ("none".to_owned(), String::new()));
    eprintln!("permission second: {call} {decision}");
    if decision == "allow" {
      if append(worktree, "approved").is_err() {
        eprintln!("cannot write README.md");
      }
      self.tool_result(&call, "ok", false);
    } else {
      self.denied(worktree, &call, &told);
    }
  }

  /// A continued session: it takes up a call it was ended while waiting on, else appends
  /// `continued` once the test no longer holds it.
  pub fn resume(
    &self,
    worktree: &Path,
    args: &[String],
    step: &str,
    opened: Option<fake_permission_prompt::Prompt>,
  ) -> ExitCode {
    if let Some(interrupted) = self.interrupted_call(worktree) {
      return self.resume_interrupted(worktree, (args, opened), interrupted, step);
    }
    wait_while_held(worktree);
    self.finish(worktree, "continued", step)
  }

  /// The call the session was waiting on when the CLI was ended, if any.
  fn interrupted_call(&self, worktree: &Path) -> Option<Ask> {
    let pending = pending_call(worktree, &self.session);
    let kept: serde_json::Value = serde_json::from_slice(&fs::read(&pending).ok()?).ok()?;
    if fs::remove_file(&pending).is_err() {
      eprintln!("cannot forget the interrupted call");
    }
    Some(Ask {
      call: kept["call"].as_str()?.to_owned(),
      tool: kept["tool"].as_str()?.to_owned(),
      input: kept["input"].clone(),
    })
  }

  /// `--resume` of a session whose last call has no result: Claude Code 2.1.284 gives that call the
  /// result "[Tool call interrupted: the session ended before this call's result was recorded, so its
  /// outcome is unknown. …]" (`Z0` in its binary), so the model sees it neither granted nor refused.
  /// The fake then asks for the same action again, in a new call.
  fn resume_interrupted(
    &self,
    worktree: &Path,
    prompt: (&[String], Option<fake_permission_prompt::Prompt>),
    interrupted: Ask,
    step: &str,
  ) -> ExitCode {
    self.tool_result(&interrupted.call, INTERRUPTED, true);
    eprintln!("resumed: {} interrupted", interrupted.call);
    let again = Ask {
      call: format!("{}_again", interrupted.call),
      ..interrupted
    };
    self.ask_prompt(worktree, prompt, &again, step)
  }
}

/// What Claude Code 2.1.284 records for a call the ended session never answered.
const INTERRUPTED: &str = "[Tool call interrupted: the session ended before this call's result was recorded, so its \
                           outcome is unknown. Check whether it took effect before relying on it or running it again.]";

/// What Claude Code 2.1.286 answers a permission prompt call that sent nothing for 30 minutes.
const GAVE_UP: &str = "Error calling tool (Bash): MCP server Orchester tool approve sent no response or progress for                        1800s; aborting.";

/// Where the fake keeps the call its session `session` waits on, next to the worktrees.
fn pending_call(worktree: &Path, session: &str) -> std::path::PathBuf {
  worktree
    .parent()
    .and_then(Path::parent)
    .map_or_else(|| worktree.to_path_buf(), Path::to_path_buf)
    .join(format!("fake-pending-{session}.json"))
}

/// `fake-release` next to the worktrees, which a test creates to let the fake go on.
fn released(worktree: &Path) -> std::path::PathBuf {
  worktree.parent().and_then(Path::parent).map_or_else(
    || worktree.join("fake-release"),
    |orchester| orchester.join("fake-release"),
  )
}
