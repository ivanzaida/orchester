//! A merge that stopped on conflicts (13c `zWlkp`): *Conflicting files* with *Open in Review →*, each file with how
//! many conflicts it holds, and in the bar *Resolve in editor*, the agent field and *Ask the agent to resolve*. On the
//! run's resolve jobs, which the task page continues (`screens::task::summary::decisions::ready::resolving`):
//! *Resolve in editor* merges the base branch into the run's worktree and opens it in the person's editor
//! (`orchester_workspace::tasks::resolve`); *Ask the agent to resolve* returns the run to its implementing step with
//! the conflict's note, as the review's conflict bar does (`conflicts::resolve`), an MCP client's press only while a
//! person lets automation decide. Both wait for the run's worktree as last read, and say why they cannot be used: the
//! run is not the task's latest, it has not ended, its worktree is gone, or a merge already waits in it (continued on
//! the task page).

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ResourceId, RunId, TaskId},
  project::TaskKey,
  run::{Answerer, RunNumber},
};
use orchester_workspace::{
  background,
  decisions::DecisionKind,
  messages::{self, Message},
  tasks::{
    detail::{RunView, TaskDetail},
    integration_jobs::{EditorOpened, JobState, ResolveArgs, ResolveStep},
    resolve::{self, Worktree},
    return_work,
    review_threads::{
      conflicts::resolve as resolving_run,
      page::{ReviewPage, review_page},
    },
  },
};

use super::{facts::link, section_head};
use crate::{
  mcp::input_gate::{Guarded, InputGate, answerer},
  navigation::link::Open,
  project::ShownProject,
  screens::review::task::{
    agent_field::{self, AgentChoice, Place},
    conflict::{count_in, tag_words},
  },
  services::Services,
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
  },
};

/// The return under way, for its button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Asking;

impl DevtoolsInspectable for Asking {}

/// What *Ask the agent to resolve* sends: the run, who decides the return and the profile chosen for it.
pub(super) type AskAction = FutureAction<(RunId, Answerer, Option<ResourceId>), Asking, Message>;

/// The run as resolving needs it: its task's latest run, its worktree as last read, and its review page, which says
/// where the files conflict and which agents the return may choose.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Resolving {
  pub detail: Arc<TaskDetail>,
  pub worktree: Worktree,
  pub review: Option<Arc<ReviewPage>>,
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

/// The run whose conflicts the page resolves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Wanted {
  pub task: TaskId,
  pub run: RunId,
  pub key: TaskKey,
  pub number: RunNumber,
}

/// Reads what resolving `wanted`, a run of its task, needs, again whenever `changes` change; nothing without one.
pub(super) fn read(
  ctx: &mut Ctx,
  project: &ShownProject,
  wanted: Option<Wanted>,
  changes: (u64, u64, u64),
) -> Option<Resolving> {
  let root = project.open.repo_root.clone();
  let idle = wanted
    .as_ref()
    .is_none_or(|wanted| !project.feed.job_state.get().run(wanted.run).resolving());
  let tasks = project.feed.tasks.with(|tasks| tasks.ready().cloned());
  let services = Services::of(ctx);
  let (worktrees, hold, programs) = (services.run_worktrees(&project.open), services.hold, services.programs);
  ctx
    .future(
      (root, wanted, tasks, idle, changes),
      move |(root, wanted, tasks, idle, _)| {
        let (worktrees, hold, programs) = (worktrees.clone(), hold.clone(), programs.clone());
        async move {
          let Some(wanted) = wanted else {
            return Ok::<_, Message>(None);
          };
          hold.released().await;
          background::run(move || -> Result<Option<Resolving>, Message> {
            let detail = TaskDetail::load_in(&root, wanted.task).map_err(|error| messages::store_error(&error))?;
            let worktree = resolve::read(&worktrees, &root, (wanted.run, idle))?;
            let review = match &tasks {
              Some(tasks) => review_page(tasks, (&root, &programs), (&wanted.key, wanted.number))
                .map_err(|refusal| refusal.message())?
                .map(Arc::new),
              None => None,
            };
            Ok(Some(Resolving {
              detail: Arc::new(detail),
              worktree,
              review,
            }))
          })
          .await?
        }
      },
    )
    .state()
    .get()
    .data
    .flatten()
}

