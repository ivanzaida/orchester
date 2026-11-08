//! What a requested pause waits for, in words (`docs/design/run-pause.md`): the header's *Pausing…* says it in its
//! tooltip and its menu row, the Summary's notice in its body. The steps come from `PauseWait::of_run`; none work
//! while the run only waits for a person, when the runner pauses it at once.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_workspace::tasks::pause_wait::PauseWait;

/// The name of the step a pause waits for.
fn step(wait: &PauseWait) -> &Arc<str> {
  match wait {
    PauseWait::Agent { step, .. } | PauseWait::Command { step, .. } => step,
  }
}

/// The names of the steps, one after the other.
fn steps(ctx: &Ctx, waits: &[PauseWait]) -> String {
  let names: Vec<&str> = waits.iter().map(|wait| &**step(wait)).collect();
  names.join(&ctx.t("task.listSeparator"))
}

/// What the header's *Pausing…* waits for: the agent that finishes its step, the command that ends, the steps that
/// end, or, while nothing works, the next checkpoint.
pub(super) fn hint(ctx: &Ctx, waits: &[PauseWait]) -> Arc<str> {
  match waits {
    [] => ctx.t("task.controls.pausingFor.none"),
    [PauseWait::Agent { step, agent }] => {
      let agent = agent.clone().unwrap_or_else(|| ctx.t("decisions.theAgent"));
      ctx.t_args(
        "task.controls.pausingFor.agent",
        [("agent", &*agent), ("step", &**step)],
      )
    }
    [PauseWait::Command { step, tool: Some(tool) }] => ctx.t_args(
      "task.controls.pausingFor.command",
      [("step", &**step), ("tool", &**tool)],
    ),
    [PauseWait::Command { step, tool: None }] => {
      ctx.t_args("task.controls.pausingFor.commandUnnamed", [("step", &**step)])
    }
    _ => ctx.t_args("task.controls.pausingFor.steps", [("steps", steps(ctx, waits))]),
  }
}

/// The Summary notice's body while the run pauses on `branch`: what the steps that still work do and that the run
/// pauses once they end. `None` while nothing works: the notice then says it as it did before.
pub(super) fn body(ctx: &Ctx, waits: &[PauseWait], branch: &str) -> Option<Arc<str>> {
  let said = match waits {
    [] => return None,
    [PauseWait::Agent { step, agent }] => {
      let agent = agent.clone().unwrap_or_else(|| ctx.t("summary.notice.theAgent"));
      ctx.t_args(
        "summary.notice.pausingBody",
        [("agent", &*agent), ("step", &**step), ("branch", branch)],
      )
    }
    [PauseWait::Command { step, tool: Some(tool) }] => ctx.t_args(
      "summary.notice.pausingBodyCommand",
      [("step", &**step), ("tool", &**tool), ("branch", branch)],
    ),
    [PauseWait::Command { step, tool: None }] => ctx.t_args(
      "summary.notice.pausingBodyCommandUnnamed",
      [("step", &**step), ("branch", branch)],
    ),
    _ => ctx.t_args(
      "summary.notice.pausingBodySteps",
      [("steps", steps(ctx, waits)), ("branch", branch.to_owned())],
    ),
  };
  Some(said)
}
