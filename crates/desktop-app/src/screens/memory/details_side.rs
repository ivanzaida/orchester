//! A record's details beside its page (`Side` `Sajww` in 05c): *Author and source* — who wrote it, where, with the
//! conversation it came from, and the agent's confidence; *History* — the person's edit and the earlier versions;
//! *Given to agents* — the invocations that were given it, the latest two, then *Show all N invocations*. Each is a
//! group of rows ([`crate::screens::hub::parts`]).

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::memory::{Memory, MemoryAuthor, MemoryStatus};
use orchester_workspace::{
  memory::{
    listing,
    read::{GivenTo, ProjectMemory},
  },
  messages::Message,
};

use super::{details, opened, parts as memory_parts, state::MemoryState, words};
use crate::{
  screens::hub::parts::{self, Entry},
  theme::HubSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    message::message_text,
  },
};

/// The invocations listed before *Show all N invocations* (`cRxBr`).
const GIVEN_SHOWN: usize = 2;

/// The three groups beside the record's page.
pub fn side(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, record): (&ProjectMemory, &Memory),
  (given, all): (Result<Arc<GivenTo>, Option<Message>>, &Signal<bool>),
) -> Element {
  Column::new()
    .id("memory-details")
    .width(Dimension::Pct(100.0))
    .spacing(HubSpacing::SideSections.role())
    .child(source(ctx, state, memory, record))
    .with_children(history(ctx, memory, record))
    .child(given_group(ctx, given, all))
    .into()
}

fn source(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, record: &Memory) -> Element {
  let mut rows: Vec<Element> = Vec::new();
  match memory.origins.written(record) {
    Some(place) => {
      let agent = place.agent.clone().unwrap_or_default();
      rows.push(
        Entry::new("memory-details-author", ctx.t("memory.inspector.author"))
          .hint(ctx.t_args("memory.inspector.agent", [("agent", &*agent)]))
          .into(),
      );
      let mut source =
        Entry::new("memory-details-source", ctx.t("memory.editor.source")).hint(details::where_worked(ctx, place));
      if let (MemoryAuthor::Agent(_), Some(task)) = (record.author(), opened::source_task(record, &memory.origins)) {
        let words = ctx.t_args("memory.inspector.showEntries", [("task", task.to_string())]);
        source = source.end(memory_parts::conversation_link(
          ctx,
          (state.project_id, &task),
          ("memory-details-conversation", &words),
        ));
      }
      rows.push(source.into());
      if !record.sources().is_empty() {
        rows.push(
          Entry::new("memory-details-transcript", ctx.t("memory.inspector.transcript"))
            .hint(ctx.t_args(
              "memory.inspector.entries",
              [("count", record.sources().len().to_string())],
            ))
            .into(),
        );
      }
      rows.push(
        Entry::new("memory-details-confidence", ctx.t("memory.inspector.confidence"))
          .hint(ctx.t_args(
            "memory.inspector.confidenceValue",
            [
              ("confidence", &*words::confidence_title(ctx, record.confidence())),
              ("agent", &*agent),
            ],
          ))
          .into(),
      );
    }
    None => rows.push(
      Entry::new("memory-details-author", ctx.t("memory.inspector.author"))
        .hint(ctx.t("memory.inspector.person"))
        .into(),
    ),
  }
  let decided = match record.status() {
    MemoryStatus::Deprecated => Some(ctx.t("memory.status.deprecated")),
    MemoryStatus::Rejected => Some(ctx.t("memory.status.rejected")),
    _ => None,
  };
  rows.extend(decided.map(|label| {
    Entry::new("memory-details-decided", label)
      .hint(ctx.t_args(
        "memory.inspector.byYou",
        [("time", words::when(ctx, record.updated_at()))],
      ))
      .into()
  }));
  let id = "memory-details-source-section";
  parts::group(
    id,
    parts::head("memory-details-source", &ctx.t("memory.inspector.source"), None, None),
    parts::boxed(&format!("{id}-rows"), rows),
  )
  .into()
}

