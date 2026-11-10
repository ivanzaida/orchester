//! The panel of the chosen pipeline (`Detail` `KGBgh` in `Pipelines · v2` P1): its head (what it is and its version
//! over its name), the chips under it (its version, whether it can run or what it lacks, and the tasks that use it),
//! the body ([`super::steps`], the flow; [`super::settings`], the *Pipeline* rows) and, pinned under the scrolling
//! body, the Save bar ([`super::bar`]).
//!
//! The panel edits the pipeline in place ([`super::edit`]): every change to it and its steps is held until *Save*,
//! which stores it and publishes it as the next version ([`super::save`]); *Cancel* puts back what is stored. The panel
//! is keyed by its pipeline, so an edit never carries over to another one; the hub asks before it goes
//! ([`super::leave`]). The head with its ⋯ menu and *Versions* is [`super::panel_head`]; an archived pipeline's panel
//! is [`super::archived`], which has no Save bar.
//!
//! The panel has three views of one pipeline: its own body, a step the route holds open ([`super::step`]) and a
//! published version read-only ([`versions`], [`super::version_view`]). The route is the stronger: a route that names a
//! step closes the version shown. Only the pipeline's own views have the head, and only the version view has no Save
//! bar.

mod saving;
mod versions;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, ScrollVertical},
  core::{ElementRef, Ref, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, PipelineEdit, StepKey, VersionNumber},
  time::Timestamp,
};
use orchester_workspace::pipelines::{data::Assignables, flow::FlowRead, local_edit::unpublishable};

use super::{
  archived, bar, chips,
  edit::{Edit, Holding},
  leave::{Hosted, UnsavedSignal, clear_unsaved},
  model::Entry,
  panel_head::{self, Props as HeadProps},
  ready,
  save::{self, Followed, Notes, Pending, Save},
  settings::{self, Settings},
  step, steps,
  version_view::{Viewed, ViewedSignal},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{PipelinesFilter, PipelinesView, ProjectPage, Route},
  screens::{local_hub::save_bar::Save as SaveState, team_hub::panel::body},
  theme::ExtraSpacing,
  ui::pressable::Press,
};

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  /// What steps can run, which the flow of an edit is drawn with.
  pub assignables: Arc<Assignables>,
  /// Whether New task offers this pipeline first.
  pub default: bool,
  /// What the screen hosts for the panel: the list's word of unsaved changes, and the archive confirmation.
  pub hosted: Hosted,
  /// The step the route holds open: the panel shows it in place of the pipeline's own body.
  pub step: Option<StepKey>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.project,
      &self.entry,
      &self.assignables,
      self.default,
      &self.hosted,
      &self.step,
    ) == (
      &other.project,
      &other.entry,
      &other.assignables,
      other.default,
      &other.hosted,
      &other.step,
    )
  }
}
impl DevtoolsInspectable for Props {}

/// The panel of the pipeline `props.entry`, keyed by it; an archived pipeline's panel has no edit to hold.
pub(super) fn panel(ctx: &mut Ctx, props: Props) -> Element {
  if props.entry.archived.is_some() {
    return archived::panel(
      ctx,
      archived::Props {
        project: props.project,
        entry: props.entry,
        now: Timestamp::now(),
      },
    );
  }
  ctx.mount_keyed::<PipelinePanel>(&format!("pipeline-hub-panel-{}", props.entry.id), props)
}

struct PipelinePanel {
  /// What the panel holds: the edit, or nothing while it shows what is stored.
  holding: Signal<Holding>,
  /// What the description's box holds.
  description: Signal<String>,
  /// Stores and publishes what the panel holds.
  save: Save,
  /// What a save would widen, which waits for the person.
  pending: Pending,
  /// What the last save said over the bar.
  notes: Notes,
  /// The pipeline as the hub lists it now, which the panel keeps current.
  stored: Ref<Option<Arc<Pipeline>>>,
  unsaved: UnsavedSignal,
  /// Whether a save is under way, which the screen's question about leaving reads.
  saving: Signal<bool>,
  /// The published version the panel shows read-only, set by the *Versions* list in the head.
  viewing: ViewedSignal,
  /// Whether the question before *Restore* replaces the edit's changes is open.
  restoring: Signal<bool>,
  /// The fields of the open step that show an error: while one does, what is typed is not what the edit holds.
  faulty: step::Faulty,
  /// How many times the edit was put back, after which the open step's fields are mounted anew.
  reset: Signal<u64>,
  /// The version *Restore* put in the edit, until it is saved or dropped.
  restored: Signal<Option<VersionNumber>>,
  /// The Deliver chip of an open step asked for the *A run delivers* row to be shown once the pipeline's body is.
  reveal_delivers: Signal<bool>,
  /// That row.
  delivers: ElementRef,
  id: ResourceId,
}

