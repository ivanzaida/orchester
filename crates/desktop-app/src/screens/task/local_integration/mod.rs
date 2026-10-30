//! ORC-24 native local integration (zMOBO): one page, inline decisions, app-local scrolling.

mod controls;
mod diff;
mod draft_controls;
mod evidence;
mod fields;
mod files;
mod notes;
mod refresh;
mod resolver;
mod selection;
mod strip;
mod target;
pub(crate) mod words;
mod work;

use std::{
  sync::{Arc, atomic::AtomicBool},
  time::Duration,
};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Interval},
    theme::{Breakpoint, SpacingSize},
  },
  components::{Column, Row, ScrollVertical},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::run::RunStatus;
use orchester_workspace::{
  background,
  tasks::local_integration::{DiscardReport, IntegrationView, Preview},
};
use selection::{Selection, ShownFile};
use work::{Outcome, Problem, Work};

use super::TaskContext;
use crate::{
  services::Services,
  ui::{
    notice::{Notice, NoticeTone},
    skeleton::{self, Skeleton},
  },
};

/// The inspector beside the evidence (`atP1P` in YxF6B); in a window narrower than the `xl` breakpoint it keeps the
/// width s4gj9 draws at 960 (`vdY0R`).
const INSPECTOR_WIDTH: f32 = 336.0;
const NARROW_INSPECTOR_WIDTH: f32 = 248.0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
  Review,
  Commit {
    tree: String,
    preview: Preview,
  },
  Preserve,
  /// The person pressed *Discard selected*: what discarding does to each selected file, to confirm.
  Discard,
  /// Discarding left some files as they were: which were discarded, which not and why, to try again.
  Discarded(DiscardReport),
  Abort,
  Cleanup,
  Cancel,
}
impl DevtoolsInspectable for Mode {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shown(Arc<IntegrationView>, u64);
impl DevtoolsInspectable for Shown {}

pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
  ctx.mount_keyed::<target::Target>("local-integration-target", task.clone())
}

fn page(ctx: &mut Ctx, task: &TaskContext) -> Element {
  if task
    .detail
    .run
    .as_ref()
    .is_none_or(|run| run.run.status() != &RunStatus::Succeeded)
  {
    return Notice::new(
      "local-not-ready",
      NoticeTone::Warning,
      ctx.t("local_integration.notReady"),
    )
    .into();
  }
  let run = task
    .detail
    .run
    .as_ref()
    .expect("the local page requires a succeeded run");
  ctx.mount_keyed::<LocalPage>(&format!("task-local-integration-{}", run.id()), task.clone())
}

struct LocalPage {
  revision: Signal<u64>,
  reading: Signal<bool>,
  editor_dirty: Signal<bool>,
  resolver: resolver::EditorState,
  acknowledgement: controls::AcknowledgementState,
  managed: controls::Managed,
  selected: Signal<Selection>,
  shown_file: Signal<ShownFile>,
  mode: Signal<Mode>,
  message: Signal<String>,
  name: Signal<String>,
  work: Work,
  stop: Arc<AtomicBool>,
  last: refresh::LastRead,
  _poll: Interval,
}

impl Component for LocalPage {
  type Props = TaskContext;

  fn create(ctx: &mut Ctx) -> Self {
    let revision = ctx.signal(0);
    let changes = revision.clone();
    let reading = ctx.signal(false);
    let pending = reading.clone();
    let poll = ctx.create_interval(Duration::from_secs(2), move || {
      if !pending.get_untracked() {
        changes.update(|value| *value += 1);
      }
    });
    poll.start();
    let mode = ctx.signal(Mode::Review);
    let action = work::action(ctx);
    let resolver = resolver::EditorState::new(ctx);
    let completed_editor = resolver.editor.clone();
    let completed_leave = resolver.leave.clone();
    let navigation = crate::navigation::Navigation::of(ctx);
    let completed_mode = mode.clone();
    let selected = ctx.signal(Selection::default());
    let completed_selection = selected.clone();
    let last = refresh::LastRead::default();
    let completed_last = last.clone();
    // The work's own `feed.saved()` already reads the view again once; the last read must not stand in for it.
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending() {
        completed_last.forget();
      }
      if !state.is_pending()
        && state.error.is_none()
        && let Some(outcome) = &state.data
      {
        completed_mode.set(match outcome {
          Outcome::CommitReview { tree, preview } => Mode::Commit {
            tree: tree.clone(),
            preview: preview.clone(),
          },
          Outcome::Discarded(report) if !report.complete() => Mode::Discarded(report.clone()),
          Outcome::Changed
          | Outcome::Aborted
          | Outcome::DraftSaved(_)
          | Outcome::Resolved(_)
          | Outcome::Discarded(_) => Mode::Review,
        });
        if let Outcome::Discarded(report) = outcome {
          completed_selection.update(|selection| selection.forget(report));
        }
        if let Outcome::DraftSaved(captured) = outcome {
          completed_editor.saved(captured);
          if !completed_editor.dirty() {
            completed_leave.leave(&navigation);
          }
        }
      }
    });
    Self {
      revision,
      reading,
      editor_dirty: ctx.signal(false),
      resolver,
      acknowledgement: controls::AcknowledgementState::new(ctx),
      managed: Arc::new(parking_lot::Mutex::new(None)),
      selected,
      shown_file: ctx.signal(ShownFile(None)),
      mode,
      message: Signal::new(String::new()),
      name: Signal::new(String::new()),
      work: action,
      stop: Arc::new(AtomicBool::new(false)),
      last,
      _poll: poll,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let dirty = self.resolver.editor.dirty();
    if self.editor_dirty.get_untracked() != dirty {
      self.editor_dirty.set(dirty);
    }
    let task = ctx.props::<TaskContext>().clone();
    let Some(run) = &task.detail.run else {
      return Element::from(Column::new());
    };
    let root = task.project.open.repo_root.clone();
    let Services { vault, programs, .. } = Services::of(ctx);
    let last = self.last.clone();
    let read = ctx
      .future(
        (
          root,
          run.id(),
          self.revision.get(),
          task.project.feed.revision.get(),
          self.shown_file.get(),
        ),
        move |(root, run, _, policy_revision, selected)| {
          let (vault, programs, last) = (vault.clone(), programs.clone(), last.clone());
          async move {
            background::run(move || {
              last.read(
                (&root, run),
                selected.0.as_ref(),
                policy_revision,
                (vault.get(), &programs),
              )
            })
            .await
            .map_err(|_| Problem("local_integration.error.io"))?
          }
        },
      )
      .state()
      .get();
    // The poll waits while a read or the page's own work runs: the work reads the view again when it ends.
    let paused = read.is_pending() || self.work.state().get().is_pending();
    if self.reading.get_untracked() != paused {
      self.reading.set(paused);
    }
    if let Some(reason) = read.error {
      return self.read_error(ctx, &task, &reason);
    }
    let Some(shown) = read.data else {
      return match read.error {
        Some(reason) => Element::from(Notice::new("local-read-error", NoticeTone::Danger, ctx.t(reason.0))),
        None => self.pending(ctx),
      };
    };
    if shown.1 != task.project.feed.revision.get() {
      // A project/secret change cannot reuse a previously safe metadata or content projection.
      return self.pending(ctx);
    }
    let view = &shown.0;
    self.keep_shown_listed(view);
    let busy = self.work.state().get().is_pending() || task.jobs.run(run.id()).busy();
    let narrow = ctx.breakpoint() < Some(Breakpoint::Xl);
    let body = self.body(ctx, &task, view, busy, narrow);
    Element::from(
      ScrollVertical::new(body)
        .id("local-integration-scroll")
        .width(Dimension::Pct(100.0))
        .height(Dimension::Pct(100.0))
        .flex(1.0),
    )
  }
}