/// *History*: the person's last edit, then the earlier versions, newest first; `None` without either.
fn history(ctx: &Ctx, memory: &ProjectMemory, record: &Memory) -> Option<Element> {
  let earlier = listing::earlier_versions(record, &memory.records);
  if earlier.is_empty() && record.edited_at().is_none() {
    return None;
  }
  let mut rows: Vec<Element> = Vec::new();
  if let Some(edited) = record.edited_at() {
    rows.push(
      Entry::new("memory-details-edited", ctx.t("memory.inspector.editedTitle"))
        .hint(words::when(ctx, edited))
        .into(),
    );
  }
  let total = earlier.len();
  for (index, version) in earlier.iter().enumerate() {
    let title = ctx.t_args(
      "memory.inspector.versionTitle",
      [("version", (total - index).to_string())],
    );
    let hint = ctx.t_args(
      "memory.inspector.versionSuperseded",
      [("time", words::when(ctx, version.updated_at()))],
    );
    rows.push(
      Entry::new(format!("memory-details-version-{index}"), title)
        .hint(hint)
        .into(),
    );
  }
  let meta = match total {
    0 => None,
    1 => Some(ctx.t("memory.editor.versionsOne")),
    count => Some(ctx.t_args("memory.editor.versionsOther", [("count", count.to_string())])),
  };
  let id = "memory-details-history";
  Some(
    parts::group(
      id,
      parts::head(id, &ctx.t("memory.inspector.history"), meta.as_deref(), None),
      parts::boxed(&format!("{id}-rows"), rows),
    )
    .into(),
  )
}

/// *Given to agents* (`Kjws0`).
fn given_group(ctx: &Ctx, given: Result<Arc<GivenTo>, Option<Message>>, all: &Signal<bool>) -> Element {
  let id = "memory-details-given";
  let title = ctx.t("memory.inspector.given");
  let given = match given {
    Ok(given) => given,
    Err(reason) => {
      let words = reason.map_or_else(
        || ctx.t("memory.inspector.givenReading"),
        |reason| message_text(ctx, &reason),
      );
      let note = parts::note(&format!("{id}-note"), crate::ui::icon::Icon::Info, &words);
      return parts::group(id, parts::head(id, &title, None, None), note).into();
    }
  };
  let count = given.invocations.len();
  let meta = if count == 1 {
    ctx.t("memory.inspector.invocationsOne")
  } else {
    ctx.t_args("memory.inspector.invocations", [("count", count.to_string())])
  };
  let head = parts::head(id, &title, Some(&meta), None);
  if count == 0 {
    let note = parts::note(
      &format!("{id}-none"),
      crate::ui::icon::Icon::Info,
      &ctx.t("memory.inspector.givenNone"),
    );
    return parts::group(id, head, note).into();
  }
  let shown = if all.get() { count } else { count.min(GIVEN_SHOWN) };
  let rows = given
    .invocations
    .iter()
    .take(shown)
    .enumerate()
    .map(|(index, invocation)| {
      let place = given.workplace(invocation);
      Entry::new(
        format!("memory-details-invocation-{index}"),
        details::where_worked(ctx, place.clone()),
      )
      .hint(details::invocation_why(ctx, invocation, &place))
      .into()
    });
  let mut column = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(parts::boxed(&format!("{id}-rows"), rows));
  if count > GIVEN_SHOWN {
    let label = if all.get() {
      ctx.t("memory.inspector.showFewer")
    } else {
      ctx.t_args("memory.inspector.showAll", [("count", count.to_string())])
    };
    let toggle = all.clone();
    column = column.child(
      ActionButton::new("memory-details-given-all", ButtonKind::Ghost, label)
        .on_click(move || toggle.update(|all| *all = !*all)),
    );
  }
  parts::group(id, head, column).into()
}
