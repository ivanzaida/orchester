//! What a native tool call tells Activity before it ends (04.H10): its tool, what a file tool did, the
//! shell of a command, and who declined a command that did not run and what the agent was told.

use orchester_core::{
  connection::stream::{AgentEvent, CallDetail, Declined, DeclinedBy, NativeCall},
  native::{
    model::StopReason,
    session::{DENIED, LoopAction, LoopInput},
    tools::ToolName,
  },
  permission::RepositoryAccess,
  run::CommandShell,
};
use serde_json::json;

use crate::support::{Layout, call, ok, response, settings, start_with, tool_call};

fn native_calls(actions: &[LoopAction]) -> Vec<(String, NativeCall)> {
  actions
    .iter()
    .filter_map(|action| match action {
      LoopAction::Emit(events) => Some(events.clone()),
      _ => None,
    })
    .flatten()
    .filter_map(|event| match event {
      AgentEvent::NativeCall { item, call } => Some((item, call)),
      _ => None,
    })
    .collect()
}

#[test]
fn native_events_tell_what_a_call_did() {
  let layout = Layout::new();
  let mut with_shell = settings(RepositoryAccess::Write, 200_000);
  with_shell.shell = Some(CommandShell::Pwsh);
  let (mut native, _) = start_with(&layout, RepositoryAccess::Write, with_shell, &["git"]);
  let items = vec![
    tool_call("r1", "read_file", json!({"path": "src/main.rs"})),
    tool_call("c1", "run_command", json!({"command": "git status"})),
  ];
  native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  let read = CallDetail::Read {
    first: 1,
    last: 1,
    bytes: 13,
  };
  let mut outcome = ok("1\tfn main() {}");
  outcome.detail = Some(read.clone());
  let actions = native.handle(LoopInput::ToolDone(call("r1"), outcome)).expect("r1");
  let told = NativeCall {
    tool: ToolName::ReadFile,
    detail: Some(read),
    declined: None,
    input: None,
  };
  assert_eq!(native_calls(&actions), [("r1".to_owned(), told)]);
  let actions = native.handle(LoopInput::ToolDone(call("c1"), ok("clean"))).expect("c1");
  let told = NativeCall {
    tool: ToolName::RunCommand,
    detail: Some(CallDetail::Command { shell: None }),
    declined: None,
    input: None,
  };
  assert_eq!(native_calls(&actions), [("c1".to_owned(), told)]);
}

#[test]
fn native_events_tell_a_denial_from_a_refusal() {
  let layout = Layout::new();
  let mut with_shell = settings(RepositoryAccess::Write, 200_000);
  with_shell.shell = Some(CommandShell::Zsh);
  let (mut native, _) = start_with(&layout, RepositoryAccess::Write, with_shell, &["ls"]);
  let items = vec![
    tool_call("c1", "run_command", json!({"command": "ls | wc"})),
    tool_call("c2", "run_command", json!({"command": "rm -r build", "shell": true})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  let [(item, refused)] = &native_calls(&actions)[..] else {
    panic!("one refusal: {actions:?}")
  };
  assert_eq!(item, "c1");
  assert_eq!(refused.detail, Some(CallDetail::Command { shell: None }));
  assert!(
    matches!(&refused.declined, Some(Declined { by: DeclinedBy::Orchester, result }) if result.contains("shell: true"))
  );
  let actions = native.handle(LoopInput::Decided(call("c2"), false)).expect("denied");
  let denied = NativeCall {
    tool: ToolName::RunCommand,
    detail: Some(CallDetail::Command {
      shell: Some(CommandShell::Zsh),
    }),
    declined: Some(Declined {
      by: DeclinedBy::Person,
      result: DENIED.to_owned(),
    }),
    input: None,
  };
  assert_eq!(native_calls(&actions), [("c2".to_owned(), denied)]);
}
