//! The git tools in the chat: they work in the chat's own project only; a write reads the person's switch *Let the
//! Orchestrator drive* when it is called, and is refused in a day's report's turn (`report_turn`) and, while the
//! chat does not drive, in a turn nobody asked for (`person_only`); a proposal of what can lose work is a card in any
//! turn. Every write and
//! proposal is kept in the conversation as an action with what ran and what git said, so the person sees it; a read
//! is not.

use orchester_core::orchestrator::{
  ActionOutcome, ChatAction,
  change::{Change, ProposedChange},
  git::GitChange,
};
use rmcp::model::JsonObject;
use serde_json::Value;

use super::{Called, Kind, Timeouts, call as call_git, command::refused};
use crate::{
  app_mcp::{Services, ToolError, Translate, blocking},
  messages::Message,
  orchestrator::{acts::drives, change, guard, keep, own_project::in_own_project, tools::ChatScope},
  runner::Runners,
};

/// Calls git tool `name`, a `kind` of tool, with `arguments` in `scope`; a write or proposal is kept in the
/// conversation, and `runners` tell the chat it changed.
pub(in crate::orchestrator) async fn call(
  (services, translate, runners): (&Services, &Translate, &Runners),
  scope: &ChatScope,
  (name, kind): (&str, Kind),
  mut arguments: JsonObject,
) -> Result<Value, ToolError> {
  in_own_project(scope, &mut arguments)?;
  if kind == Kind::Read {
    return match run(services, scope, (name, arguments), false).await? {
      Called::Read(read) => Ok(read),
      Called::Wrote { result, .. } => result,
      Called::Proposed(_) => Err(ToolError::InvalidArguments(format!("{name} only reads"))),
    };
  }
  let called = async {
    let writes = drives(services, scope).await?;
    asked_in_turn(scope, kind, writes).await?;
    run(services, scope, (name, arguments), writes).await
  }
  .await;
  let kept = |change: GitChange| ProposedChange {
    subject: scope.name.clone(),
    change: Change::Git(Box::new(change)),
  };
  let (result, change, outcome) = match called {
    Ok(Called::Wrote { result, change }) => {
      let outcome = outcome_of(&result, translate);
      (result, Some(kept(change)), outcome)
    }
    Ok(Called::Proposed(change)) => {
      let proposed = kept(change);
      (
        Ok(change::told(translate, &proposed)),
        Some(proposed),
        ActionOutcome::Waits,
      )
    }
    Ok(Called::Read(read)) => (Ok(read), None, ActionOutcome::Done),
    Err(error) => {
      let outcome = keep::refusal(&error, translate);
      (Err(error), None, outcome)
    }
  };
  let action = ChatAction {
    change: change.map(Box::new),
    ..ChatAction::new(name, None, outcome)
  };
  keep::keep(scope, (None, None), action).await;
  runners.chat_changed(&scope.root);
  result
}

/// Runs git tool `name` with `arguments` in `scope`'s project, `writes` whether the chat drives: in a run's worktree
/// through the project's runner.
async fn run(
  services: &Services,
  scope: &ChatScope,
  (name, arguments): (&str, JsonObject),
  writes: bool,
) -> Result<Called, ToolError> {
  let (programs, root, name) = (services.programs().clone(), scope.root.clone(), name.to_owned());
  let runs = services.run_git(root.clone());
  blocking(move || {
    call_git(
      (&programs, &runs),
      &root,
      (&name, &arguments),
      (writes, Timeouts::STANDARD),
    )
  })
  .await
}

/// Refused for a write in a turn nobody asked for: a day's report's turn changes nothing (`report_turn`), and a turn
/// a wait or a run's end started writes nothing (`person_only`) unless the chat `drives`. A proposal is a card the
/// person applies in any turn.
async fn asked_in_turn(scope: &ChatScope, kind: Kind, drives: bool) -> Result<(), ToolError> {
  if !scope.on_its_own || kind == Kind::Propose {
    return Ok(());
  }
  if guard::in_report_turn(scope).await? {
    return Err(refused(
      "report_turn",
      Message::new("orchestrator.tools.reportChangesNothing"),
    ));
  }
  if drives {
    return Ok(());
  }
  Err(refused("person_only", Message::new("orchestrator.git.onItsOwn")))
}

/// How a write went, as its action keeps it.
fn outcome_of(result: &Result<Value, ToolError>, translate: &Translate) -> ActionOutcome {
  match result {
    Ok(_) => ActionOutcome::Done,
    Err(error) => keep::refusal(error, translate),
  }
}
