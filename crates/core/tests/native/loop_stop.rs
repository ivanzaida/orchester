//! The native loop's stop modes: now, after the turn, after the running command, and at an approval.

use orchester_core::{
  native::{
    item::Item,
    model::StopReason,
    session::{LoopAction, LoopEnd, LoopInput, NativeLoop, StopMode},
  },
  permission::RepositoryAccess,
};
use serde_json::json;

use crate::support::{Layout, call, ended, ok, persisted, request, response, run_ids, start, tool_call};

/// A loop whose model answered with a read, then a listed command, then a command that asks.
fn three_calls(layout: &Layout) -> (NativeLoop, Vec<LoopAction>) {
  let (mut native, _) = start(layout, RepositoryAccess::Write, &["cargo build"]);
  let items = vec![
    tool_call("r1", "read_file", json!({"path": "src/main.rs"})),
    tool_call("c1", "run_command", json!({"command": "cargo build"})),
    tool_call("c2", "run_command", json!({"command": "npm install"})),
  ];
  let actions = native
    .handle(LoopInput::Responded(response(items, StopReason::ToolUse, 10)))
    .expect("response");
  (native, actions)
}

fn stop(native: &mut NativeLoop, mode: StopMode) -> Vec<LoopAction> {
  native.handle(LoopInput::Stop(mode)).expect("stop")
}

#[test]
fn native_stop_now() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  assert_eq!(
    stop(&mut native, StopMode::Now),
    vec![LoopAction::End(LoopEnd::Stopped(StopMode::Now))]
  );
  assert!(native.has_ended());
  assert!(
    stop(&mut native, StopMode::Now).is_empty(),
    "an ended loop ignores stops"
  );

  let (mut native, _) = three_calls(&layout);
  assert_eq!(
    ended(&stop(&mut native, StopMode::Now)),
    Some(&LoopEnd::Stopped(StopMode::Now))
  );
}

#[test]
fn native_stop_after_turn_while_the_model_writes() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  assert!(stop(&mut native, StopMode::AfterTurn).is_empty());
  let read = tool_call("r1", "read_file", json!({"path": "src/main.rs"}));
  let actions = native
    .handle(LoopInput::Responded(response(
      vec![read.clone()],
      StopReason::ToolUse,
      10,
    )))
    .expect("response");
  assert_eq!(persisted(&actions), vec![read], "the response is kept");
  assert!(run_ids(&actions).is_empty(), "its tool calls do not run");
  assert_eq!(ended(&actions), Some(&LoopEnd::Stopped(StopMode::AfterTurn)));
}

#[test]
fn native_stop_after_turn_while_tools_run() {
  let layout = Layout::new();
  let (mut native, actions) = three_calls(&layout);
  assert_eq!(run_ids(&actions), ["r1"]);
  assert!(stop(&mut native, StopMode::AfterTurn).is_empty());
  let actions = native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  assert!(run_ids(&actions).is_empty(), "the remaining calls do not run");
  assert_eq!(ended(&actions), Some(&LoopEnd::Stopped(StopMode::AfterTurn)));
}

#[test]
fn native_stop_after_command() {
  let layout = Layout::new();
  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  assert_eq!(
    ended(&stop(&mut native, StopMode::AfterCommand)),
    Some(&LoopEnd::Stopped(StopMode::AfterCommand)),
    "nothing runs, so it ends at once"
  );

  let (mut native, _) = three_calls(&layout);
  native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  assert!(stop(&mut native, StopMode::AfterCommand).is_empty(), "c1 runs");
  let actions = native.handle(LoopInput::ToolDone(call("c1"), ok("built"))).expect("c1");
  assert_eq!(persisted(&actions), vec![Item::tool_result(call("c1"), "built", false)]);
  assert_eq!(ended(&actions), Some(&LoopEnd::Stopped(StopMode::AfterCommand)));
}

#[test]
fn native_stop_at_approval() {
  let layout = Layout::new();
  let (mut native, _) = three_calls(&layout);
  assert!(stop(&mut native, StopMode::AtApproval).is_empty());
  let actions = native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  assert_eq!(run_ids(&actions), ["c1"], "ordinary work goes on");
  let actions = native.handle(LoopInput::ToolDone(call("c1"), ok("built"))).expect("c1");
  assert_eq!(
    ended(&actions),
    Some(&LoopEnd::Stopped(StopMode::AtApproval)),
    "instead of asking about c2"
  );

  let (mut native, _) = three_calls(&layout);
  native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  let actions = native.handle(LoopInput::ToolDone(call("c1"), ok("built"))).expect("c1");
  assert!(matches!(actions.last(), Some(LoopAction::Ask(_))));
  assert_eq!(
    ended(&stop(&mut native, StopMode::AtApproval)),
    Some(&LoopEnd::Stopped(StopMode::AtApproval)),
    "a waiting approval ends at once"
  );
}

#[test]
fn native_stop_withdrawn() {
  let layout = Layout::new();
  let (mut native, _) = three_calls(&layout);
  stop(&mut native, StopMode::AtApproval);
  assert!(native.handle(LoopInput::Withdraw).expect("withdraw").is_empty());
  native.handle(LoopInput::ToolDone(call("r1"), ok("text"))).expect("r1");
  let actions = native.handle(LoopInput::ToolDone(call("c1"), ok("built"))).expect("c1");
  assert!(matches!(actions.last(), Some(LoopAction::Ask(ask)) if ask.call == call("c2")));

  let (mut native, _) = start(&layout, RepositoryAccess::Write, &[]);
  stop(&mut native, StopMode::AfterTurn);
  native.handle(LoopInput::Withdraw).expect("withdraw");
  let actions = native
    .handle(LoopInput::Responded(response(Vec::new(), StopReason::EndTurn, 10)))
    .expect("response");
  assert_eq!(
    ended(&actions),
    Some(&LoopEnd::Stopped(StopMode::AfterTurn)),
    "a budget stop is not withdrawn"
  );
  assert!(request(&actions).is_none());
}