impl LocalPage {
  fn pending(&self, ctx: &mut Ctx) -> Element {
    // No loading frame draws Local integration: the checkout's facts are lines of bones until they are read.
    let bones = skeleton::lines(ctx, &[360.0, 520.0, 440.0, 300.0]);
    let loading = Skeleton::new("local-loading", ctx.t("local_integration.loading")).show(ctx, bones);
    let state = self.work.state().get();
    if !state.is_pending() && state.error.is_none() && matches!(state.data, Some(Outcome::Aborted)) {
      // Report the verified worker outcome while fresh checkout evidence loads; offer no decisions yet.
      Column::new()
        .spacing(SpacingSize::Md)
        .child(controls::aborted(ctx))
        .child(loading)
        .into()
    } else {
      loading
    }
  }

  fn read_error(&self, ctx: &mut Ctx, task: &TaskContext, reason: &Problem) -> Element {
    let mut body = Column::new().spacing(SpacingSize::Md).child(Notice::new(
      "local-read-error",
      NoticeTone::Danger,
      ctx.t(reason.0),
    ));
    if let Some(refused) = self.work.state().get().error {
      body = body.child(Notice::new(
        "local-work-error",
        NoticeTone::Danger,
        ctx.t(refused.problem.0),
      ));
    }
    if self.resolver.editor.dirty() {
      let source = self.resolver.seeded.lock().clone();
      let request = source.and_then(|(file, _, generation)| {
        generation.map(|generation| work::Request {
          task: task.clone(),
          action: work::Action::SaveDraft {
            file: file.file,
            expected: file.expected,
            candidate: self.resolver.editor.current(),
            generation,
          },
          stop: self.stop.clone(),
          input: None,
          generation: Some(generation),
        })
      });
      body = body.child(draft_controls::inline(
        ctx,
        &self.work,
        &self.resolver,
        request,
        (self.work.state().get().is_pending(), false),
        task,
      ));
    }
    body.into()
  }

  /// The review shows a listed snapshot's diff: the first one until the person picks another, and the first
  /// again once the picked one is no longer listed (committed, preserved or reverted).
  fn keep_shown_listed(&self, view: &IntegrationView) {
    let shown = self.shown_file.get_untracked().0;
    if shown.as_ref().is_some_and(|shown| files::listed(view, shown)) {
      return;
    }
    let first = files::first(view);
    if first != shown {
      self.shown_file.set(ShownFile(first));
    }
  }

  fn body(&self, ctx: &mut Ctx, task: &TaskContext, view: &IntegrationView, busy: bool, narrow: bool) -> Element {
    let run = task
      .detail
      .run
      .as_ref()
      .expect("the mounted local page has a succeeded run");
    let mode = self.mode.get();
    let left = evidence::column(ctx, self, task, view, busy, narrow);
    let side = Column::new()
      .width(Dimension::Px(if narrow {
        NARROW_INSPECTOR_WIDTH
      } else {
        INSPECTOR_WIDTH
      }))
      .flex_shrink(0.0)
      .child(controls::inspector(ctx, self, (task, view), busy, narrow));
    Column::new()
      .id("task-local-integration")
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Lg)
          .left(SpacingSize::Section)
          .right(SpacingSize::Section)
          .bottom(SpacingSize::Section),
      )
      .spacing(SpacingSize::Lg)
      .child(strip::identities(ctx, run.run.branch().as_str(), view))
      .child(strip::trail(ctx, view, &mode))
      .child(
        Row::new()
          .id("local-columns")
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Xl)
          .align_items(Alignment::Start)
          .child(left)
          .child(side),
      )
      .into()
  }
}
