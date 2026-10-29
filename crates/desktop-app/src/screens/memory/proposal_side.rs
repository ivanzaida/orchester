//! A proposal's details beside its review page (`05e Memory · review proposal` `Ybk74`): *Source* — where it was
//! written, with how many transcript events and the conversation it came from, the agent's confidence, and that an edit
//! keeps its author; *Versions* — the earlier versions of the record it would replace. A proposal is given to no agent,
//! so it has no *Given to agents*.

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::{Memory, MemoryAuthor};
use orchester_workspace::memory::{listing, read::ProjectMemory};

use super::{details, opened, parts as memory_parts, state::MemoryState, words};
use crate::{
  screens::hub::parts::{self, Entry},
  theme::HubSpacing,
};

/// *Source*, then *Versions* when the record it would replace has earlier versions.
pub fn side(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, record: &Memory) -> Element {
  Column::new()
    .id("memory-details")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::SideSections.role())
    .child(source(ctx, state, memory, record))
    .with_children(versions(ctx, memory, record))
    .into()
}

fn source(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, record: &Memory) -> Element {
  let mut rows: Vec<Element> = Vec::new();
  if let Some(place) = memory.origins.written(record) {
    let agent = place.agent.clone().unwrap_or_default();
    let place = details::where_worked(ctx, place);
    let mut written = Entry::new(
      "memory-details-written",
      ctx.t_args("memory.inspector.writtenIn", [("place", &*place)]),
    );
    written = match record.sources().len() {
      0 => written,
      1 => written.hint(ctx.t_args("memory.inspector.eventsOne", [("place", &*place)])),
      count => written.hint(ctx.t_args(
        "memory.inspector.eventsOther",
        [("count", count.to_string().as_str()), ("place", &*place)],
      )),
    };
    if let (MemoryAuthor::Agent(_), Some(task)) = (record.author(), opened::source_task(record, &memory.origins)) {
      written = written.end(memory_parts::conversation_button(
        ctx,
        (state.project_id, &task),
        "memory-details-conversation",
      ));
    }
    rows.push(written.into());
    rows.push(
      Entry::new("memory-details-confidence", ctx.t("memory.inspector.agentConfidence"))
        .hint(ctx.t_args(
          "memory.inspector.confidenceKept",
          [
            ("confidence", &*words::confidence_title(ctx, record.confidence())),
            ("agent", &*agent),
          ],
        ))
        .into(),
    );
  }
  rows.push(
    Entry::new("memory-details-author", ctx.t("memory.inspector.author"))
      .hint(ctx.t("memory.inspector.authorKept"))
      .into(),
  );
  let id = "memory-details-source-section";
  parts::group(
    id,
    parts::head(
      "memory-details-source",
      &ctx.t("memory.inspector.proposalSource"),
      None,
      None,
    ),
    parts::boxed(&format!("{id}-rows"), rows),
  )
  .into()
}

/// *Versions*: how many earlier versions the replaced record has, and the newest of them; `None` without any.
fn versions(ctx: &Ctx, memory: &ProjectMemory, record: &Memory) -> Option<Element> {
  let replaced = memory.records.iter().find(|r| Some(r.id()) == record.supersedes())?;
  let earlier = listing::earlier_versions(replaced, &memory.records);
  let newest = earlier.first()?;
  let title = match earlier.len() {
    1 => ctx.t("memory.editor.versionsOne"),
    count => ctx.t_args("memory.editor.versionsOther", [("count", count.to_string())]),
  };
  let hint = ctx.t_args(
    "memory.inspector.versionNewest",
    [
      ("version", earlier.len().to_string()),
      ("time", words::when(ctx, newest.updated_at())),
    ],
  );
  let id = "memory-details-versions";
  Some(
    parts::group(
      id,
      parts::head(id, &ctx.t("memory.inspector.versions"), None, None),
      parts::boxed(
        &format!("{id}-rows"),
        vec![Entry::new("memory-details-versions-latest", title).hint(hint).into()],
      ),
    )
    .into(),
  )
}
