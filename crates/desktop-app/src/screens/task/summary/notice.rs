//! Where the task stands and what that means, at the top of the Summary (`Notice · Implement is running
//! · attempt 2` `uqbUH` in `lUXff`, stopped at the token limit in `oTzJZ`, waits after retries in
//! `DVuXu`, ready for review in `F2LtFn`): one notice per phase of the task's latest run, with Open
//! conversation where the conversation is where the person looks or answers.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::{
  id::StepExecutionId,
  run::{PauseReason, RunLimit, RunStatus, Stuck},
  task::TaskStatus,
  tokens::compact_tokens,
};
use orchester_workspace::{
  display,
  runner::state::RunnerState,
  tasks::{
    approvals::ShownApproval,
    detail::{RunView, StepView, TaskDetail},
  },
};

use crate::ui::{
  icon::Icon,
  notice::{Notice, NoticeTone},
};

/// What the notice says, before its action.
pub struct Said {
  pub tone: NoticeTone,
  pub icon: Icon,
  pub title: Arc<str>,
  pub body: Arc<str>,
  /// The conversation is where to look or answer.
  pub conversation: bool,
}

impl Said {
  fn new(tone: NoticeTone, icon: Icon, (title, body): (Arc<str>, Arc<str>)) -> Self {
    Self {
      tone,
      icon,
      title,
      body,
      conversation: false,
    }
  }

  fn in_conversation(mut self) -> Self {
    self.conversation = true;
    self
  }

  pub fn notice(self) -> Notice {
    Notice::new("task-notice", self.tone, self.title)
      .icon(self.icon)
      .body(self.body)
  }
}

/// The notice of `detail`. `runner`: the project's runner, while a run recorded as running lost it;
/// `approval`: a tool request of the run that waits for the person.
pub fn said(ctx: &Ctx, detail: &TaskDetail, runner: Option<&RunnerState>, approval: Option<&ShownApproval>) -> Said {
  let Some(run) = &detail.run else {
    return without_run(ctx, detail);
  };
  if let Some(runner) = runner {
    return lost(ctx, run, runner);
  }
  if let Some(approval) = approval {
    return waits_for_approval(ctx, run, approval);
  }
  match run.run.status() {
    RunStatus::Running => running(ctx, run),
    RunStatus::Paused(PauseReason::Requested) => paused(ctx, run),
    RunStatus::Paused(PauseReason::LimitReached(limit)) => stopped(ctx, run, *limit),
    RunStatus::Interrupted(_) => interrupted(ctx, run),
    _ => settled(ctx, detail, run),
  }
}

/// A draft, or a queued task whose run did not start.
fn without_run(ctx: &Ctx, detail: &TaskDetail) -> Said {
  if detail.task.status() == TaskStatus::Draft {
    // Guidance only while something is missing; a draft with every detail is ready to run.
    let text = if detail.task.details().missing().is_empty() {
      (ctx.t("summary.notice.readyTitle"), ctx.t("summary.notice.readyBody"))
    } else {
      (ctx.t("summary.notice.draftTitle"), ctx.t("summary.notice.draftBody"))
    };
    return Said::new(NoticeTone::Info, Icon::CircleDashed, text);
  }
  let text = (ctx.t("summary.notice.queuedTitle"), ctx.t("summary.notice.queuedBody"));
  Said::new(NoticeTone::Info, Icon::Clock, text)
}

/// The words of a step: its name and its agent, "the agent" for a step without one.
fn step_words(ctx: &Ctx, step: Option<&StepView>) -> (String, String) {
  let name = step.map(|step| step.name.to_string()).unwrap_or_default();
  let agent = step
    .and_then(|step| step.agent.as_deref())
    .map_or_else(|| ctx.t("summary.notice.theAgent").to_string(), str::to_owned);
  (name, agent)
}

