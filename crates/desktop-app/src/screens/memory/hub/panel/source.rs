//! The panel's *Source* (`Source` `TYGeS` in Y1, `NmjbF` in Y2) — a proposal's key, the importance, the step that
//! wrote it with how many transcript events and *Show in Conversation ↗*, the run, the agent's profile and its
//! confidence; a person's record says it is theirs — and *Given to agents* (`Given to agents` `kIRpz` in Y2): how many
//! invocations were given the record, the latest two of them with when and how many tokens, then *Show all N
//! invocations*. The invocations are read off the UI thread when the record is picked.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::dimension::Dimension,
};
use orchester_core::memory::{Memory, MemoryAuthor, MemoryStatus};
use orchester_workspace::{
  background,
  memory::{
    origin::Workplace,
    read::{GivenTo, ProjectMemory},
  },
  messages::{self, Message},
};

use crate::{
  screens::{
    memory::{
      opened, parts,
      state::{MemoryState, Shared},
      words,
    },
    team_hub::rows,
  },
  services::Services,
  theme::HubText,
  ui::{
    button::{ActionButton, ButtonKind},
    message::message_text,
  },
};

/// The invocations listed before *Show all N invocations* (`hzoBL`).
const GIVEN_SHOWN: usize = 2;

/// *Source*: where the record came from, a row each.
pub(super) fn source(ctx: &mut Ctx, state: &MemoryState, memory: &ProjectMemory, record: &Memory) -> Column {
  let content = record.content();
  let value = |id: &str, words: &str| rows::words(&format!("{id}-value"), words);
  let mut rows = Vec::new();
  if record.status() == MemoryStatus::Proposed
    && let Some(key) = content.key()
  {
    rows.push(rows::row(
      "memory-source-key",
      &ctx.t("memory.hub.source.key"),
      None,
      rows::mono("memory-source-key-value", key.as_str()),
    ));
  }
  rows.push(rows::row(
    "memory-source-importance",
    &ctx.t("memory.editor.importance"),
    None,
    value(
      "memory-source-importance",
      &words::importance(ctx, content.importance()),
    ),
  ));
  match memory.origins.written(record) {
    Some(place) => rows.extend(agent_rows(ctx, state, (memory, record), &place)),
    None => rows.push(rows::row(
      "memory-source-author",
      &ctx.t("memory.editor.author"),
      None,
      value("memory-source-author", &ctx.t("memory.meta.you")),
    )),
  }
  rows::section("memory-source", &ctx.t("memory.editor.source"), Vec::new(), rows)
}

/// Where an agent wrote the record: its step with the conversation, its run, its profile and its confidence.
fn agent_rows(
  ctx: &mut Ctx,
  state: &MemoryState,
  (memory, record): (&ProjectMemory, &Memory),
  place: &Workplace,
) -> Vec<Row> {
  let value = |id: &str, words: &str| rows::words(&format!("{id}-value"), words);
  let mut rows = Vec::new();
  if let Some((number, name)) = &place.step {
    let step = words::step(ctx, *number, name);
    let mut said = vec![ctx.t_args("memory.editor.sourceStep", [("step", &*step)]).to_string()];
    said.extend(match record.sources().len() {
      0 => None,
      1 => Some(ctx.t("memory.hub.source.eventOne").to_string()),
      count => Some(
        ctx
          .t_args("memory.hub.source.events", [("count", count.to_string())])
          .to_string(),
      ),
    });
    let link = opened::source_task(record, &memory.origins).map(|task| {
      parts::conversation_link(
        ctx,
        (state.project_id, &task),
        ("memory-source-conversation", &ctx.t("memory.editor.sourceLink")),
      )
    });
    let step = Row::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .wrap()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&words::join(ctx, &said))
          .id("memory-source-step-value")
          .variant(HubText::Value.role()),
      )
      .with_children(link);
    rows.push(rows::row(
      "memory-source-step",
      &ctx.t("memory.hub.source.step"),
      None,
      step,
    ));
  }
  if let Some(run) = place.run {
    let mut said = vec![
      ctx
        .t_args("memory.meta.run", [("run", run.get().to_string())])
        .to_string(),
    ];
    said.extend(place.attempt.map(|attempt| {
      ctx
        .t_args("memory.meta.attempt", [("attempt", attempt.get().to_string())])
        .to_string()
    }));
    said.push(words::when(ctx, record.created_at()));
    rows.push(rows::row(
      "memory-source-run",
      &ctx.t("memory.hub.source.run"),
      None,
      value("memory-source-run", &words::join(ctx, &said)),
    ));
  }
  let agent = place.agent.clone().unwrap_or_default();
  let mut profile = vec![agent.to_string()];
  if record.edited_at().is_some() && matches!(record.author(), MemoryAuthor::Agent(_)) {
    profile.push(ctx.t("memory.hub.source.authorKept").to_string());
  }
  rows.push(rows::row(
    "memory-source-agent",
    &ctx.t("memory.hub.source.agent"),
    None,
    value("memory-source-agent", &words::join(ctx, &profile)),
  ));
  rows.push(rows::row(
    "memory-source-confidence",
    &ctx.t("memory.editor.confidence"),
    None,
    value(
      "memory-source-confidence",
      &ctx.t_args(
        "memory.editor.confidenceValue",
        [
          ("confidence", &*words::confidence(ctx, record.confidence())),
          ("agent", &*agent),
        ],
      ),
    ),
  ));
  rows
}

