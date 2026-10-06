//! "Find a stopped background task": `orchester-fake-claude` as Claude Code resumed run 22's session (2026-10-06),
//! whose background command the previous process left. Before it takes the prompt it reports that command stopped,
//! starts and ends a turn of its own with an empty result, then starts again; once it took the prompt it writes
//! prose and gives the step result `{"outcome":"done","summary":"Finished <step>"}` through its `StructuredOutput`
//! tool, as the CLI does when given `--json-schema`.

use std::process::ExitCode;

use super::{Fake, NO_USAGE, USAGE, fake_claude_input, flush};

pub const PROMPT: &str = "Find a stopped background task";

/// What the CLI prints before its first `init`: the stopped command and the turn that ends it.
pub fn stopped_task(session: &str) {
  println!(
    "{}",
    serde_json::json!({"type": "system", "subtype": "task_notification", "task_id": "bo3qfn01g",
      "tool_use_id": "toolu_background", "status": "stopped", "output_file": "",
      "summary": "Background shell command didn't finish before the previous session ended", "session_id": session})
  );
  println!(
    r#"{{"type":"system","subtype":"init","cwd":"worktree","session_id":"{session}","tools":[],"mcp_servers":[],"model":"claude-haiku-4-5","permissionMode":"acceptEdits","apiKeySource":"none","claude_code_version":"2.1.286"}}"#
  );
  println!(
    "{}",
    serde_json::json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 0, "result": "",
      "session_id": session, "usage": serde_json::from_str::<serde_json::Value>(NO_USAGE).expect("usage is JSON"),
      "permission_denials": []})
  );
}

impl Fake {
  /// Prose, then the step result through `StructuredOutput`, in one assistant message.
  pub fn structured_output(&self, step: &str) -> ExitCode {
    let prose = "Design verification passed.";
    self.assistant(
      "msg_verify",
      &serde_json::json!({"type": "text", "text": prose}).to_string(),
    );
    let result = serde_json::json!({"outcome": "done", "summary": format!("Finished {step}")});
    let call =
      serde_json::json!({"type": "tool_use", "id": "toolu_result", "name": "StructuredOutput", "input": result});
    self.message("msg_verify", &call.to_string(), USAGE);
    self.tool_result("toolu_result", "Structured output provided successfully", false);
    println!(
      "{}",
      serde_json::json!({"type": "result", "subtype": "success", "is_error": false, "num_turns": 2, "result": prose,
        "structured_output": result, "session_id": self.session,
        "usage": {"input_tokens": 0, "output_tokens": 0}, "permission_denials": []})
    );
    flush();
    fake_claude_input::later_turns(&self.session);
    ExitCode::SUCCESS
  }
}
