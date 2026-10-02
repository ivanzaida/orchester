//! How `orchester-fake-claude` asks its permission prompt about a tool call (`fake_permission_prompt`), and takes up
//! a call its session was ended while waiting on.

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
    // As the real CLI's transcript would, the session keeps the call without a result while it
    // waits: a runner that ends the CLI now leaves it unanswered for `--resume`.
    let pending = pending_call(worktree, &self.session);
    let kept = serde_json::json!({"call": ask.call, "tool": ask.tool, "input": ask.input});
    if let Err(error) = fs::write(&pending, kept.to_string()) {
      eprintln!("cannot keep the pending call: {error}");
    }
    let decision = prompt
      .ask(&ask.tool, &ask.input, &ask.call)
      .unwrap_or_else(|| "none".to_owned());
    if fs::remove_file(&pending).is_err() {
      eprintln!("the pending call was not kept");
    }
    eprintln!("permission: {} {decision}", &ask.call);
    if decision == "allow" {
      if let Err(error) = append(worktree, "approved") {
        eprintln!("cannot write README.md: {error}");
        return ExitCode::from(2);
      }
      self.tool_result(&ask.call, "ok", false);
      let again = prompt
        .ask(&ask.tool, &ask.input, &ask.call)
        .unwrap_or_else(|| "none".to_owned());
      eprintln!("permission again: {} {again}", &ask.call);
      if knocks {
        eprintln!("second: {}", fake_permission_prompt::knock(args, None));
      }
    } else {
      self.tool_result(&ask.call, "Denied in Orchester.", true);
    }
    flush();
    self.finish(worktree, "hello", step)
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