/// The invocations given `record`, read when it is picked; `Err(None)` while they are read.
pub(super) fn given(ctx: &mut Ctx, state: &MemoryState, record: &Memory) -> Result<Arc<GivenTo>, Option<Message>> {
  let hold = Services::of(ctx).hold;
  let read = ctx
    .future(
      (state.repo.clone(), record.id(), record.revision()),
      move |(repo, id, _)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || GivenTo::load_in(&repo, id).map(Shared::new))
            .await?
            .map_err(|error| messages::store_error(&error))
        }
      },
    )
    .state()
    .get();
  if let Some(error) = read.error.clone().filter(|_| read.is_rejected()) {
    return Err(Some(error));
  }
  read.data.map(|given| given.0).ok_or(None)
}

/// *Given to agents · N invocations* and the latest of them, or what stands in for them while they are read, when
/// they cannot be, and when none was given it.
pub(super) fn given_section(ctx: &Ctx, given: Result<Arc<GivenTo>, Option<Message>>, all: &Signal<bool>) -> Column {
  let id = "memory-given";
  let label = ctx.t("memory.inspector.given");
  let given = match given {
    Ok(given) => given,
    Err(reason) => {
      let words = reason.map_or_else(
        || ctx.t("memory.inspector.givenReading"),
        |reason| message_text(ctx, &reason),
      );
      return quiet(id, &label, &words);
    }
  };
  let count = given.invocations.len();
  if count == 0 {
    return quiet(id, &label, &ctx.t("memory.inspector.givenNone"));
  }
  let invocations = if count == 1 {
    ctx.t("memory.inspector.invocationsOne")
  } else {
    ctx.t_args("memory.inspector.invocations", [("count", count.to_string())])
  };
  let after = Text::new(&invocations)
    .id(format!("{id}-count"))
    .variant(HubText::PanelHint.role())
    .nowrap();
  let shown = if all.get() { count } else { count.min(GIVEN_SHOWN) };
  let rows = given
    .invocations
    .iter()
    .take(shown)
    .enumerate()
    .map(|(index, invocation)| {
      let place = given.workplace(invocation);
      let row_id = format!("{id}-invocation-{index}");
      let when = Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(Text::new(&words::when(ctx, invocation.started_at)).variant(HubText::Value.role()))
        .child(Row::new().flex(1.0))
        .child(
          Text::new(&ctx.t_args("memory.inspector.tokens", [("tokens", invocation.tokens.to_string())]))
            .variant(HubText::PanelHint.role())
            .nowrap(),
        );
      rows::row(&row_id, &where_worked(ctx, place), None, when)
    })
    .collect();
  let mut column = rows::section(id, &label, vec![after.into()], rows);
  if count > GIVEN_SHOWN {
    let words = if all.get() {
      ctx.t("memory.inspector.showFewer")
    } else {
      ctx.t_args("memory.inspector.showAll", [("count", count.to_string())])
    };
    let toggle = all.clone();
    column = column.child(
      Row::new().child(
        ActionButton::new(format!("{id}-all"), ButtonKind::Ghost, words)
          .on_click(move || toggle.update(|all| *all = !*all)),
      ),
    );
  }
  column
}

/// A section that says `words` in place of its rows.
fn quiet(id: &str, label: &str, words: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Text::new(label)
        .id(format!("{id}-label"))
        .variant(HubText::PanelLabel.role()),
    )
    .child(
      Text::new(words)
        .id(format!("{id}-note"))
        .variant(HubText::PanelHint.role())
        .color(PaletteColor::TextMuted),
    )
}

/// Where an agent worked, without its role: *KON-8 · Run 104 · 02 Plan*.
fn where_worked(ctx: &Ctx, place: Workplace) -> String {
  let mut parts = Vec::new();
  parts.extend(place.task.as_ref().map(ToString::to_string));
  let place = Workplace {
    agent: None,
    task: None,
    ..place
  };
  parts.extend(words::workplace(ctx, &place, false));
  words::join(ctx, &parts).to_string()
}
