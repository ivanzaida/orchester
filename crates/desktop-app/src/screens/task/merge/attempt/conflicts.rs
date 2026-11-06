//! A merge that stopped on conflicts (09f `Q6yPN8`): *Conflicting files*, *Ask an agent to resolve* with the writing
//! step it returns the run to, and the line that says what *Resolve in editor*, the page's primary action, does. On the
//! run's resolve jobs, which the task page continues (`screens::task::summary::decisions::ready::resolving`):
//! *Resolve in editor* merges the base branch into the run's worktree and opens it in the person's editor
//! (`orchester_workspace::tasks::resolve`); *Ask an agent to resolve* returns the run to the chosen step with the
//! conflict's note (`return_work::resolve_command`), an MCP client's press only while a person lets automation decide.
//! Both wait for the run's worktree as last read, and say why they cannot be used: the run is not the task's latest, it
//! has not ended, its worktree is gone, or a merge already waits in it (continued on the task page).

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{RunId, TaskId},
  resource::StepKey,
  run::Answerer,
};
use orchester_ipc::Command;
use orchester_workspace::{
  background,
  decisions::DecisionKind,
  messages::{self, Message},
  tasks::{
    detail::{RunView, TaskDetail},
    integration_jobs::{EditorOpened, JobState, ResolveArgs, ResolveStep},
    resolve::{self, Editor, Worktree},
    return_work,
    run::control_message,
  },
};

use super::section_head;
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  project::ShownProject,
  services::Services,
  theme::{AttemptSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{SelectField, field_error},
    icon::{Icon, icon},
    message::message_text,
    unavailable::reason_line,
  },
};

/// The step select's width (`r20mh`).
const STEP_SELECT_WIDTH: f32 = 180.0;

/// The writing step *Ask an agent to resolve* returns the run to; `None` until it is seeded with the implementing one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct StepChoice(pub Option<StepKey>);

impl DevtoolsInspectable for StepChoice {}

/// The return under way, for its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Asking;

impl DevtoolsInspectable for Asking {}

/// The return of a run *Ask an agent to resolve* sends.
pub(super) type AskAction = FutureAction<(RunId, Command), Asking, Message>;

/// The run as resolving needs it: its task's latest run, and its worktree as last read.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Resolving {
  pub detail: Arc<TaskDetail>,
  pub worktree: Worktree,
}

impl DevtoolsInspectable for Resolving {}

impl Resolving {
  /// The run, while it is its task's latest.
  fn view(&self, run: RunId) -> Option<&RunView> {
    self.detail.run.as_ref().filter(|view| view.id() == run)
  }

  /// Why neither way to resolve can be used now; `None` when they can.
  fn refusal(&self, ctx: &Ctx, run: RunId) -> Option<Arc<str>> {
    let Some(view) = self.view(run) else {
      return Some(ctx.t("merges.page.resolve.notLatest"));
    };
    return_work::refusal(&self.detail, view, &self.worktree).map(|refusal| message_text(ctx, &refusal.message()))
  }
}

/// Reads what resolving `wanted`, a run of its task, needs, again whenever `changes` change; nothing without one.
pub(super) fn read(
  ctx: &mut Ctx,
  project: &ShownProject,
  wanted: Option<(TaskId, RunId)>,
  changes: (u64, u64, u64),
) -> Option<Resolving> {
  let root = project.open.repo_root.clone();
  let idle = wanted.is_none_or(|(_, run)| !project.feed.job_state.get().run(run).resolving());
  let services = Services::of(ctx);
  let (worktrees, hold) = (services.run_worktrees(&project.open), services.hold);
  ctx
    .future((root, wanted, idle, changes), move |(root, wanted, idle, _)| {
      let (worktrees, hold) = (worktrees.clone(), hold.clone());
      async move {
        let Some((task, run)) = wanted else {
          return Ok::<_, Message>(None);
        };
        hold.released().await;
        background::run(move || -> Result<Option<Resolving>, Message> {
          let detail = TaskDetail::load_in(&root, task).map_err(|error| messages::store_error(&error))?;
          let worktree = resolve::read(&worktrees, &root, (run, idle))?;
          Ok(Some(Resolving {
            detail: Arc::new(detail),
            worktree,
          }))
        })
        .await?
      }
    })
    .state()
    .get()
    .data
    .flatten()
}

/// Sends *Ask an agent to resolve*'s return of a run to the runner of `project`; once it took it, the merge that
/// failed is forgotten, so the run that works and succeeds again is not shown its old conflicts.
pub(super) fn ask_action(ctx: &mut Ctx, project: &ShownProject) -> AskAction {
  let runners = Services::of(ctx).runners;
  let (open, feed) = (project.open.clone(), project.feed.clone());
  ctx.future_action(move |(run, command): (RunId, Command)| {
    let (runners, open, jobs) = (runners.clone(), open.clone(), feed.jobs.of(run));
    async move {
      runners.control_run_in(&open, command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not take the return");
        control_message(&error)
      })?;
      jobs.forget_merge_failure();
      Ok(Asking)
    }
  })
}