fn running(ctx: &Ctx, run: &RunView) -> Said {
  let (step, agent) = step_words(ctx, run.current_step());
  let branch = run.run.branch().as_str();
  let args = [("step", step.as_str()), ("agent", agent.as_str()), ("branch", branch)];
  if run.run.pause_requested() {
    let text = (
      ctx.t("summary.notice.pausingTitle"),
      ctx.t_args("summary.notice.pausingBody", args),
    );
    return Said::new(NoticeTone::Info, Icon::CirclePause, text);
  }
  if let Some(said) = waiting(ctx, run) {
    return said;
  }
  let attempt = run
    .run
    .current_attempt()
    .map(|attempt| attempt.number().get().to_string())
    .unwrap_or_default();
  let working: Vec<&str> = run.working_steps().map(|step| &*step.name).collect();
  let title = if working.len() > 1 {
    let steps = working.join(&ctx.t("task.listSeparator"));
    ctx.t_args("summary.notice.runningTogetherTitle", [("steps", steps.as_str())])
  } else {
    ctx.t_args(
      "summary.notice.runningTitle",
      [("step", step.as_str()), ("attempt", attempt.as_str())],
    )
  };
  let body = ctx.t_args("summary.notice.runningBody", args);
  Said::new(NoticeTone::Info, Icon::LoaderCircle, (title, body)).in_conversation()
}

/// A step that waits for the person's answer to a question or for their decision on its plan.
fn waiting(ctx: &Ctx, run: &RunView) -> Option<Said> {
  let asked: Option<StepExecutionId> = run
    .run
    .awaiting_answer()
    .first()
    .copied()
    .or_else(|| run.waiting_questions.first().map(|asked| asked.request.execution()));
  if let Some(execution) = asked {
    let (step, agent) = step_words(ctx, run.step_of(execution));
    let text = (
      ctx.t_args("summary.notice.askingTitle", [("agent", agent.as_str())]),
      ctx.t_args("summary.notice.askingBody", [("step", step.as_str())]),
    );
    return Some(Said::new(NoticeTone::Warning, Icon::Hand, text).in_conversation());
  }
  let planned = run.run.awaiting_plan().first().copied()?;
  let (step, agent) = step_words(ctx, run.step_of(planned));
  let version = run
    .plans
    .iter()
    .filter(|plan| plan.execution == planned)
    .map(|plan| plan.version.get())
    .max()
    .unwrap_or(1);
  // What approving starts: the step after the planned one in the pipeline, as the design names it.
  let number = run.step_of(planned).map_or(0, |step| step.number);
  let next = run.steps.iter().find(|step| step.number == number + 1);
  let body = match next {
    Some(next) => {
      let (next_name, next_agent) = step_words(ctx, Some(next));
      ctx.t_args(
        "summary.notice.planBodyNext",
        [
          ("agent", agent.as_str()),
          ("step", step.as_str()),
          ("next", format!("{:02} {next_name}", next.number).as_str()),
          ("nextAgent", next_agent.as_str()),
        ],
      )
    }
    None => ctx.t_args(
      "summary.notice.planBody",
      [("agent", agent.as_str()), ("step", step.as_str())],
    ),
  };
  let text = (
    ctx.t_args("summary.notice.planTitle", [("version", version.to_string())]),
    body,
  );
  Some(Said::new(NoticeTone::Warning, Icon::Hand, text).in_conversation())
}

fn waits_for_approval(ctx: &Ctx, run: &RunView, approval: &ShownApproval) -> Said {
  let (step, _) = step_words(ctx, run.step_of(approval.approval.execution));
  let time = display::clock(approval.approval.requested_at);
  let text = (
    ctx.t_args(
      "summary.notice.approvalTitle",
      [("agent", approval.approval.agent.as_str())],
    ),
    ctx.t_args(
      "summary.notice.approvalBody",
      [("step", step.as_str()), ("time", time.as_str())],
    ),
  );
  Said::new(NoticeTone::Warning, Icon::Hand, text).in_conversation()
}

