//! *On this computer* (`On this computer` in `Agents · Tools` `u9Hcc`): a secret named by id cannot be filled in here
//! yet, so the row says so and has no action; a tool that names none has nothing to fill in. Nothing here is sent to
//! the team.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
};
use orchester_workspace::agents::team_tool::{TeamToolSheet, ToolSecret};

use crate::{
  screens::team_hub::{
    fill::{self, Tone},
    model::Entry,
    rows::{self, Sub},
  },
  theme::HubText,
  ui::icon::Icon,
};

pub(super) fn section(ctx: &Ctx, entry: &Entry, sheet: &TeamToolSheet) -> Column {
  let rows = match sheet.secret {
    ToolSecret::ById(_) => vec![by_id_row(ctx)],
    ToolSecret::None => vec![fill::nothing_here(
      "team-hub-nothing",
      &ctx.t("team_hub.tool.nothingHere"),
    )],
  };
  rows::on_this_computer(ctx, rows, note(ctx, entry, sheet).map(Into::into))
}

/// The secret the tool names by id: its value cannot be set here yet, so the row says so and holds no control.
fn by_id_row(ctx: &Ctx) -> Row {
  let id = "team-hub-secret-by-id";
  let value = fill::input_box(
    &format!("{id}-field"),
    (Icon::Lock, PaletteColor::TextMuted),
    Text::new(&ctx.t("team_hub.tool.cannotFill"))
      .id(format!("{id}-words"))
      .variant(HubText::Value.role())
      .color(PaletteColor::TextMuted),
  );
  let sub = Sub {
    words: ctx.t("team_hub.tool.cannotFillSub"),
    color: PaletteColor::Warning,
  };
  rows::row(id, &ctx.t("team_hub.tool.valueLabel"), Some(sub), value)
}

/// What stands under the rows: that a secret named by id cannot be filled in here yet; nothing for a tool that names
/// none.
fn note(ctx: &Ctx, entry: &Entry, sheet: &TeamToolSheet) -> Option<Row> {
  match sheet.secret {
    ToolSecret::ById(_) => Some(fill::note(
      "team-hub-tool-note",
      Tone::Warning,
      &ctx.t_args("team_hub.tool.note.byId", [("name", entry.name.as_str())]),
    )),
    ToolSecret::None => None,
  }
}