/// Sends *Ask the agent to resolve*'s return of a run to the runner of `project`; once it took it, the merge that
/// failed is forgotten, so the run that works and succeeds again is not shown its old conflicts.
pub(super) fn ask_action(ctx: &mut Ctx, project: &ShownProject) -> AskAction {
  let services = Services::of(ctx);
  let worktrees = services.run_worktrees(&project.open);
  let runners = services.runners;
  let (open, feed) = (project.open.clone(), project.feed.clone());
  ctx.future_action(move |(run, by, profile): (RunId, Answerer, Option<ResourceId>)| {
    let (runners, worktrees, open, jobs) = (runners.clone(), worktrees.clone(), open.clone(), feed.jobs.of(run));
    async move {
      let root = open.repo_root.clone();
      let command = background::run(move || resolving_run::begin_by(&root, &worktrees, run, (profile, by)))
        .await?
        .map_err(|refusal| refusal.message())?;
      resolving_run::send(&runners, &open, command)
        .await
        .map_err(|refusal| refusal.message())?;
      jobs.forget_merge_failure();
      Ok(Asking)
    }
  })
}

/// *Resolve in editor*, a ghost action of the bar: waits for the worktree to be read, and is unavailable without an
/// editor or while the run cannot be returned to.
fn editor_button(ctx: &mut Ctx, project: &ShownProject, run: RunId, resolving: Option<&Resolving>) -> Element {
  let services = Services::of(ctx);
  let button = ActionButton::new(
    "merge-attempt-resolve-in-editor",
    ButtonKind::Ghost,
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

/// What the conflicts section and the bar ask of the page.
pub(super) struct Conflicts<'a> {
  pub project: &'a ShownProject,
  pub run: RunId,
  pub files: &'a [String],
  pub resolving: Option<&'a Resolving>,
  pub choice: &'a AgentChoice,
  pub ask: &'a AskAction,
  /// Window input pressed *Ask the agent to resolve* while a person does not let automation decide for them.
  pub person_only: &'a Signal<bool>,
}

impl Conflicts<'_> {
  /// *Conflicting files*, with *Open in Review* at the end of its head: `review` opens the task's review.
  pub(super) fn section(&self, ctx: &Ctx, review: Open) -> Element {
    let conflicts = self
      .resolving
      .and_then(|resolving| resolving.review.as_ref()?.conflicts.as_ref());
    let entries = self.files.iter().map(|path| {
      let count = conflicts.map_or(0, |conflicts| count_in(conflicts, path));
      let words = if count == 0 {
        ctx.t("merges.page.conflicted")
      } else {
        tag_words(ctx, count)
      };
      (path.as_str(), words, PaletteColor::Danger)
    });
    let list = files_list(entries);
    let open = link(
      "merge-attempt-open-in-review",
      (Icon::ArrowRight, &ctx.t("merges.page.openInReview")),
      Press::Open(review),
    );
    Column::new()
      .id("merge-attempt-conflicts")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(section_head(
        "merge-attempt-conflicting-head",
        &ctx.t("merges.page.conflicting"),
        (Some(&self.files.len().to_string()), Some(open)),
      ))
      .child(list)
      .into()
  }

  /// What went wrong with *Resolve in editor* or *Ask the agent to resolve*, for the page to show with its other
  /// failures.
  pub(super) fn failure(&self) -> Option<Message> {
    if let JobState::Failed(message) = &self.project.feed.job_state.get().run(self.run).resolve.state {
      return Some(message.clone());
    }
    if self.person_only.get() {
      return Some(Message::new("appMcp.errors.automationNotAllowed"));
    }
    let asked = self.ask.state().get();
    let pending = asked.is_pending();
    asked.error.filter(|_| !pending)
  }

  /// Whether *Resolve in editor* found nothing to resolve: git merged the base branch without a conflict, or the run's
  /// branch held it already, so no editor was opened.
  pub(super) fn nothing_to_resolve(&self) -> bool {
    self
      .project
      .feed
      .job_state
      .get()
      .run(self.run)
      .resolve
      .nothing_to_resolve
  }

  /// The bar's actions: *Resolve in editor*, the agent field and *Ask the agent to resolve*.
  pub(super) fn actions(&self, ctx: &mut Ctx) -> Vec<Element> {
    let review = self.resolving.and_then(|resolving| resolving.review.as_ref());
    let agent = review.and_then(|review| agent_field::field(ctx, (review, self.choice), Place::Merge));
    let mut actions = vec![editor_button(ctx, self.project, self.run, self.resolving)];
    actions.extend(agent);
    actions.push(self.ask_button(ctx, review));
    actions
  }

  /// Why neither way to resolve can be used now, for the bar to say in place of its words: there is no editor, or
  /// [`Self::refusal`].
  pub(super) fn stuck(&self, ctx: &mut Ctx) -> Option<Arc<str>> {
    if Services::of(ctx).editor.is_none() {
      return Some(ctx.t("decisions.ready.resolve.noEditor"));
    }
    self.refusal(ctx)
  }

  /// Why the run cannot be returned to its agent now; `None` when it can, or while its worktree is read.
  pub(super) fn refusal(&self, ctx: &Ctx) -> Option<Arc<str>> {
    let resolving = self.resolving?;
    resolving.refusal(ctx, self.run).or_else(|| {
      let view = resolving.view(self.run)?;
      if return_work::writing_targets(view).is_empty() {
        return Some(ctx.t("decisions.ready.resolve.noWritingStep"));
      }
      let online = self.project.feed.runner_online(&self.project.open.repo_root);
      (!online).then(|| ctx.t("task.controls.offline"))
    })
  }

  /// *Ask the agent to resolve*: unavailable, with why, while the run cannot be returned to.
  fn ask_button(&self, ctx: &mut Ctx, review: Option<&Arc<ReviewPage>>) -> Element {
    let button = ActionButton::new(
      "merge-attempt-ask-agent",
      ButtonKind::Primary,
      ctx.t("merges.page.bar.askAgent"),
    )
    .icon(Icon::Bot);
    let (Some(_), Some(review)) = (self.resolving, review) else {
      return button.disabled().into();
    };
    if let Some(reason) = self.refusal(ctx) {
      return button.unavailable(reason).into();
    }
    if self.ask.state().get().is_pending() {
      return button.loading().into();
    }
    if self.project.feed.job_state.get().run(self.run).busy() {
      return button.disabled().into();
    }
    let (ask, person_only, gate) = (
      self.ask.clone(),
      self.person_only.clone(),
      ctx.use_context::<InputGate>(),
    );
    let (run, choice, review) = (self.run, self.choice.clone(), review.clone());
    button
      .on_click(move || {
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
        ask.run((run, by, choice.profile(&review)));
      })
      .into()
  }
}