impl Component for PipelinePanel {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let (project, id) = (props.project.open.project.id(), props.entry.id);
    let description = props
      .entry
      .body
      .as_ref()
      .map_or_else(String::new, |body| body.description().to_owned());
    let this = Self {
      holding: ctx.signal(Holding::default()),
      description: ctx.signal(description),
      save: save::save(ctx, props.project.open.repo_root.clone(), id),
      pending: ctx.signal(None),
      notes: Notes::new(ctx),
      stored: ctx.create_ref(props.entry.body.clone()),
      unsaved: props.hosted.unsaved,
      saving: props.hosted.saving,
      viewing: ctx.signal(None),
      restoring: ctx.signal(false),
      faulty: ctx.signal(Vec::new()),
      reset: ctx.signal(0),
      restored: ctx.signal(None),
      reveal_delivers: ctx.signal(false),
      delivers: ctx.element_ref(),
      id,
    };
    save::follow(
      ctx,
      &this.save,
      &Followed {
        pending: this.pending.clone(),
        holding: this.holding.clone(),
        notes: this.notes.clone(),
        stored: this.stored.clone(),
        published: (props.hosted.published, id),
      },
      project,
    );
    this.follow_discard(ctx);
    this.follow_typed(ctx);
    this
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props {
      project,
      entry,
      assignables,
      default,
      hosted,
      step,
    } = ctx.props::<Props>().clone();
    // `panel` leaves the pipelines that have no steps to hold, the archived ones, to their own panel.
    let stored = entry
      .body
      .clone()
      .expect("a pipeline that is not archived has its steps");
    let listed = entry
      .flow
      .clone()
      .expect("a pipeline that is not archived has its flow");
    self.settle(&stored);
    let edit = Edit::new(&self.holding, stored).following(&self.stored);
    let current = edit.current();
    step::forget_gone(&self.faulty, &current);
    let changed = edit.is_changed();
    let unsaved = changed || step::holds_invalid(&self.faulty, &current);
    self.follow_edit(&current, (changed, unsaved));
    let read = if edit.holds() {
      Arc::new(FlowRead::of_edit(&current, &assignables))
    } else {
      listed
    };
    let asked = self.pending.get();
    let saving = self.save.state().get().is_pending();
    mark_saving(&hosted.saving, saving);
    let locked = saving || asked.is_some();
    let line = bar::Line {
      entry: &entry,
      unsaved,
      fix: step::fix_words(ctx, &self.faulty, &current),
      unpublishable: unpublishable(&current, &read),
      restored: self.restored.get(),
    };
    let off = line.fix.clone().or_else(|| bar::publish_blocked(ctx, &line));
    let save = self.save_now(&edit, (changed, entry.unpublished), (locked, off));
    let press = match &save {
      SaveState::Ready(press) => Some(press.clone()),
      _ => None,
    };
    let shown = Shown {
      project: &project,
      entry: &entry,
      assignables: &assignables,
      default,
      edit: &edit,
      current: &current,
      read: &read,
      locked,
      changed,
      save: press.as_ref(),
    };
    let (body, viewed) = self.views(ctx, &shown, hosted, step);
    let refused = self.notes.told().refused.is_some();
    let mut pinned = self.pinned(ctx, &entry, asked.as_ref(), ready::can_run(&entry, &read));
    let bar = match &viewed {
      // A version is read, not edited: the bar belongs to the edit, which waits behind it.
      Some(viewed) => {
        pinned.extend(self.replace_question(ctx, &edit, viewed));
        None
      }
      // Cancel drops unsaved changes, and a refusal of a save that changed nothing, which stays until then.
      None => Some(bar::bar(
        ctx,
        (save, ((unsaved || refused) && !saving).then(|| self.cancel_now(&edit))),
        &line,
      )),
    };
    frame(&entry, body, pinned, bar)
  }
}

/// What the panel shows of the pipeline in this draw, for the body of either view.
#[derive(Clone, Copy)]
struct Shown<'a> {
  project: &'a ShownProject,
  entry: &'a Entry,
  assignables: &'a Assignables,
  default: bool,
  edit: &'a Edit,
  /// What the panel shows: the edit, or the stored pipeline.
  current: &'a PipelineEdit,
  read: &'a Arc<FlowRead>,
  /// A save is under way, or a question about it waits: nothing is changed now.
  locked: bool,
  changed: bool,
  /// What *Save* does now, when it can save.
  save: Option<&'a Press>,
}

