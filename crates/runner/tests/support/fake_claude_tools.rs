//! How `orchester-fake-claude` uses Orchester's tools ("Use Orchester's tools", "Use the file tools"), as Claude Code
//! does: they are tools of the permission prompt's server, a call of one runs at once when an allow rule names it,
//! and a call of one without a rule asks the permission prompt first and runs only when allowed. The fake lists the
//! tools (`orchester-tools: <names>`), then makes the calls of [`fake_tool_calls`], reporting each decision
//! (`orchester-permission: <tool> <decision>`) and answer (`orchester-call: <tool> <ok|error> <bytes>: <first line> |
//! <last line>`). With "Use Orchester's tools without asking" it runs the calls that should ask without asking
//! (`orchester-permission: <tool> skipped`), as a CLI that ignored its rules would.

use std::{path::Path, process::ExitCode};

use serde_json::{Value, json};

use super::{Fake, fake_claude_rules, fake_permission_prompt::Prompt, fake_skill_calls, fake_tool_calls};

#[path = "fake_app_tools.rs"]
mod fake_app_tools;

#[path = "fake_browser_tools.rs"]
mod fake_browser_tools;

#[path = "fake_index_prompt.rs"]
mod fake_index_prompt;

#[path = "fake_process_tools.rs"]
mod fake_process_tools;

