//! *Resolve the conflicts* under the files a merge into the base branch conflicted in (*Not merged into main*,
//! `D3kggY` in `U7c5T`; boards 1, 2 and 6 of `fvCVT`, `bP1fs`). *Resolve in editor* merges the base branch into the
//! run's worktree and opens it in the person's editor ([`orchester_workspace::tasks::resolve`]); the line under it
//! says which editor opens, or that none was found. *Ask an agent to resolve* returns the run to the agent step the
//! person picks among those that may write in its worktree, the implementing one first, with a note that says what to
//! merge and how ([`return_work::conflict_note`]); the pipeline goes on from there as after any return. Window input
//! from an MCP client may press either, as it may return a run (`continue_run`); it starts no editor a person did not
//! name for this ([`orchester_workspace::tasks::resolve::EDITOR_VARIABLE`]).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, events::is_synthetic_input, theme::SpacingSize},
  components::{Column, Row, SelectOption, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  resource::StepKey,
  run::{Answerer, MergeConflicts},
};
use orchester_workspace::tasks::{
  detail::RunView,
  integration_jobs::{EditorOpened, JobState, ResolveArgs, ResolveStep},
  resolve::{Editor, Worktree},
  return_work,
};

use super::super::{
  super::section_head,
  TaskContext,
  action::{DecisionAction, state},
};
use crate::{
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{SelectField, field_error},
    icon::Icon,
    message::message_text,
    unavailable::reason_line,
  },
};

/// The step select's width between *Resolve in editor* and *Ask an agent to resolve* (`ZYna8` in `o2e66y`).
const STEP_SELECT_WIDTH: f32 = 210.0;

/// The writing step *Ask an agent to resolve* returns the run to; `None` until it is seeded with the implementing
/// step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepChoice(pub Option<StepKey>);

impl lurq::app::component::DevtoolsInspectable for StepChoice {}

/// The return under way, for its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Asking;

impl lurq::app::component::DevtoolsInspectable for Asking {}

/// *Resolve the conflicts* of a merge that stopped at `conflicts`.
pub struct Offer<'a> {
  pub task: &'a TaskContext,
  pub run: &'a RunView,
  /// The run's worktree as last read; `None` before it is first read.
  pub worktree: Option<&'a Worktree>,
  pub conflicts: &'a [String],
  pub step: &'a Signal<StepChoice>,
  pub asked: &'a Signal<Option<Asking>>,
  pub action: &'a DecisionAction<Asking>,
}

