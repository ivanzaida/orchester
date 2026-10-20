//! How the Memory screens list records (`docs/design/orchester-workspace.md`, "Memory screens"): filtered by
//! type, status, author and tag, searched in their text, keys and tags, and grouped by type in a fixed order with
//! the records waiting for approval first. Within a group pinned records come first, then the oldest.

use orchester_core::{
  id::MemoryId,
  memory::{Memory, MemoryAuthor, MemoryKind, MemoryScope, MemoryStatus},
};

/// Which statuses a listing shows (the *Status* filter): the task's Memory tab shows the active and the
/// waiting records at first, the project's page the active ones (its queue holds the waiting).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StatusFilter {
  #[default]
  ActiveAndWaiting,
  Active,
  Waiting,
  Deprecated,
  Superseded,
  Rejected,
  Withdrawn,
  All,
}

impl StatusFilter {
  pub const ALL: [Self; 8] = [
    Self::ActiveAndWaiting,
    Self::Active,
    Self::Waiting,
    Self::Deprecated,
    Self::Superseded,
    Self::Rejected,
    Self::Withdrawn,
    Self::All,
  ];

  pub fn shows(self, status: MemoryStatus) -> bool {
    match self {
      Self::ActiveAndWaiting => matches!(status, MemoryStatus::Active | MemoryStatus::Proposed),
      Self::Active => status == MemoryStatus::Active,
      Self::Waiting => status == MemoryStatus::Proposed,
      Self::Deprecated => status == MemoryStatus::Deprecated,
      Self::Superseded => status == MemoryStatus::Superseded,
      Self::Rejected => status == MemoryStatus::Rejected,
      Self::Withdrawn => status == MemoryStatus::Withdrawn,
      Self::All => true,
    }
  }
}

/// Whose records a listing shows (the *Author* filter).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AuthorFilter {
  #[default]
  Any,
  Agents,
  People,
}

impl AuthorFilter {
  pub const ALL: [Self; 3] = [Self::Any, Self::Agents, Self::People];

  pub const fn shows(self, author: &MemoryAuthor) -> bool {
    match self {
      Self::Any => true,
      Self::Agents => matches!(author, MemoryAuthor::Agent(_)),
      Self::People => matches!(author, MemoryAuthor::Person),
    }
  }
}

/// What a listing is narrowed to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Filters {
  /// One type, or every type.
  pub kind: Option<MemoryKind>,
  pub status: StatusFilter,
  pub author: AuthorFilter,
  /// One tag, or any.
  pub tag: Option<String>,
  /// Words each record must hold in its text, key or tags; empty for no search.
  pub search: String,
}

impl Filters {
  /// The project page's at first: active records.
  pub fn project() -> Self {
    Self {
      status: StatusFilter::Active,
      ..Self::default()
    }
  }

  /// Whether `memory` passes every filter.
  pub fn shows(&self, memory: &Memory) -> bool {
    self.kind.is_none_or(|kind| memory.content().kind() == kind) && self.shows_besides_kind(memory)
  }

  /// Whether `memory` passes every filter but the type: a search that finds nothing of the chosen type says how
  /// many records of other types it found.
  pub fn shows_besides_kind(&self, memory: &Memory) -> bool {
    self.status.shows(memory.status())
      && self.author.shows(memory.author())
      && self
        .tag
        .as_deref()
        .is_none_or(|tag| memory.content().tags().contains(tag))
      && found(memory, &self.search)
  }

  /// Whether a search, a type, an author or a tag narrows the listing beyond its statuses.
  pub fn narrowed(&self) -> bool {
    self.kind.is_some() || self.author != AuthorFilter::Any || self.tag.is_some() || !self.search.trim().is_empty()
  }
}

/// Whether every word of `search` is in `memory`'s text, key or tags, regardless of case: parts of words count,
/// as the design's search (lexical, no synonyms) promises.
pub fn found(memory: &Memory, search: &str) -> bool {
  let content = memory.content();
  let haystack = format!(
    "{}\n{}\n{}",
    content.text(),
    content.key().map_or("", |key| key.as_str()),
    content.tags().joined()
  )
  .to_lowercase();
  search
    .split_whitespace()
    .all(|word| haystack.contains(&word.to_lowercase()))
}

/// A group of a listing: the records waiting for approval, or one type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
  Waiting,
  Kind(MemoryKind),
}

/// The order of the groups of one type.
pub const KIND_ORDER: [MemoryKind; 8] = [
  MemoryKind::Handoff,
  MemoryKind::Decision,
  MemoryKind::Warning,
  MemoryKind::Fact,
  MemoryKind::Todo,
  MemoryKind::Summary,
  MemoryKind::Observation,
  MemoryKind::Artifact,
];

/// A group of records as listed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed<'a> {
  pub group: Group,
  pub records: Vec<&'a Memory>,
}