/// *Resolve in editor*, the page's primary action on conflicts: waits for the worktree to be read, and is unavailable
/// without an editor or while the run cannot be returned to.
pub(super) fn editor_button(
  ctx: &mut Ctx,
  project: &ShownProject,
  run: RunId,
  resolving: Option<&Resolving>,
) -> Element {
  let services = Services::of(ctx);
  let button = ActionButton::new(
    "merge-attempt-resolve-in-editor",
    ButtonKind::Primary,
    ctx.t("decisions.ready.resolve.inEditor"),
  )
  .icon(Icon::CodeXml);
  let Some(editor) = services.editor.clone() else {
    return button.unavailable(ctx.t("decisions.ready.resolve.noEditor")).into();
  };
  let job = project.feed.job_state.get().run(run);
  if job.resolving() {
    return button.loading().into();
  }
  let Some(resolving) = resolving else {
    return button.disabled().into();
  };
  if let Some(reason) = resolving.refusal(ctx, run) {
    return button.unavailable(reason).into();
  }
  if job.busy() {
    return button.disabled().into();
  }
  let feed = project.feed.clone();
  let args = ResolveArgs {
    open: project.open.clone(),
    run,
    worktrees: services.run_worktrees(&project.open),
    editor: Err(EditorOpened::NoEditor),
    changed: Arc::new(move || feed.saved()),
  };
  let jobs = project.feed.jobs.of(run);
  button
    .on_click(move || {
      let opening = if is_synthetic_input() && !editor.named() {
        Err(EditorOpened::PersonOnly)
      } else {
        Ok(editor.clone())
      };
      jobs.start_resolving(
        ResolveStep::Start,
        ResolveArgs {
          editor: opening,
          ..args.clone()
        },
      );
    })
    .into()
}

/// What the conflicts section asks of the page.
pub(super) struct Conflicts<'a> {
  pub project: &'a ShownProject,
  pub run: RunId,
  pub base: &'a str,
  pub files: &'a [String],
  pub resolving: Option<&'a Resolving>,
  pub step: &'a Signal<StepChoice>,
  pub ask: &'a AskAction,
  /// Window input pressed *Ask an agent to resolve* while a person does not let automation decide for them.
  pub person_only: &'a Signal<bool>,
}