/// The short revision of the run's last checkpoint.
fn checkpoint(run: &RunView) -> Option<String> {
  run
    .run
    .last_checkpoint()
    .map(|checkpoint| display::short_revision(checkpoint.revision().as_str()).to_owned())
}

fn paused(ctx: &Ctx, run: &RunView) -> Said {
  let (step, _) = step_words(ctx, run.stopped_step().or_else(|| run.current_step()));
  let title = match checkpoint(run) {
    Some(checkpoint) => ctx.t_args("summary.notice.pausedTitle", [("checkpoint", checkpoint.as_str())]),
    None => ctx.t("summary.notice.pausedNoCheckpointTitle"),
  };
  let body = ctx.t_args("summary.notice.pausedBody", [("step", step.as_str())]);
  Said::new(NoticeTone::Info, Icon::CirclePause, (title, body))
}

/// Stopped at a limit: the run's token or time limit, a step's budget, its group's or the project's
/// daily budget.
fn stopped(ctx: &Ctx, run: &RunView, limit: RunLimit) -> Said {
  let (step, agent) = step_words(ctx, run.stopped_step());
  let checkpoint = checkpoint(run).filter(|_| run.saved_at_pause).unwrap_or_default();
  let kept = if checkpoint.is_empty() {
    ctx.t("summary.notice.stoppedUnsaved")
  } else {
    ctx.t_args(
      "summary.notice.stoppedKept",
      [
        ("checkpoint", checkpoint.as_str()),
        ("branch", run.run.branch().as_str()),
      ],
    )
  };
  let (title, body) = match limit {
    RunLimit::Tokens if !checkpoint.is_empty() => (
      ctx.t_args(
        "summary.notice.tokenLimitCheckpointTitle",
        [("checkpoint", checkpoint.as_str())],
      ),
      ctx.t_args(
        "summary.notice.tokenLimitBody",
        [
          ("step", step.as_str()),
          ("budget", &compact_tokens(run.run.budget().get())),
          ("kept", &kept),
        ],
      ),
    ),
    RunLimit::Tokens => (
      ctx.t("summary.notice.tokenLimitTitle"),
      ctx.t_args(
        "summary.notice.tokenLimitBody",
        [
          ("step", step.as_str()),
          ("budget", &compact_tokens(run.run.budget().get())),
          ("kept", &kept),
        ],
      ),
    ),
    RunLimit::Time => (
      ctx.t("summary.notice.timeLimitTitle"),
      ctx.t_args(
        "summary.notice.timeLimitBody",
        [("step", step.as_str()), ("kept", &*kept)],
      ),
    ),
    RunLimit::StepBudget { .. } => (
      ctx.t_args("summary.notice.stepBudgetTitle", [("step", step.as_str())]),
      ctx.t_args(
        "summary.notice.stepBudgetBody",
        [("step", step.as_str()), ("kept", &*kept)],
      ),
    ),
    RunLimit::GroupBudget => (
      ctx.t("summary.notice.groupBudgetTitle"),
      ctx.t_args("summary.notice.groupBudgetBody", [("kept", &*kept)]),
    ),
    RunLimit::DailyBudget => (
      ctx.t("summary.notice.dailyBudgetTitle"),
      ctx.t_args(
        "summary.notice.dailyBudgetBody",
        [("agent", agent.as_str()), ("kept", &*kept)],
      ),
    ),
  };
  Said::new(NoticeTone::Warning, Icon::Gauge, (title, body))
}

