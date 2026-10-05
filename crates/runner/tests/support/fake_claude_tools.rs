//! How `orchester-fake-claude` uses Orchester's tools ("Use Orchester's tools"), as Claude Code does: they are
//! tools of the permission prompt's server, a call of one runs at once when an allow rule names it, and a
//! call of one without a rule asks the permission prompt first and runs only when allowed. The fake lists
//! the tools (`orchester-tools: <names>`), then stages and commits a file larger than a git answer holds,
//! shows it, reads a file outside the worktree and tries to stage one there, reporting each decision
//! (`orchester-permission: <tool> <decision>`) and answer (`orchester-call: <tool> <ok|error> <bytes>: <first
//! line> | <last line>`).

use std::{path::Path, process::ExitCode};

use serde_json::{Value, json};

use super::{Fake, fake_claude_rules, fake_permission_prompt::Prompt};

/// The size of the file the fake commits, more than a git answer holds (128 KiB).
const BIG: usize = 200 * 1024;
/// The file outside the worktree the fake reads: next to the repository's `.orchester` folder.
pub const OUTSIDE: &str = "outside-notes.txt";

impl Fake {
  pub fn use_orchester_tools(
    &self,
    worktree: &Path,
    (args, opened): (&[String], Option<Prompt>),
    step: &str,
  ) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    let listed = server.list().unwrap_or_default();
    eprintln!("orchester-tools: {}", listed.join(","));
    if let Err(error) = std::fs::write(worktree.join("big.txt"), "x".repeat(BIG)) {
      eprintln!("cannot write big.txt: {error}");
      return ExitCode::from(2);
    }
    let outside = worktree
      .ancestors()
      .nth(3)
      .map(|repository| repository.join(OUTSIDE).to_string_lossy().into_owned());
    let calls = [
      ("toolu_status", "git_status", json!({})),
      ("toolu_add", "git_add", json!({"paths": ["big.txt"]})),
      (
        "toolu_commit",
        "git_commit",
        json!({"message": "Add big.txt; rm -rf / $(whoami)"}),
      ),
      ("toolu_show", "git_show", json!({"revision": "HEAD", "path": "big.txt"})),
      ("toolu_outside", "read_outside_file", json!({ "path": outside })),
      (
        "toolu_add_outside",
        "git_add",
        json!({"paths": ["../../../outside-notes.txt"]}),
      ),
    ];
    for (call, tool, input) in calls {
      self.use_tool(args, &mut server, (call, tool, &input));
    }
    self.finish(worktree, "hello", step)
  }

  /// One call of Orchester's `tool`, asked about first when no rule allows it.
  fn use_tool(&self, args: &[String], server: &mut Prompt, (call, tool, input): (&str, &str, &Value)) {
    let name = format!("mcp__Orchester__{tool}");
    let block = json!({"type": "tool_use", "id": call, "name": name, "input": input});
    self.assistant(&format!("msg_{call}"), &block.to_string());
    let rule = fake_claude_rules::mcp(args, &name);
    let decision = if rule == fake_claude_rules::Decision::Ask {
      server
        .ask(&name, input, call)
        .map_or_else(|| "none".to_owned(), |(decision, _)| decision)
    } else {
      rule.as_str().to_owned()
    };
    eprintln!("orchester-permission: {tool} {decision}");
    if !(rule.runs() || decision == "allow") {
      self.tool_result(call, "Denied.", true);
      return;
    }
    let Some((text, failed)) = server.call(tool, input) else {
      eprintln!("orchester-call: {tool} no answer");
      self.tool_result(call, "No answer.", true);
      return;
    };
    let first = text.lines().next().unwrap_or_default();
    let last = text.lines().last().unwrap_or_default();
    let status = if failed { "error" } else { "ok" };
    eprintln!("orchester-call: {tool} {status} {}: {first} | {last}", text.len());
    self.tool_result(call, status, failed);
  }
}
