//! The panel of the chosen pipeline (`Detail` `KGBgh` in `Pipelines · v2` P1): its head (what it is and its version
//! over its name), the chips under it (its version, whether it can run or what it lacks, and the tasks that use it),
//! the body ([`super::steps`], the flow; [`super::settings`], the *Pipeline* rows) and, pinned under the scrolling
//! body, the Save bar ([`super::bar`]).
//!
//! The panel edits the pipeline in place ([`super::edit`]): every change to it and its steps is held until *Save*,
//! which stores it and publishes it as the next version ([`super::save`]); *Cancel* puts back what is stored. The panel
//! is keyed by its pipeline, so an edit never carries over to another one; the hub asks before it goes
//! ([`super::leave`]). The head with its ⋯ menu is [`super::panel_head`]; an archived pipeline's panel is
//! [`super::archived`], which has no Save bar.

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
  resource::{Pipeline, PipelineEdit, StepKey},
  time::Timestamp,
};
use orchester_workspace::pipelines::{data::Assignables, flow::FlowRead};

use super::{
  archived,
  bar::{self, Said},
  chips,
  edit::{Edit, Holding},
  leave::{Hosted, Unsaved, UnsavedSignal, clear_unsaved, mark_unsaved},
  model::Entry,
  panel_head::{self, Props as HeadProps},
  ready,
  save::{self, Ask, Asked, Followed, Notes, Pending, Save},
  settings::{self, Settings},
  step, steps,
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
  /// The fields of the open step that show an error: while one does, what is typed is not what the edit holds.
  faulty: step::Faulty,
  /// How many times the edit was put back, after which the open step's fields are mounted anew.
  reset: Signal<u64>,
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
      faulty: ctx.signal(Vec::new()),
      reset: ctx.signal(0),
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
    self.stored.set(Some(stored.clone()));
    self.settle(&stored);
    let edit = Edit::new(&self.holding, stored).following(&self.stored);
    let current = edit.current();
    let changed = edit.is_changed();
    mark_unsaved(&self.unsaved, self.id, changed);
    if changed {
      self.notes.forget_stale(&current);
    }
    if !changed && self.description.get_untracked() != current.description() {
      self.description.set(current.description().to_owned());
    }
    let read = if edit.holds() {
      Arc::new(FlowRead::of_edit(&current, &assignables))
    } else {
      listed
    };
    let asked = self.pending.get();
    let saving = self.save.state().get().is_pending();
    mark_saving(&hosted.saving, saving);
    let locked = saving || asked.is_some();
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
    };
    let body = match self.open_step(ctx, &shown, step) {
      Some(key) => self.step_body(ctx, &shown, &key),
      None => self.pipeline_body(ctx, &shown, hosted),
    };
    let pinned = self.pinned(ctx, &entry, asked.as_ref());
    let bar = bar::bar(
      ctx,
      (
        self.save_now(&edit, (changed, entry.unpublished), saving || asked.is_some()),
        (changed && !saving).then(|| self.cancel_now(&edit)),
      ),
      (&entry, changed, step::fix_words(ctx, &self.faulty, &current)),
    );
    frame(&entry, body, pinned, Some(bar))
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
  read: &'a FlowRead,
  /// A save is under way, or a question about it waits: nothing is changed now.
  locked: bool,
  changed: bool,
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
      changed,
      ..
    } = *shown;
    let head = panel_head::head(
      ctx,
      HeadProps {
        project: project.clone(),
        entry: entry.clone(),
        default,
        now: Timestamp::now(),
        hosted,
        holds_changes: changed,
      },
    );
    if self.reveal_delivers.get_untracked() {
      ctx.scroll_into_view(&self.delivers);
      self.reveal_delivers.set(false);
    }
    body()
      .spacing(ExtraSpacing::Inset.role())
      .child(head)
      .child(chips::chips(ctx, entry, default))
      .with_children(steps::parts(ctx, project, entry, read))
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
      .with_children(ready::group(ctx, project, entry, read))
  }
}

impl Drop for PipelinePanel {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, self.id);
    mark_saving(&self.saving, false);
  }
}