/// The records of `records` that `filters` show, grouped: the waiting ones first when `waiting_first`, then one
/// group per type in [`KIND_ORDER`]; empty groups are left out.
pub fn grouped<'a>(records: &'a [Memory], filters: &Filters, waiting_first: bool) -> Vec<Listed<'a>> {
  let mut shown: Vec<&Memory> = records.iter().filter(|memory| filters.shows(memory)).collect();
  shown.sort_by_key(|memory| (!memory.is_pinned(), memory.created_at()));
  let waiting = |memory: &&Memory| waiting_first && memory.status() == MemoryStatus::Proposed;
  let mut groups = Vec::new();
  let proposals: Vec<&Memory> = shown.iter().copied().filter(waiting).collect();
  if !proposals.is_empty() {
    groups.push(Listed {
      group: Group::Waiting,
      records: proposals,
    });
  }
  for kind in KIND_ORDER {
    let of_kind: Vec<&Memory> = shown
      .iter()
      .copied()
      .filter(|memory| !waiting(memory) && memory.content().kind() == kind)
      .collect();
    if !of_kind.is_empty() {
      groups.push(Listed {
        group: Group::Kind(kind),
        records: of_kind,
      });
    }
  }
  groups
}

/// How many records `filters` would show of types other than the chosen one; zero without a chosen type.
pub fn found_elsewhere(records: &[Memory], filters: &Filters) -> usize {
  let Some(kind) = filters.kind else {
    return 0;
  };
  records
    .iter()
    .filter(|memory| memory.content().kind() != kind && filters.shows_besides_kind(memory))
    .count()
}

/// How many records of one type there are under the listing's other filters but the search: the
/// *0 of 3 decisions* beside a search that found none.
pub fn of_kind(records: &[Memory], filters: &Filters) -> usize {
  let unsearched = Filters {
    search: String::new(),
    ..filters.clone()
  };
  records.iter().filter(|memory| unsearched.shows(memory)).count()
}

/// The records of a scope by status, as a toolbar and the project's panel count them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
  pub active: usize,
  pub waiting: usize,
  pub superseded: usize,
  pub deprecated: usize,
  pub rejected: usize,
  pub withdrawn: usize,
}

impl Counts {
  pub fn of<'a>(records: impl IntoIterator<Item = &'a Memory>) -> Self {
    let mut counts = Self::default();
    for memory in records {
      let count = match memory.status() {
        MemoryStatus::Active => &mut counts.active,
        MemoryStatus::Proposed => &mut counts.waiting,
        MemoryStatus::Superseded => &mut counts.superseded,
        MemoryStatus::Deprecated => &mut counts.deprecated,
        MemoryStatus::Rejected => &mut counts.rejected,
        MemoryStatus::Withdrawn => &mut counts.withdrawn,
      };
      *count += 1;
    }
    counts
  }

  /// Records that no agent is given: neither active nor waiting.
  pub const fn not_given(self) -> usize {
    self.superseded + self.deprecated + self.rejected + self.withdrawn
  }

  pub const fn total(self) -> usize {
    self.active + self.waiting + self.not_given()
  }
}

/// The task's latest active handoff: the one every agent of the task is given ([`orchester_core::memory::recall`]
/// takes the newest).
pub fn latest_handoff(records: &[Memory]) -> Option<MemoryId> {
  records
    .iter()
    .filter(|memory| {
      memory.content().kind() == MemoryKind::Handoff
        && memory.status() == MemoryStatus::Active
        && matches!(memory.scope(), MemoryScope::Task(_))
    })
    .max_by_key(|memory| memory.created_at())
    .map(Memory::id)
}

/// The earlier versions of `memory` among `records`, newest first: the record it replaced once it was active,
/// and that one's, and so on.
pub fn earlier_versions<'a>(memory: &Memory, records: &'a [Memory]) -> Vec<&'a Memory> {
  let mut versions = Vec::new();
  let mut next = memory.earlier_version();
  while let Some(id) = next {
    let Some(earlier) = records.iter().find(|record| record.id() == id) else {
      break;
    };
    if versions.iter().any(|seen: &&Memory| seen.id() == id) {
      break;
    }
    versions.push(earlier);
    next = earlier.earlier_version();
  }
  versions
}

/// The record that superseded `memory`, among `records`, if one did.
pub fn superseded_by<'a>(memory: &Memory, records: &'a [Memory]) -> Option<&'a Memory> {
  if memory.status() != MemoryStatus::Superseded {
    return None;
  }
  records
    .iter()
    .find(|record| record.earlier_version() == Some(memory.id()))
}

/// The tags of `records`, sorted, each once: the *Tag* filter's options.
pub fn tags(records: &[Memory]) -> Vec<String> {
  let mut tags: Vec<String> = records
    .iter()
    .flat_map(|memory| memory.content().tags().iter().map(str::to_owned))
    .collect();
  tags.sort();
  tags.dedup();
  tags
}
