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
    theme::SpacingSize,
  },
  components::{Column, Row, ScrollVertical, Text, TextOverflow},
  core::{Ref, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ResourceId, resource::Pipeline, time::Timestamp};
use orchester_workspace::{
  messages::Message,
  pipelines::{data::Assignables, flow::FlowRead},
};

use super::{
  archived,
  bar::{self, Said},
  edit::{Edit, Holding},
  leave::{Hosted, Unsaved, UnsavedSignal, clear_unsaved, mark_unsaved},
  model::Entry,
  panel_head::{self, Props as HeadProps},
  save::{self, Ask, Asked, Followed, Kept, Pending, Save},
  settings::{self, Settings},
  steps, words,
};
use crate::{
  project::ShownProject,
  screens::{local_hub::save_bar::Save as SaveState, team_hub::panel::body},
  theme::{ExtraSpacing, HubText},
  ui::{
    chip::{BadgeTone, badge},
    pressable::Press,
  },
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
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.project,
      &self.entry,
      &self.assignables,
      self.default,
      &self.hosted,
    ) == (
      &other.project,
      &other.entry,
      &other.assignables,
      other.default,
      &other.hosted,
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
  /// The draft a save kept because it could not publish, and why.
  kept: Kept,
  /// The pipeline as the hub lists it now, which the panel keeps current.
  stored: Ref<Option<Arc<Pipeline>>>,
  unsaved: UnsavedSignal,
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
      kept: ctx.signal(None),
      stored: ctx.create_ref(props.entry.body.clone()),
      unsaved: props.hosted.unsaved,
      id,
    };
    save::follow(
      ctx,
      &this.save,
      &Followed {
        pending: this.pending.clone(),
        holding: this.holding.clone(),
        kept: this.kept.clone(),
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
    let edit = Edit::new(&self.holding, stored);
    let current = edit.current();
    let changed = edit.is_changed();
    mark_unsaved(&self.unsaved, self.id, changed);
    if changed {
      self.forget_what_was_said();
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
    let locked = saving || asked.is_some();
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
    let body = body()
      .spacing(ExtraSpacing::Inset.role())
      .child(head)
      .child(chips(ctx, &entry, default))
      .with_children(steps::parts(ctx, &project, &entry, &read))
      .child(settings::section(
        ctx,
        &Settings {
          project: &project,
          edit: &edit,
          current: &current,
          description: &self.description,
          locked,
        },
      ));
    let pinned = self.pinned(ctx, &entry, asked.as_ref());
    let bar = bar::bar(
      ctx,
      (
        self.save_now(&edit, (changed, entry.unpublished), saving || asked.is_some()),
        (changed && !saving).then(|| self.cancel_now(&edit)),
      ),
      &entry,
    );
    frame(&entry, body, pinned, Some(bar))
  }
}

impl Drop for PipelinePanel {
  fn drop(&mut self) {
    clear_unsaved(&self.unsaved, self.id);
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
    match (self.refusal(), self.kept.get()) {
      (Some(reason), _) => pinned.push(bar::notice(ctx, entry, &Said::Refused(&reason))),
      (None, Some(why)) => pinned.push(bar::notice(ctx, entry, &Said::Kept(&why))),
      (None, None) => {}
    }
    pinned
  }

  /// A change makes what the last save said stale: the draft it kept, and the question it asked.
  fn forget_what_was_said(&self) {
    if self.kept.get_untracked().is_some() {
      self.kept.set(None);
    }
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
    if waiting {
      return SaveState::Unchanged;
    }
    let (edit, save, kept) = (edit.clone(), self.save.clone(), self.kept.clone());
    SaveState::Ready(Press::run(move || {
      kept.set(None);
      save.run(Ask {
        held: edit.to_save(),
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
    let (description, stored, pending, kept) = (
      self.description.clone(),
      self.stored.clone(),
      self.pending.clone(),
      self.kept.clone(),
    );
    move || {
      if let Some(stored) = stored.get() {
        description.set(stored.description().to_owned());
      }
      pending.set(None);
      kept.set(None);
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

  /// Why the last save did nothing, as the workspace said.
  fn refusal(&self) -> Option<Message> {
    let save = self.save.state().get();
    save.error.clone().filter(|_| save.is_rejected())
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

/// The chips under the head, then *used by 13 tasks* at their end; it ends in an ellipsis when it runs out of room.
pub(super) fn chips(ctx: &Ctx, entry: &Entry, default: bool) -> Row {
  let used = entry.archived.is_none().then(|| words::used_by(ctx, entry));
  Row::new()
    .id("pipeline-hub-chips")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .with_children(
      standing(ctx, entry, default)
        .into_iter()
        .enumerate()
        .map(|(index, (words, tone))| badge(&format!("pipeline-hub-chip-{index}"), tone, &words)),
    )
    .child(Row::new().flex(1.0))
    .with_children(used.map(|used| {
      Text::new(&used)
        .id("pipeline-hub-chips-end")
        .variant(HubText::PanelHint.role())
        .nowrap()
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis)
    }))
}

/// How the pipeline stands, as the chips: its version, then that it is ready or what it lacks; for a draft that it is
/// one, then its first problem when it has one; for an archived one that it is; last, that New task offers it first.
fn standing(ctx: &Ctx, entry: &Entry, default: bool) -> Vec<(Arc<str>, BadgeTone)> {
  let version = entry
    .number
    .map(|number| (Arc::from(number.to_string()), BadgeTone::Neutral));
  let rest = if entry.archived.is_some() {
    vec![(ctx.t("pipeline_hub.chip.archived"), BadgeTone::Neutral)]
  } else if entry.is_draft() {
    let problem = words::needs(ctx, entry).map(|words| (words, BadgeTone::Warning));
    std::iter::once((ctx.t("pipeline_hub.chip.draft"), BadgeTone::Info))
      .chain(problem)
      .collect()
  } else {
    vec![match words::needs(ctx, entry) {
      Some(needs) => (needs, BadgeTone::Warning),
      None => (ctx.t("pipeline_hub.chip.ready"), BadgeTone::Success),
    }]
  };
  let default = default.then(|| (ctx.t("pipeline_hub.chip.default"), BadgeTone::Info));
  version.into_iter().chain(rest).chain(default).collect()
}
