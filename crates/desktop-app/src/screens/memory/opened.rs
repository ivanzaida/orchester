//! What an editor or a delete confirmation open in a record's place is given: who wrote the record and where (the
//! subtitle), the record approving replaces, and the facts the person does not change.

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::memory::{Memory, MemoryAuthor, MemoryScope, MemoryStatus};
use orchester_workspace::memory::{listing, origin::Origins, read::ProjectMemory};

use super::{
  delete::DeleteProps,
  editor::{EditorMode, EditorProps, Replaced},
  words,
};

/// The records and their origins an editor is opened among.
#[derive(Clone, Copy)]
pub struct Among<'a> {
  pub records: &'a [Memory],
  pub origins: &'a Origins,
  /// The meta line names the task an agent worked on.
  pub with_task: bool,
  /// The project's memory as its page read it, which knows how many invocations were given a replaced record.
  pub project: Option<&'a ProjectMemory>,
}

/// The editor of `memory` in `mode`.
pub fn editor(ctx: &Ctx, memory: &Memory, mode: EditorMode, among: Among<'_>) -> EditorProps {
  let replaced = (mode == EditorMode::Review)
    .then(|| replaced(memory, among.records))
    .flatten()
    .map(|old| Replaced {
      memory: old.clone(),
      meta: words::meta(ctx, old, among.origins, among.with_task),
      proposer: among.origins.written(memory).and_then(|place| place.agent),
      given: among.project.and_then(|project| project.given_count(old.id())),
    });
  let facts = match mode {
    EditorMode::Review => review_facts(ctx, memory, among.origins),
    EditorMode::Edit => edit_facts(ctx, memory, among),
    EditorMode::New => Vec::new(),
  };
  EditorProps {
    mode,
    memory: Some(memory.clone()),
    subtitle: subtitle(ctx, memory, mode, among.origins),
    replaced,
    facts: facts.into(),
    source_task: source_task(memory, among.origins).filter(|_| mode == EditorMode::Review),
  }
}

/// *New record*'s editor.
pub fn new_record(ctx: &Ctx, project: &str) -> EditorProps {
  EditorProps {
    mode: EditorMode::New,
    memory: None,
    subtitle: ctx.t_args("memory.editor.newSubtitle", [("project", project)]),
    replaced: None,
    facts: Arc::from([]),
    source_task: None,
  }
}

/// *Delete this record?* of `memory`.
pub fn delete(memory: &Memory, among: Among<'_>) -> DeleteProps {
  DeleteProps {
    memory: memory.clone(),
    earlier: listing::earlier_versions(memory, among.records).len(),
  }
}

/// The active record `memory`, a proposal, would replace.
fn replaced<'a>(memory: &Memory, records: &'a [Memory]) -> Option<&'a Memory> {
  records
    .iter()
    .find(|record| Some(record.id()) == memory.supersedes() && record.status() == MemoryStatus::Active)
}

/// Whose memory it is and who wrote it where (`cEtf8` in `t0jt2k`, `JO3uC` in `MO5Mu`).
fn subtitle(ctx: &Ctx, memory: &Memory, mode: EditorMode, origins: &Origins) -> Arc<str> {
  let mut parts = vec![words::scope(ctx, memory, origins).to_string()];
  match origins.written(memory) {
    Some(mut place) => {
      if mode == EditorMode::Review {
        let agent = place.agent.take().unwrap_or_default();
        parts.push(ctx.t_args("memory.editor.proposedBy", [("agent", &*agent)]).to_string());
      }
      parts.extend(words::workplace(ctx, &place, false));
    }
    None => parts.push(ctx.t("memory.meta.you").to_string()),
  }
  parts.push(words::when(ctx, memory.created_at()));
  words::join(ctx, &parts)
}

/// *Source* and *Agent's confidence* of a proposal.
fn review_facts(ctx: &Ctx, memory: &Memory, origins: &Origins) -> Vec<(Arc<str>, Arc<str>)> {
  let Some(ReviewSource { source, confidence, .. }) = review_source(ctx, memory, origins) else {
    return Vec::new();
  };
  vec![
    (ctx.t("memory.editor.source"), source),
    (ctx.t("memory.editor.confidence"), confidence),
  ]
}

/// Where a proposal was written and what an agent said of it, as the review page's *Source* says it (`H2OVV` in 05e).
pub struct ReviewSource {
  /// *Written in 03 Implement*.
  pub written: Arc<str>,
  /// *1 event in 03 Implement*.
  pub source: Arc<str>,
  /// *high · set by Builder; people don't change it*.
  pub confidence: Arc<str>,
}

/// The [`ReviewSource`] of `memory`; `None` when no agent wrote it.
pub fn review_source(ctx: &Ctx, memory: &Memory, origins: &Origins) -> Option<ReviewSource> {
  let place = origins.written(memory)?;
  let step = place
    .step
    .as_ref()
    .map(|(number, name)| words::step(ctx, *number, name))
    .unwrap_or_default();
  let written = ctx.t_args("memory.editor.sourceStep", [("step", &*step)]);
  let source = match memory.sources().len() {
    0 => written.clone(),
    1 => ctx.t_args("memory.editor.sourceEvent", [("step", &*step)]),
    count => ctx.t_args(
      "memory.editor.sourceEvents",
      [("count", count.to_string().as_str()), ("step", &*step)],
    ),
  };
  let agent = place.agent.unwrap_or_default();
  let confidence = ctx.t_args(
    "memory.editor.confidenceValue",
    [
      ("confidence", &*words::confidence_title(ctx, memory.confidence())),
      ("agent", &*agent),
    ],
  );
  Some(ReviewSource {
    written,
    source,
    confidence,
  })
}

/// *Author* and *Versions* of a record being edited.
fn edit_facts(ctx: &Ctx, memory: &Memory, among: Among<'_>) -> Vec<(Arc<str>, Arc<str>)> {
  let mut author = match among.origins.written(memory) {
    Some(place) => words::workplace(ctx, &place, among.with_task),
    None => vec![ctx.t("memory.meta.you").to_string()],
  };
  author.push(words::when(ctx, memory.created_at()));
  if matches!(memory.author(), MemoryAuthor::Agent(_)) {
    author.push(ctx.t("memory.editor.authorKept").to_string());
  }
  let versions = match listing::earlier_versions(memory, among.records).len() {
    0 => ctx.t("memory.editor.versionsNone"),
    1 => ctx.t("memory.editor.versionsOne"),
    count => ctx.t_args("memory.editor.versionsOther", [("count", count.to_string())]),
  };
  vec![
    (ctx.t("memory.editor.author"), words::join(ctx, &author)),
    (ctx.t("memory.editor.versions"), versions),
  ]
}

/// The task an agent's record came from: its own task, or the task of the run that wrote a project's record.
pub fn source_task(memory: &Memory, origins: &Origins) -> Option<orchester_core::project::TaskKey> {
  if let MemoryScope::Task(task) = memory.scope()
    && let Some(key) = origins.task_key(task)
  {
    return Some(key.clone());
  }
  origins.written(memory).and_then(|place| place.task)
}