fn interrupted(ctx: &Ctx, run: &RunView) -> Said {
  let (step, _) = step_words(ctx, run.current_step());
  let branch = run.run.branch().as_str();
  if let Some(removed) = run.worktree_removed {
    let number = run.run.number().get().to_string();
    let time = display::clock(removed);
    let text = (
      ctx.t_args("summary.notice.cantContinueTitle", [("run", number.as_str())]),
      ctx.t_args(
        "summary.notice.cantContinueBody",
        [("time", time.as_str()), ("branch", branch)],
      ),
    );
    return Said::new(NoticeTone::Warning, Icon::FolderX, text);
  }
  let body = match checkpoint(run) {
    Some(checkpoint) => ctx.t_args("summary.notice.interruptedBody", [("checkpoint", checkpoint.as_str())]),
    None => ctx.t("summary.notice.interruptedNoCheckpointBody"),
  };
  let title = ctx.t_args("summary.notice.interruptedTitle", [("step", step.as_str())]);
  Said::new(NoticeTone::Warning, Icon::CirclePause, (title, body))
}

/// A run recorded as running whose runner is gone: it is interrupted until a runner is back.
fn lost(ctx: &Ctx, run: &RunView, runner: &RunnerState) -> Said {
  let (step, _) = step_words(ctx, run.current_step());
  let body = match runner {
    RunnerState::Reconnecting { .. } => ctx.t("summary.notice.lostReconnecting"),
    RunnerState::Failed { .. } => ctx.t("summary.notice.lostOffline"),
    _ => ctx.t("summary.notice.lostNotConnected"),
  };
  let title = ctx.t_args("summary.notice.interruptedTitle", [("step", step.as_str())]);
  Said::new(NoticeTone::Warning, Icon::CirclePause, (title, body))
}

/// A run that does not go on by itself: queued, ended, or waiting for the person's decision.
fn settled(ctx: &Ctx, detail: &TaskDetail, run: &RunView) -> Said {
  let (step, agent) = step_words(ctx, run.current_step());
  let branch = run.run.branch().as_str();
  let args = [("step", step.as_str()), ("agent", agent.as_str()), ("branch", branch)];
  let text = |title, body| (ctx.t_args(title, args), ctx.t_args(body, args));
  match run.run.status() {
    RunStatus::Pending => Said::new(
      NoticeTone::Info,
      Icon::Clock,
      text("summary.notice.pendingTitle", "summary.notice.pendingBody"),
    ),
    RunStatus::Succeeded if detail.merge.is_some() => Said::new(
      NoticeTone::Success,
      Icon::GitMerge,
      text("summary.notice.mergedTitle", "summary.notice.mergedBody"),
    ),
    RunStatus::Succeeded => Said::new(
      NoticeTone::Success,
      Icon::CircleCheck,
      text("summary.notice.succeededTitle", "summary.notice.succeededBody"),
    ),
    RunStatus::WaitingForPerson(Stuck::BrokeAgain(_)) => Said::new(
      NoticeTone::Warning,
      Icon::Hand,
      text("summary.notice.brokeAgainTitle", "summary.notice.stuckBody"),
    ),
    RunStatus::WaitingForPerson(_) => Said::new(
      NoticeTone::Warning,
      Icon::Hand,
      text("summary.notice.retriesTitle", "summary.notice.stuckBody"),
    ),
    RunStatus::Failed(_) if run.current_step().is_some_and(|step| step.tool_step) => Said::new(
      NoticeTone::Danger,
      Icon::CircleX,
      text("summary.notice.failedTitle", "summary.notice.failedCommandBody"),
    ),
    RunStatus::Failed(_) => Said::new(
      NoticeTone::Danger,
      Icon::CircleX,
      text("summary.notice.failedTitle", "summary.notice.failedBody"),
    ),
    RunStatus::Cancelled if run.run.current_attempt().is_none() => Said::new(
      NoticeTone::Info,
      Icon::CircleSlash,
      text(
        "summary.notice.cancelledBeforeStartTitle",
        "summary.notice.cancelledBeforeStartBody",
      ),
    ),
    _ => Said::new(
      NoticeTone::Info,
      Icon::CircleSlash,
      text("summary.notice.cancelledTitle", "summary.notice.cancelledBody"),
    ),
  }
}
