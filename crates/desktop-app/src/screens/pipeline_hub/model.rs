//! What the hub's list shows: the project's pipelines grouped by whether they can run as they are (*Needs you*,
//! *Ready*), the drafts, what was archived apart, and the one the panel shows.

use std::sync::Arc;

use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, VersionNumber},
  time::Timestamp,
};
use orchester_workspace::{
  pipelines::{data::Assignables, flow::FlowRead, hub::PipelineHub, ready::UndecidedMember},
  readiness::{Group, ReadinessStanding},
};

use crate::routes::{PipelinesFilter, PipelinesView};

/// One row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Entry {
  pub id: ResourceId,
  pub name: Arc<str>,
  pub step_count: usize,
  pub used_by: usize,
  /// The latest published version; `None` only for an archived pipeline that never had one.
  pub number: Option<VersionNumber>,
  pub published_at: Option<Timestamp>,
  pub group: Group,
  pub standing: Option<ReadinessStanding>,
  /// Every profile or tool its latest version runs that cannot be told to be ready; empty when it can be told.
  pub undecided: Vec<UndecidedMember>,
  /// When it was archived; `None` while it is not.
  pub archived: Option<Timestamp>,
  /// Its steps as the panel draws them; none for an archived pipeline, which has no steps to open.
  pub flow: Option<Arc<FlowRead>>,
  /// The steps the flow was read from, which an edit of the panel starts from; none for an archived pipeline.
  pub body: Option<Arc<Pipeline>>,
  /// The latest published version's steps, which the list line and the chips speak of; `None` while it has none.
  pub published: Option<Arc<FlowRead>>,
}

impl Entry {
  /// Whether it was never published.
  pub(super) const fn is_draft(&self) -> bool {
    self.number.is_none()
  }

  /// Whether the filter holds it: an archived one only under Archived, a draft under Drafts, the others under All.
  fn held_by(&self, filter: PipelinesFilter) -> bool {
    match filter {
      PipelinesFilter::Archived => self.archived.is_some(),
      _ if self.archived.is_some() => false,
      PipelinesFilter::All => true,
      PipelinesFilter::Drafts => self.is_draft(),
    }
  }
}

/// The groups' order in the list.
pub(super) const GROUPS: [Group; 2] = [Group::NeedsYouHere, Group::Ready];

/// Every entry of the hub, and how many each filter holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Listing {
  entries: Vec<Entry>,
  /// What the steps of the pipelines can run, which the flow of an edit is drawn with.
  assignables: Arc<Assignables>,
}

impl Listing {
  /// Every entry, in the order of the list: by group, then name; the archived ones by when, the latest first.
  pub(super) fn of(hub: &PipelineHub) -> Self {
    let mut entries: Vec<Entry> = hub
      .pipelines()
      .iter()
      .map(|found| Entry {
        id: found.id,
        name: found.name.clone(),
        step_count: found.step_count,
        used_by: found.used_by,
        number: Some(found.version),
        published_at: Some(found.published_at),
        group: found.group(),
        standing: found.standing.clone(),
        undecided: found.undecided.clone(),
        archived: None,
        flow: Some(found.flow.clone()),
        body: Some(found.body.clone()),
        published: Some(found.flow.clone()),
      })
      .collect();
    entries.sort_by_cached_key(|entry| (group_rank(entry.group), entry.name.to_lowercase()));
    entries.extend(hub.archived().iter().map(|found| Entry {
      id: found.id,
      name: found.name.clone(),
      step_count: 0,
      used_by: 0,
      number: found.latest.map(|(number, _)| number),
      published_at: found.latest.map(|(_, at)| at),
      group: Group::Ready,
      standing: None,
      undecided: Vec::new(),
      archived: Some(found.at),
      flow: None,
      body: None,
      published: None,
    }));
    Self {
      entries,
      assignables: hub.assignables(),
    }
  }

  /// How many entries `filter` holds.
  pub(super) fn count(&self, filter: PipelinesFilter) -> usize {
    self.shown(filter).count()
  }

  pub(super) fn shown(&self, filter: PipelinesFilter) -> impl Iterator<Item = &Entry> {
    self.entries.iter().filter(move |entry| entry.held_by(filter))
  }

  /// The entries of `group` that `filter` holds.
  pub(super) fn group(&self, filter: PipelinesFilter, group: Group) -> impl Iterator<Item = &Entry> {
    self.shown(filter).filter(move |entry| entry.group == group)
  }

  /// The entry the panel shows: the one the route names while the filter holds it, else the first of the list.
  pub(super) fn selected(&self, view: PipelinesView) -> Option<&Entry> {
    view
      .item
      .and_then(|id| self.shown(view.filter).find(|entry| entry.id == id))
      .or_else(|| self.shown(view.filter).next())
  }

  /// What the steps of the pipelines can run.
  pub(super) const fn assignables(&self) -> &Arc<Assignables> {
    &self.assignables
  }

  /// The name of the entry `id`, in the list or among the archived.
  pub(super) fn name_of(&self, id: ResourceId) -> Option<&str> {
    self
      .entries
      .iter()
      .find(|entry| entry.id == id)
      .map(|entry| &*entry.name)
  }

  /// Whether `filter` holds the entry `id`.
  pub(super) fn holds(&self, filter: PipelinesFilter, id: ResourceId) -> bool {
    self.shown(filter).any(|entry| entry.id == id)
  }
}

const fn group_rank(group: Group) -> u8 {
  match group {
    Group::NeedsYouHere => 0,
    Group::Ready | Group::OffHere => 1,
  }
}