impl PipelinePanel {
  /// After a save the panel shows what it saved until the hub lists another pipeline; then what the hub lists.
  fn settle(&self, stored: &Arc<Pipeline>) {
    let holding = self.holding.get_untracked();
    if holding.saved_over.as_ref().is_some_and(|over| over != stored) {
      self.holding.set(Holding::default());
    }
  }

  /// What is pinned over the bar: the question a save asks, and what the last save said.
  fn pinned(&self, ctx: &mut Ctx, entry: &Entry, asked: Option<&Asked>) -> Vec<Element> {
    let mut pinned: Vec<Element> = Vec::new();
    if let Some(asked) = asked {
      pinned.push(save::question(ctx, asked, (&self.save, &self.pending)));
    }
    let told = self.notes.told();
    match (told.refused, told.kept, told.made) {
      (Some((reason, _)), ..) => pinned.push(bar::notice(ctx, entry, &Said::Refused(&reason))),
      (None, Some(why), _) => pinned.push(bar::notice(ctx, entry, &Said::Kept(&why))),
      (None, None, Some(version)) => pinned.push(bar::notice(ctx, entry, &Said::Made(version))),
      (None, None, None) => {}
    }
    pinned
  }

  /// What *Save* does now: nothing while nothing differs and no draft waits to be published, else it stores and
  /// publishes what the panel holds.
  fn save_now(&self, edit: &Edit, (changed, unpublished): (bool, bool), waiting: bool) -> SaveState {
    if self.save.state().get().is_pending() {
      return SaveState::Saving;
    }
    if !changed && !unpublished {
      return SaveState::Unchanged;
    }
    // Text that is not valid is not in the edit, so saving now would store what the box no longer says.
    if waiting || step::holds_invalid(&self.faulty, &edit.current()) {
      return SaveState::Unchanged;
    }
    let (edit, save, notes) = (edit.clone(), self.save.clone(), self.notes.clone());
    SaveState::Ready(Press::run(move || {
      let held = edit.to_save();
      notes.begin(held.edit());
      save.run(Ask {
        held,
        allowed: Vec::new(),
      });
    }))
  }

  /// *Cancel*: the panel shows what is stored again at once.
  fn cancel_now(&self, edit: &Edit) -> Press {
    let (edit, put_back) = (edit.clone(), self.put_back());
    Press::run(move || {
      edit.cancel();
      put_back();
    })
  }

  /// What dropping the edit puts back besides the edit: the description's box, and what the last save said.
  fn put_back(&self) -> impl Fn() + Send + Sync + use<> {
    let (description, stored, pending, notes, (reset, faulty)) = (
      self.description.clone(),
      self.stored.clone(),
      self.pending.clone(),
      self.notes.clone(),
      (self.reset.clone(), self.faulty.clone()),
    );
    move || {
      if let Some(stored) = stored.get() {
        description.set(stored.description().to_owned());
      }
      pending.set(None);
      notes.forget();
      reset.update(|resets| *resets += 1);
      faulty.set(Vec::new());
    }
  }

  /// What is typed in the description's box is the edit's description, whether the text came from a person or was set;
  /// a box that already says what the edit says changes nothing.
  fn follow_typed(&self, ctx: &mut Ctx) {
    let (holding, stored) = (self.holding.clone(), self.stored.clone());
    ctx.watch(&self.description, move |typed: &String| {
      let Some(stored) = stored.get() else {
        return;
      };
      let edit = Edit::new(&holding, stored);
      if edit.description_now() != *typed {
        edit.change(|current| current.set_description(typed));
      }
    });
  }

  /// The hub's question was answered *Discard changes*: the edit goes with them, whether the panel stays (another tab
  /// that holds this pipeline) or goes.
  fn follow_discard(&self, ctx: &mut Ctx) {
    let (holding, put_back, id) = (self.holding.clone(), self.put_back(), self.id);
    ctx.watch(&self.unsaved, move |unsaved: &Unsaved| {
      let holds = holding.get_untracked().held.is_some_and(|held| held.is_changed());
      if unsaved.0 != Some(id) && holds {
        holding.set(Holding::default());
        put_back();
      }
    });
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