impl Fake {
  /// "Use Orchester's tools": the calls outside the worktree ([`fake_tool_calls::external`]).
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
    for (call, tool, input) in fake_tool_calls::external(worktree) {
      self.use_tool(args, &mut server, (call, tool, &input));
    }
    self.finish(worktree, "hello", step)
  }

  /// "Use the file tools": Orchester's file tools in the worktree of a writing step ([`fake_tool_calls::files`]), after
  /// writing a file larger than an answer holds.
  pub fn use_file_tools(&self, worktree: &Path, (args, opened): (&[String], Option<Prompt>), step: &str) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
    let wide = format!("{}\n", "y".repeat(1000)).repeat(300);
    if let Err(error) = std::fs::write(worktree.join("wide.txt"), wide) {
      eprintln!("cannot write wide.txt: {error}");
      return ExitCode::from(2);
    }
    for (call, tool, input) in fake_tool_calls::files(false) {
      self.use_tool(args, &mut server, (call, tool, &input));
    }
    self.finish(worktree, "hello", step)
  }

  /// "Use the process tools", or "Use the browser tools" ([`Self::use_browser_tools`]).
  pub fn use_process_tools(
    &self,
    worktree: &Path,
    (args, opened): (&[String], Option<Prompt>),
    step: &str,
  ) -> ExitCode {
    if self.prompt.contains("Use the browser tools") {
      return self.use_browser_tools(worktree, (args, opened), step);
    }
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      return ExitCode::from(2);
    };
    if self.prompt.contains("Call the run app") {
      fake_app_tools::exercise(worktree, true, |tool, input| {
        let name = format!("mcp__Orchester__{tool}");
        assert!(fake_claude_rules::mcp(args, &name).runs(), "exact allow rule");
        eprintln!("app-permission: allow-by-rule");
        let result = server.call_result(tool, input).expect("relay");
        // Claude Code prints a tool result's text blocks, never an image's data.
        let mut printed = result.clone();
        for block in printed["content"].as_array_mut().into_iter().flatten() {
          if block["type"] == "image" {
            block["data"] = json!("");
          }
        }
        self.tool_result(
          "app-call",
          &printed.to_string(),
          result["isError"].as_bool().unwrap_or(false),
        );
        result
      });
      return self.finish(worktree, "hello", step);
    }
    let mut index = 0;
    fake_process_tools::exercise(worktree, |tool, input| {
      index += 1;
      self.use_tool(args, &mut server, (&format!("toolu_process_{index}"), tool, input));
    });
    self.finish(worktree, "hello", step)
  }

  /// "Use the browser tools": the calls of [`fake_browser_tools`], each reporting whether an allow rule names it
  /// (`browser-permission: <tool> <allow-by-rule|ask>`), as the runner asks about a browser call itself.
  pub fn use_browser_tools(
    &self,
    worktree: &Path,
    (args, opened): (&[String], Option<Prompt>),
    step: &str,
  ) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
    fake_browser_tools::exercise(worktree, &mut |tool, input| {
      let rule = fake_claude_rules::mcp(args, &format!("mcp__Orchester__{tool}"));
      eprintln!("browser-permission: {tool} {}", rule.as_str());
      server.call_result(tool, input)
    });
    self.finish(worktree, "hello", step)
  }

  /// "Use the run tools": the calls of [`fake_tool_calls::run_tools`].
  pub fn use_run_tools(&self, worktree: &Path, (args, opened): (&[String], Option<Prompt>), step: &str) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
    for (call, tool, input) in fake_tool_calls::run_tools() {
      self.use_tool(args, &mut server, (call, tool, &input));
    }
    self.finish(worktree, "hello", step)
  }

  /// "Ask with options": lists the tools, then calls `ask_question` with three options (several allowed when the
  /// prompt says "several allowed"; only once `fake-release` exists next to the worktrees when it says "after
  /// release") and reports the answer it gets back (`orchester-answer: <ok|error> <text, lines joined by ' | '>`),
  /// as Claude Code would read it as the call's result; then finishes as usual.
  pub fn ask_with_options(&self, worktree: &Path, (args, opened): (&[String], Option<Prompt>), step: &str) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
    if self.prompt.contains("after release") {
      let release = worktree
        .ancestors()
        .nth(2)
        .map(|orchester| orchester.join("fake-release"));
      while !release.as_ref().is_some_and(|release| release.exists()) {
        std::thread::sleep(std::time::Duration::from_millis(50));
      }
    }
    let several = self.prompt.contains("several allowed");
    let input = json!({"question": "Where should the sign-in button go?", "header": "Docs layout",
    "multi_select": several, "options": [
      {"label": "Top of the page", "description": "Before the first step"},
      {"label": "Under Sign in", "description": null},
      {"label": "In the sidebar", "description": "Always visible"}
    ]});
    let block = json!({"type": "tool_use", "id": "toolu_ask", "name": "mcp__Orchester__ask_question", "input": input});
    self.assistant("msg_toolu_ask", &block.to_string());
    let Some((text, failed)) = server.call("ask_question", &input) else {
      eprintln!("orchester-answer: none");
      return ExitCode::from(2);
    };
    let status = if failed { "error" } else { "ok" };
    eprintln!(
      "orchester-answer: {status} {}",
      text.lines().collect::<Vec<_>>().join(" | ")
    );
    self.tool_result("toolu_ask", &text, failed);
    self.finish(worktree, "hello", step)
  }

  /// "Use the skill tools": the calls of [`fake_skill_calls`], each decision and answer reported.
  pub fn use_skill_tools(&self, worktree: &Path, (args, opened): (&[String], Option<Prompt>), step: &str) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    eprintln!("orchester-tools: {}", server.list().unwrap_or_default().join(","));
    for (call, tool, input) in fake_skill_calls::calls(worktree) {
      self.use_tool(args, &mut server, (call, tool, &input));
    }
    self.finish(worktree, "hello", step)
  }

  /// "Search the code index" (ORC-90): `find_symbol` through Orchester's tools, then a `Task` sub-agent that reads
  /// the index's `map.md` with Claude Code's `Read`, its lines marked with the `Task` call as Claude Code marks them;
  /// then finishes as usual.
  pub fn search_index(&self, worktree: &Path, (args, opened): (&[String], Option<Prompt>), step: &str) -> ExitCode {
    let Some(mut server) = opened.or_else(|| Prompt::open(args)) else {
      eprintln!("no permission prompt");
      return ExitCode::from(2);
    };
    self.use_tool(
      args,
      &mut server,
      ("toolu_find", "find_symbol", &json!({"name": "parse_config"})),
    );
    let Some(folder) = fake_index_prompt::index_folder(&self.prompt) else {
      eprintln!("index-call: the prompt names no code index");
      return ExitCode::from(2);
    };
    let task = json!({"type": "tool_use", "id": "toolu_task", "name": "Task",
      "input": {"description": "Explore", "prompt": "Read the index map"}});
    self.assistant("msg_task", &task.to_string());
    let map = Path::new(&folder).join("map.md");
    let read = json!({"type": "tool_use", "id": "toolu_sub_read", "name": "Read",
      "input": {"file_path": map.to_string_lossy()}});
    println!(
      "{}",
      json!({"type": "assistant", "parent_tool_use_id": "toolu_task", "session_id": self.session,
        "message": {"id": "msg_sub_read", "role": "assistant", "type": "message", "content": [read],
          "usage": {"input_tokens": 10, "output_tokens": 1}}})
    );
    println!(
      "{}",
      json!({"type": "user", "parent_tool_use_id": "toolu_task", "session_id": self.session,
        "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "toolu_sub_read",
          "content": "# map", "is_error": false}]}})
    );
    eprintln!("index-command: toolu_sub_read");
    self.tool_result("toolu_task", "the map lists the packages", false);
    self.finish(worktree, "hello", step)
  }

  /// One call of Orchester's `tool`, asked about first when no rule allows it.
  fn use_tool(&self, args: &[String], server: &mut Prompt, (call, tool, input): (&str, &str, &Value)) {
    let name = format!("mcp__Orchester__{tool}");
    let block = json!({"type": "tool_use", "id": call, "name": name, "input": input});
    self.assistant(&format!("msg_{call}"), &block.to_string());
    let rule = fake_claude_rules::mcp(args, &name);
    // A CLI that runs a call it should have asked about: the runner must ask itself.
    // The whole trigger: Orchester's own prompt says its run-process tools run "without asking".
    let unasked =
      self.prompt.contains("Use Orchester's tools without asking") && rule == fake_claude_rules::Decision::Ask;
    let decision = if unasked {
      "skipped".to_owned()
    } else if rule == fake_claude_rules::Decision::Ask {
      server
        .ask(&name, input, call)
        .map_or_else(|| "none".to_owned(), |(decision, _)| decision)
    } else {
      rule.as_str().to_owned()
    };
    eprintln!("orchester-permission: {tool} {decision}");
    if !(rule.runs() || decision == "allow" || unasked) {
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