impl Offer<'_> {
  pub fn render(&self, ctx: &mut Ctx) -> Element {
    let (editor, editor_line) = self.editor(ctx);
    let (agent, agent_reason) = self.agent(ctx);
    let (pending, refused) = state(self.action, self.asked.get());
    let agent = match agent {
      AgentPart::Choose(select, button) => vec![select, self.ask_button(button, pending).into()],
      AgentPart::Unavailable(button) => vec![button.into()],
    };
    let failure = match &self.task.jobs.run(self.run.id()).resolve.state {
      JobState::Failed(message) => Some(field_error("task-resolve-failed", &message_text(ctx, message))),
      _ => None,
    };
    Column::new()
      .id("task-resolve")
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .child(section_head(
        ctx,
        "task-resolve-head",
        "decisions.ready.resolve.title",
        None,
      ))
      .child(
        Row::new()
          .id("task-resolve-actions")
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(editor)
          .with_children(agent),
      )
      .child(
        Text::new(&editor_line)
          .id("task-resolve-editor")
          .variant(ExtraTypography::Note.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(agent_reason.map(|reason| reason_line("task-resolve-agent-reason", &reason)))
      .with_children(failure)
      .with_children(refused.map(|refused| field_error("task-resolve-refused", &message_text(ctx, &refused))))
      .into()
  }

  /// *Resolve in editor* and the line that says which editor opens.
  fn editor(&self, ctx: &mut Ctx) -> (Element, Arc<str>) {
    let services = Services::of(ctx);
    let button = ActionButton::new(
      "task-resolve-in-editor",
      ButtonKind::Secondary,
      ctx.t("decisions.ready.resolve.inEditor"),
    )
    .icon(Icon::FilePenLine);
    let Some(editor) = services.editor.clone() else {
      let reason = ctx.t("decisions.ready.resolve.noEditor");
      return (button.unavailable(reason.clone()).into(), reason);
    };
    let branch = self.run.run.base_branch().map_or("", |branch| branch.as_str());
    let line = editor_line(ctx, &editor, branch);
    let job = self.task.jobs.run(self.run.id());
    if job.resolving() {
      return (button.loading().into(), line);
    }
    let Some(worktree) = self.worktree else {
      return (button.disabled().into(), line);
    };
    if let Some(refusal) = return_work::refusal(&self.task.detail, self.run, worktree) {
      return (button.unavailable(message_text(ctx, &refusal.message())).into(), line);
    }
    if job.busy() {
      return (button.disabled().into(), line);
    }
    let args = resolve_args(ctx, self.task, self.run);
    let jobs = services.jobs.of(self.task.project.open.project.id()).of(self.run.id());
    let start = move || {
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
    };
    (button.on_click(start).into(), line)
  }

  /// The step select and *Ask an agent to resolve*, or why no agent can be asked.
  fn agent(&self, ctx: &mut Ctx) -> (AgentPart, Option<Arc<str>>) {
    let button = ActionButton::new(
      "task-resolve-ask-agent",
      ButtonKind::Secondary,
      ctx.t("decisions.ready.resolve.askAgent"),
    )
    .icon(Icon::Bot);
    if let Some(refusal) = self
      .worktree
      .and_then(|worktree| return_work::refusal(&self.task.detail, self.run, worktree))
    {
      let reason = message_text(ctx, &refusal.message());
      return (AgentPart::Unavailable(button.unavailable(reason.clone())), Some(reason));
    }
    let writing = return_work::writing_targets(self.run);
    if writing.is_empty() {
      let reason = ctx.t("decisions.ready.resolve.noWritingStep");
      return (AgentPart::Unavailable(button.unavailable(reason.clone())), Some(reason));
    }
    if !self.task.online {
      let reason = ctx.t("task.controls.offline");
      return (AgentPart::Unavailable(button.unavailable(reason.clone())), Some(reason));
    }
    if self.step.get_untracked().0.is_none() {
      self.step.set(StepChoice(return_work::implementing_step(self.run)));
    }
    let options = writing
      .iter()
      .map(|step| {
        let agent = step.agent.clone().unwrap_or_else(|| ctx.t("decisions.theAgent"));
        let label = ctx.t_args(
          "decisions.ready.resolve.stepOption",
          [("step", step.name.as_ref()), ("agent", agent.as_ref())],
        );
        SelectOption::new(StepChoice(Some(step.key.clone())), label)
      })
      .collect();
    let name = ctx.t("decisions.ready.resolve.step");
    let select = SelectField::new("task-resolve-step", name, self.step.clone(), options)
      .width(Dimension::Px(STEP_SELECT_WIDTH))
      .render(ctx);
    (AgentPart::Choose(select, button), None)
  }

  /// *Ask an agent to resolve*, which returns the run to the chosen step with the conflict's note.
  fn ask_button(&self, button: ActionButton, pending: Option<Asking>) -> ActionButton {
    let chosen = self.step.get().0.filter(|key| {
      return_work::writing_targets(self.run)
        .iter()
        .any(|step| step.key == *key)
    });
    let (Some(step), Some(base)) = (chosen, self.run.run.base_branch()) else {
      return button.disabled();
    };
    if pending.is_some() {
      return button.loading();
    }
    // The worktree is read first, and nothing of the run works on it meanwhile: *Resolve in editor* may be merging.
    if self.worktree.is_none() || self.task.jobs.run(self.run.id()).busy() {
      return button.disabled();
    }
    let conflicts = MergeConflicts {
      base: base.clone(),
      files: self.conflicts.to_vec(),
    };
    let note = return_work::conflict_note(base);
    let (person, automation) = (
      return_work::command(
        self.run,
        step.clone(),
        (note.clone(), Some(conflicts.clone())),
        Answerer::Person,
      ),
      return_work::command(self.run, step, (note, Some(conflicts)), Answerer::Automation),
    );
    let (action, asked) = (self.action.clone(), self.asked.clone());
    button.on_click(move || {
      // A second press while the first is under way would replace it (lurq's `FutureAction::run`).
      if action.state().get_untracked().is_pending() {
        return;
      }
      asked.set(Some(Asking));
      // An MCP client's press is automation's, as a return it asks for with `continue_run` is.
      let command = if is_synthetic_input() { &automation } else { &person };
      action.run((Asking, command.clone()));
    })
  }
}

/// The step select with its button, or the button that cannot be used.
enum AgentPart {
  Choose(Element, ActionButton),
  Unavailable(ActionButton),
}

/// What *Resolve in editor* does with `branch` and which editor opens, as the line under it says it.
fn editor_line(ctx: &Ctx, editor: &Editor, branch: &str) -> Arc<str> {
  match editor {
    Editor::Named(program) => {
      let name = program.file_name().map_or_else(
        || program.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
      );
      ctx.t_args(
        "decisions.ready.resolve.opensNamed",
        [("branch", branch.to_owned()), ("program", name)],
      )
    }
    Editor::VsCode(_) => ctx.t_args("decisions.ready.resolve.opensVsCode", [("branch", branch)]),
    Editor::SystemFolders => ctx.t_args("decisions.ready.resolve.opensFolder", [("branch", branch)]),
  }
}

/// What resolving `run`'s merge by hand needs; its editor is filled in where it is started.
pub fn resolve_args(ctx: &mut Ctx, task: &TaskContext, run: &RunView) -> ResolveArgs {
  let services = Services::of(ctx);
  let feed = task.project.feed.clone();
  ResolveArgs {
    open: task.project.open.clone(),
    run: run.id(),
    programs: services.programs,
    editor: Err(EditorOpened::NoEditor),
    changed: Arc::new(move || feed.saved()),
  }
}
