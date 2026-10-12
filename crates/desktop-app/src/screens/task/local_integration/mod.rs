//! ORC-24 native local integration (zMOBO): one page, inline decisions, app-local scrolling.

mod controls;
mod draft_controls;
mod fields;
mod files;
mod resolver;
mod selection;
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
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::{integration::local::LocalPhase, run::RunStatus};
use orchester_workspace::{
  background,
  tasks::local_integration::{IntegrationView, Preview},
};
use selection::{Selection, ShownFile};
use work::{Outcome, Problem, Work};

use super::TaskContext;
use crate::{
  services::Services,
  ui::{
    notice::{Notice, NoticeTone},
    state::loading,
  },
};

/// Action inspector of YxF6B / OoP6s; at 960 it remains beside evidence as s4gj9 draws.
const INSPECTOR_WIDTH: f32 = 336.0;
const NARROW_INSPECTOR_WIDTH: f32 = 256.0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
  Review,
  Commit { tree: String, preview: Preview },
  Preserve,
  Abort,
  Cleanup,
  Cancel,
}
impl DevtoolsInspectable for Mode {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shown(Arc<IntegrationView>, u64);
impl DevtoolsInspectable for Shown {}

pub fn tab(ctx: &mut Ctx, task: &TaskContext) -> Element {
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
  ctx.mount_keyed::<LocalPage>("task-local-integration", task.clone())
}

struct LocalPage {
  revision: Signal<u64>,
  reading: Signal<bool>,
  editor_dirty: Signal<bool>,
  resolver: resolver::EditorState,
  acknowledgement: controls::AcknowledgementState,
  selected: Signal<Selection>,
  shown_file: Signal<ShownFile>,
  mode: Signal<Mode>,
  message: Signal<String>,
  name: Signal<String>,
  work: Work,
  stop: Arc<AtomicBool>,
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
    let mode = ctx.signal(Mode::Review);
    let action = work::action(ctx);
    let resolver = resolver::EditorState::new(ctx);
    let completed_editor = resolver.editor.clone();
    let completed_leave = resolver.leave.clone();
    let navigation = crate::navigation::Navigation::of(ctx);
    let (completed_mode, completed_revision) = (mode.clone(), revision.clone());
    ctx.watch(&action.state(), move |state| {
      if !state.is_pending()
        && state.error.is_none()
        && let Some(outcome) = &state.data
      {
        completed_mode.set(match outcome {
          Outcome::CommitReview { tree, preview } => Mode::Commit {
            tree: tree.clone(),
            preview: preview.clone(),
          },
          Outcome::Changed | Outcome::DraftSaved(_) | Outcome::Resolved(_) => Mode::Review,
        });
        completed_revision.update(|value| *value += 1);
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
      selected: ctx.signal(Selection(Vec::new())),
      shown_file: ctx.signal(ShownFile(None)),
      mode,
      message: Signal::new(String::new()),
      name: Signal::new(String::new()),
      work: action,
      stop: Arc::new(AtomicBool::new(false)),
      _poll: poll,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    self.editor_dirty.set(self.resolver.editor.dirty());
    let task = ctx.props::<TaskContext>().clone();
    let Some(run) = &task.detail.run else {
      return Element::from(Column::new());
    };
    let root = task.project.open.repo_root.clone();
    let vault = Services::of(ctx).vault;
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
          let vault = vault.clone();
          async move {
            background::run(move || {
              orchester_workspace::tasks::local_integration::read(&root, run, selected.0.as_ref(), vault.get())
                .map(|view| Shown(Arc::new(view), policy_revision))
                .map_err(work::problem)
            })
            .await
            .map_err(|_| Problem("local_integration.error.io"))?
          }
        },
      )
      .state()
      .get();
    self.reading.set(read.is_pending());
    if let Some(reason) = read.error {
      return self.read_error(ctx, &task, &reason);
    }
    let Some(shown) = read.data else {
      return match read.error {
        Some(reason) => Element::from(Notice::new("local-read-error", NoticeTone::Danger, ctx.t(reason.0))),
        None => loading("local-loading", &ctx.t("local_integration.loading")).into(),
      };
    };
    if shown.1 != task.project.feed.revision.get() {
      // A project/secret change cannot reuse a previously safe metadata or content projection.
      return loading("local-loading", &ctx.t("local_integration.loading")).into();
    }
    let view = &shown.0;
    let busy = self.work.state().get().is_pending() || task.jobs.run(run.id()).busy();
    let narrow = ctx.breakpoint().is_none();
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
  fn read_error(&self, ctx: &mut Ctx, task: &TaskContext, reason: &Problem) -> Element {
    let mut body = Column::new().spacing(SpacingSize::Md).child(Notice::new(
      "local-read-error",
      NoticeTone::Danger,
      ctx.t(reason.0),
    ));
    if let Some(error) = self.work.state().get().error {
      body = body.child(Notice::new("local-work-error", NoticeTone::Danger, ctx.t(error.0)));
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
        })
      });
      body = body.child(draft_controls::inline(
        ctx,
        &self.work,
        &self.resolver,
        request,
        self.work.state().get().is_pending(),
        task,
      ));
    }
    body.into()
  }

  fn body(&self, ctx: &mut Ctx, task: &TaskContext, view: &IntegrationView, busy: bool, narrow: bool) -> Element {
    let run = task
      .detail
      .run
      .as_ref()
      .expect("the mounted local page has a succeeded run");
    let identities = ctx.t_args(
      "local_integration.identities",
      [
        ("source", run.run.branch().as_str()),
        ("target", view.checkout.branch.as_str()),
        ("path", view.checkout.root.as_str()),
        ("head", view.checkout.head.as_str()),
      ],
    );
    let phase = view.operation.as_ref().map_or(LocalPhase::Review, |op| op.phase());
    let mut left = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(SpacingSize::Md)
      .child(
        Notice::new("local-phase", phase_tone(phase), ctx.t(controls::phase_title(phase)))
          .body(ctx.t(controls::phase_body(phase))),
      );
    left = left.with_children(external_notice(ctx, view));
    if let Some(problem) = self.work.state().get().error {
      left = left.child(Notice::new("local-work-error", NoticeTone::Danger, ctx.t(problem.0)));
    }
    if busy {
      left = left.child(loading(
        "local-working",
        &ctx.t(if self.stop.load(std::sync::atomic::Ordering::Acquire) {
          "local_integration.stopping"
        } else {
          "local_integration.busy"
        }),
      ));
    }
    left = if matches!(phase, LocalPhase::TaskConflicts | LocalPhase::RestoreConflicts) || self.editor_dirty.get() {
      left.child(resolver::mount(ctx, self, task, view, busy, narrow))
    } else {
      left.child(files::content(ctx, self, view, busy))
    };
    let side = controls::inspector(ctx, self, task, view, busy).width(Dimension::Px(if narrow {
      NARROW_INSPECTOR_WIDTH
    } else {
      INSPECTOR_WIDTH
    }));
    Column::new()
      .id("task-local-integration")
      .width(Dimension::Pct(100.0))
      .padding(if narrow { SpacingSize::Xl } else { SpacingSize::Section })
      .spacing(SpacingSize::Md)
      .child(
        Text::new(&identities)
          .variant(TypographyStyle::Mono)
          .color(PaletteColor::TextMuted),
      )
      .child(Text::new(&ctx.t("local_integration.phases")).variant(TypographyStyle::Description))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Xl)
          .child(left)
          .child(side),
      )
      .into()
  }
}
fn phase_tone(phase: LocalPhase) -> NoticeTone {
  match phase {
    LocalPhase::Complete | LocalPhase::RestorationVerified => NoticeTone::Success,
    LocalPhase::TaskConflicts => NoticeTone::Danger,
    LocalPhase::Review
    | LocalPhase::RestoreConflicts
    | LocalPhase::MergedAwaitingRestore
    | LocalPhase::Paused
    | LocalPhase::NeedsReconciliation => NoticeTone::Warning,
    _ => NoticeTone::Info,
  }
}

fn external_notice(ctx: &Ctx, view: &IntegrationView) -> Option<Element> {
  let evidence = view.external.as_ref()?;
  Some(
    Notice::new(
      "local-external-integration",
      NoticeTone::Warning,
      ctx.t("local_integration.externalTitle"),
    )
    .body(ctx.t_args(
      words::evidence_body(evidence.kind),
      [
        ("source", evidence.source.as_str()),
        ("target", evidence.target.as_str()),
      ],
    ))
    .into(),
  )
}