impl Conflicts<'_> {
  /// *Conflicting files* with their rows, then *Ask an agent to resolve* and the editor's line.
  pub(super) fn render(&self, ctx: &mut Ctx) -> Element {
    let rows: Vec<Element> = self
      .files
      .iter()
      .enumerate()
      .map(|(index, path)| file_row(ctx, index, path, index + 1 == self.files.len()))
      .collect();
    let list = Column::new()
      .id("merge-attempt-conflicting-files")
      .width(Dimension::Pct(100.0))
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .clip()
      .with_children(rows);
    let failure = match &self.project.feed.job_state.get().run(self.run).resolve.state {
      JobState::Failed(message) => Some(field_error("merge-attempt-resolve-failed", &message_text(ctx, message))),
      _ => None,
    };
    let refused = self
      .ask
      .state()
      .get()
      .error
      .filter(|_| !self.ask.state().get().is_pending());
    Column::new()
      .id("merge-attempt-conflicts")
      .width(Dimension::Pct(100.0))
      .spacing(crate::theme::DeliverySpacing::Lanes.role())
      .child(section_head(
        "merge-attempt-conflicting-head",
        &ctx.t("merges.page.conflicting"),
        Some(&self.files.len().to_string()),
      ))
      .child(list)
      .child(self.agent(ctx))
      .child(
        Text::new(&self.editor_line(ctx))
          .id("merge-attempt-resolve-editor")
          .variant(ExtraTypography::Property.role())
          .color(PaletteColor::TextMuted)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(failure)
      .with_children(self.person_only.get().then(|| {
        field_error(
          "merge-attempt-resolve-person-only",
          &message_text(ctx, &Message::new("appMcp.errors.automationNotAllowed")),
        )
      }))
      .with_children(refused.map(|refused| field_error("merge-attempt-resolve-refused", &message_text(ctx, &refused))))
      .into()
  }

  /// *Ask an agent to resolve*, *Step* and its select; or the button that cannot be used and why.
  fn agent(&self, ctx: &mut Ctx) -> Element {
    let button = ActionButton::new(
      "merge-attempt-ask-agent",
      ButtonKind::Secondary,
      ctx.t("decisions.ready.resolve.askAgent"),
    )
    .icon(Icon::Bot);
    let row = Row::new()
      .id("merge-attempt-resolve-actions")
      .spacing(crate::theme::ExtraSpacing::Loose.role())
      .align_items(Alignment::Center);
    let Some(resolving) = self.resolving else {
      return row.child(button.disabled()).into();
    };
    let reason = resolving.refusal(ctx, self.run).or_else(|| {
      let view = resolving.view(self.run)?;
      if return_work::writing_targets(view).is_empty() {
        return Some(ctx.t("decisions.ready.resolve.noWritingStep"));
      }
      let online = self.project.feed.runner_online(&self.project.open.repo_root);
      (!online).then(|| ctx.t("task.controls.offline"))
    });
    if let Some(reason) = reason {
      return Column::new()
        .spacing(crate::theme::ExtraSpacing::Snug.role())
        .child(row.child(button.unavailable(reason.clone())))
        .child(reason_line("merge-attempt-ask-agent-reason", &reason))
        .into();
    }
    let view = resolving
      .view(self.run)
      .expect("a run with no refusal is its task's latest");
    if self.step.get_untracked().0.is_none() {
      self.step.set(StepChoice(return_work::implementing_step(view)));
    }
    let options = return_work::writing_targets(view)
      .iter()
      .map(|step| {
        let agent = step.agent.clone().unwrap_or_else(|| ctx.t("decisions.theAgent"));
        let label = ctx.t_args(
          "merges.page.stepOption",
          [
            ("number", format!("{:02}", step.number)),
            ("step", step.name.to_string()),
            ("agent", agent.to_string()),
          ],
        );
        SelectOption::new(StepChoice(Some(step.key.clone())), label)
      })
      .collect();
    let name = ctx.t("decisions.ready.resolve.step");
    let select = SelectField::new("merge-attempt-resolve-step", name.clone(), self.step.clone(), options)
      .width(Dimension::Px(STEP_SELECT_WIDTH))
      .render(ctx);
    row
      .child(self.ask_button(ctx, button, view))
      .child(
        Text::new(&name)
          .variant(ExtraTypography::Property.role())
          .color(PaletteColor::TextMuted),
      )
      .child(select)
      .into()
  }

  /// *Ask an agent to resolve*, which returns the run to the chosen step with the conflict's note.
  fn ask_button(&self, ctx: &mut Ctx, button: ActionButton, view: &RunView) -> ActionButton {
    let chosen = self
      .step
      .get()
      .0
      .filter(|key| return_work::writing_targets(view).iter().any(|step| step.key == *key));
    let returned = |by| return_work::resolve_command(view, chosen.clone()?, (self.files, None), by);
    let (Some(person), Some(automation)) = (returned(Answerer::Person), returned(Answerer::Automation)) else {
      return button.disabled();
    };
    if self.ask.state().get().is_pending() {
      return button.loading();
    }
    if self.project.feed.job_state.get().run(self.run).busy() {
      return button.disabled();
    }
    let (ask, person_only, gate) = (
      self.ask.clone(),
      self.person_only.clone(),
      ctx.use_context::<InputGate>(),
    );
    let run = self.run;
    button.on_click(move || {
      // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
      if ask.state().get_untracked().is_pending() {
        return;
      }
      // Returning a run decides for the person, as `continue_run` does: an MCP client's press only while a person
      // lets automation answer, and it is recorded as automation's.
      let Some(by) = answerer(gate.as_ref(), Guarded::Decision(DecisionKind::Stuck)) else {
        tracing::warn!("synthetic input asked an agent to resolve without consent; not taken");
        person_only.set(true);
        return;
      };
      person_only.set(false);
      let command = if by == Answerer::Automation {
        automation.clone()
      } else {
        person.clone()
      };
      ask.run((run, command));
    })
  }

  /// What *Resolve in editor* does with the base branch and which editor opens.
  fn editor_line(&self, ctx: &mut Ctx) -> Arc<str> {
    let branch = self.base;
    match Services::of(ctx).editor {
      Some(Editor::Named(program)) => {
        let name = program.file_name().map_or_else(
          || program.display().to_string(),
          |name| name.to_string_lossy().into_owned(),
        );
        ctx.t_args(
          "merges.page.resolve.opensNamed",
          [("branch", branch.to_owned()), ("program", name)],
        )
      }
      Some(Editor::VsCode(_)) => ctx.t_args("merges.page.resolve.opensVsCode", [("branch", branch)]),
      Some(Editor::SystemFolders) => ctx.t_args("merges.page.resolve.opensFolder", [("branch", branch)]),
      None => ctx.t("decisions.ready.resolve.noEditor"),
    }
  }
}

/// A conflicting file: its glyph, its path and *Conflicted*.
fn file_row(ctx: &Ctx, index: usize, path: &str, last: bool) -> Element {
  let mut row = Row::new()
    .id(format!("merge-attempt-conflict-{index}"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, AttemptSpacing::CheckY.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(icon(Icon::FileCode, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Text::new(path)
        .variant(ExtraTypography::PropertyMono.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t("merges.page.conflicted"))
        .variant(ExtraTypography::Property.role())
        .color(PaletteColor::Warning)
        .nowrap(),
    );
  if !last {
    row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  row.into()
}