impl PipelinePanel {
  /// The step the route holds open, when the pipeline has it. A route that names a step the pipeline has not (any
  /// more, as after *Cancel* dropped a copy) shows the pipeline and is corrected to say so.
  fn open_step(&self, ctx: &mut Ctx, shown: &Shown<'_>, named: Option<StepKey>) -> Option<StepKey> {
    let named = named?;
    if shown.current.step(&named).is_some() {
      return Some(named);
    }
    let route = ProjectPage::pipelines(PipelinesView::new(PipelinesFilter::All, Some(shown.entry.id)));
    Navigation::of(ctx).replace(Route::Project(shown.project.open.project.id(), route));
    None
  }

  /// The body of an open step ([`step`]).
  fn step_body(&self, ctx: &mut Ctx, shown: &Shown<'_>, key: &StepKey) -> Column {
    body().spacing(ExtraSpacing::Inset.role()).with_children(step::parts(
      ctx,
      &step::Open {
        project: shown.project,
        entry: shown.entry,
        key,
        edit: shown.edit,
        current: shown.current,
        read: shown.read,
        assignables: shown.assignables,
        locked: shown.locked,
        epoch: self.notes.stored.get() + self.reset.get(),
        faulty: &self.faulty,
        reveal_delivers: &self.reveal_delivers,
      },
    ))
  }

  /// The body the panel shows, and the version in it when it is the version's: a step the route holds open wins over a
  /// version that was shown, which it closes.
  fn views(&self, ctx: &mut Ctx, shown: &Shown<'_>, hosted: Hosted, step: Option<StepKey>) -> (Column, Option<Viewed>) {
    if let Some(key) = self.open_step(ctx, shown, step) {
      self.close_version();
      return (self.step_body(ctx, shown, &key), None);
    }
    match self.viewing.get() {
      Some(viewed) => (self.version_body(ctx, shown, hosted, &viewed), Some(viewed)),
      None => (self.pipeline_body(ctx, shown, hosted), None),
    }
  }

  /// The panel's head: what the pipeline is and its version, *Versions* and ⋯.
  fn head(&self, ctx: &mut Ctx, shown: &Shown<'_>, hosted: Hosted) -> Element {
    panel_head::head(
      ctx,
      HeadProps {
        project: shown.project.clone(),
        entry: shown.entry.clone(),
        default: shown.default,
        now: Timestamp::now(),
        hosted,
        holds_changes: shown.changed,
        viewing: self.viewing.clone(),
      },
    )
  }

  /// The body of the pipeline: its head, chips, flow, settings and *Ready?*.
  fn pipeline_body(&self, ctx: &mut Ctx, shown: &Shown<'_>, hosted: Hosted) -> Column {
    let Shown {
      project,
      entry,
      default,
      edit,
      current,
      read,
      locked,
      save,
      ..
    } = *shown;
    let head = self.head(ctx, shown, hosted);
    if self.reveal_delivers.get_untracked() {
      ctx.scroll_into_view(&self.delivers);
      self.reveal_delivers.set(false);
    }
    body()
      .spacing(ExtraSpacing::Inset.role())
      .child(head)
      .child(chips::chips(ctx, entry, default))
      .with_children(steps::parts(ctx, project, entry, (read, edit, current), locked))
      .child(settings::section(
        ctx,
        &Settings {
          project,
          edit,
          current,
          description: &self.description,
          locked,
          saved: self.notes.stored.get(),
          delivers: &self.delivers,
        },
      ))
      .with_children(ready::group(
        ctx,
        &ready::Shown {
          project,
          entry,
          read,
          current,
          edit,
          locked,
          save,
        },
      ))
  }
}

impl Drop for PipelinePanel {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, self.id);
    mark_saving(&self.saving, false);
  }
}

/// Tells the screen whether a save is under way, so the question about leaving does not offer to drop what is being
/// saved; set only when it differs so the page is not read again for nothing.
fn mark_saving(saving: &Signal<bool>, now: bool) {
  if saving.get_untracked() != now {
    saving.set(now);
  }
}

/// The panel of `entry`: `body` scrolls over what is pinned: what the last save said, and the bar.
fn frame(entry: &Entry, body: Column, pinned: Vec<Element>, bar: Option<Element>) -> Element {
  let scroll = ScrollVertical::new(body)
    .id("pipeline-hub-panel-scroll")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .min_height(Dimension::Px(0.0));
  Column::new()
    .id("pipeline-hub-panel")
    .describe("pipeline", entry.id.to_string())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .child(scroll)
    .with_children(pinned)
    .with_children(bar)
    .into()
}
