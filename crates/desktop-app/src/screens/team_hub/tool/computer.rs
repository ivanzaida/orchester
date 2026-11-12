//! *On this computer* (`On this computer` in `Agents · Tools` `u9Hcc`, `yTVeE`): the value of the team secret the tool
//! names, set here or not, and under the rows how the tool stands. A tool that names none has nothing to fill in; one
//! whose version names its secret by id cannot be filled in here, and the row says so without the id. Nothing here is
//! sent to the team.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::{Column, Row, Text},
};
use orchester_core::sharing::Missing;
use orchester_workspace::agents::team_tool::{TeamToolSheet, ToolSecret};

use crate::{
  screens::team_hub::{
    fill::{self, Tone},
    model::Entry,
    rows::{self, Sub},
  },
  theme::HubText,
  ui::{icon::Icon, pressable::Press},
};

/// The section; `ask` gives a value missing here when its box is pressed (an admin's edit), else the bar does.
pub(super) fn section(
  ctx: &Ctx,
  (entry, sheet): (&Entry, &TeamToolSheet),
  missing: &[Missing],
  ask: Option<Press>,
) -> Column {
  let rows = match &sheet.secret {
    ToolSecret::Named(name) => vec![fill::secret_row(ctx, name, fill::value_of(missing, name), ask)],
    ToolSecret::ById => vec![by_id_row(ctx)],
    ToolSecret::None => vec![fill::nothing_here(
      "team-hub-nothing",
      &ctx.t("team_hub.tool.nothingHere"),
    )],
  };
  rows::on_this_computer(ctx, rows, note(ctx, entry, sheet, missing).map(Into::into))
}

/// The secret a version names by id: its value cannot be set here, so the row says so and holds no control.
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

/// What stands under the rows: whether the secret has a value here, or that a version naming it by id cannot be filled
/// in; nothing for a tool that names none.
fn note(ctx: &Ctx, entry: &Entry, sheet: &TeamToolSheet, missing: &[Missing]) -> Option<Row> {
  let tool = entry.name.as_str();
  let (tone, words) = match &sheet.secret {
    ToolSecret::None => return None,
    ToolSecret::ById => (Tone::Warning, ctx.t_args("team_hub.tool.note.byId", [("name", tool)])),
    ToolSecret::Named(secret) => {
      let args = [("secret", secret.to_string()), ("name", tool.to_owned())];
      match missing.first() {
        None => (Tone::Ready, ctx.t_args("team_hub.tool.note.ready", args)),
        Some(Missing::SecretUnreadable { .. }) => (Tone::Danger, ctx.t_args("team_hub.tool.note.unreadable", args)),
        Some(_) => (Tone::Warning, ctx.t_args("team_hub.tool.note.secret", args)),
      }
    }
  };
  Some(fill::note("team-hub-tool-note", tone, &words))
}