/// The files of a section in one outline, each with its glyph, its path and its words, both in `color`: a conflicting
/// file says how many conflicts it holds, or *Conflicted* while the review's read of the diff, which says where they
/// are, is not there; a file that conflicted before the run was merged says *Conflicted then*.
pub(super) fn files_list<'a>(entries: impl Iterator<Item = (&'a str, Arc<str>, PaletteColor)>) -> Element {
  let entries: Vec<_> = entries.collect();
  let last = entries.len();
  let rows: Vec<Element> = entries
    .into_iter()
    .enumerate()
    .map(|(index, entry)| file_row(index, entry, index + 1 == last))
    .collect();
  Column::new()
    .id("merge-attempt-conflicting-files")
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .with_children(rows)
    .into()
}

fn file_row(index: usize, (path, words, color): (&str, Arc<str>, PaletteColor), last: bool) -> Element {
  let mut row = Row::new()
    .id(format!("merge-attempt-conflict-{index}"))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Lg.value()))
    .padding(Padding::horizontal(ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::GitCompareArrows, IconSize::Md, color.clone()))
    .child(
      Text::new(path)
        .variant(ExtraTypography::PropertyMono.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap(),
    )
    .child(
      Text::new(&words)
        .id(format!("merge-attempt-conflict-{index}-count"))
        .variant(ExtraTypography::Property.role())
        .color(color)
        .nowrap(),
    );
  if !last {
    row = row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
  }
  row.into()
}
