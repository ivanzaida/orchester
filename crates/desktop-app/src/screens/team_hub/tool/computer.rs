//! *On this computer* (`On this computer` in `Agents · Tools` `u9Hcc`): the value of the secret the tool takes, with
//! *No value here* until a person adds one, and under the rows how the tool stands. A secret named by id cannot be
//! filled in here yet: the row says so and has no action. Nothing here is sent to the team.

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
    rows::{self, Key, Sub},
  },
  theme::HubText,
  ui::icon::Icon,
};

pub(super) fn section(ctx: &Ctx, entry: &Entry, sheet: &TeamToolSheet, missing: &[Missing]) -> Column {
  let mut rows = Vec::new();
  let mut named = Vec::new();
  for part in missing {
    if let Missing::Secret { name } | Missing::SecretUnreadable { name } = part
      && !named.contains(name)
    {
      named.push(name.clone());
      rows.push(fill::secret_row(ctx, Key::Wide, name, fill::value_of(missing, name)));
    }
  }
  if matches!(sheet.secret, ToolSecret::ById(_)) {
    rows.push(by_id_row(ctx));
  }
  if rows.is_empty() {
    rows.push(fill::nothing_here(
      "team-hub-nothing",
      &ctx.t("team_hub.tool.nothingHere"),
    ));
  }
  rows::on_this_computer(ctx, rows, note(ctx, entry, sheet, missing).map(Into::into))
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

/// What stands under the rows, in the color of how the tool stands: nothing for a tool that is ready.
fn note(ctx: &Ctx, entry: &Entry, sheet: &TeamToolSheet, missing: &[Missing]) -> Option<Row> {
  let name = entry.name.as_str();
  let (tone, words) = match missing.first() {
    Some(Missing::Secret { name: secret }) => (
      Tone::Warning,
      ctx.t_args(
        "team_hub.tool.note.secret",
        [("secret", secret.to_string()), ("name", name.to_owned())],
      ),
    ),
    Some(Missing::SecretUnreadable { name: secret }) => (
      Tone::Danger,
      ctx.t_args("team_hub.mcp.note.unreadable", [("name", secret.to_string())]),
    ),
    // Parts of a profile, an MCP server, a reviewer or a local project: a tool of the team has none of them.
    Some(
      Missing::Connection { .. }
      | Missing::DecisionConnection { .. }
      | Missing::McpOff { .. }
      | Missing::McpProgram { .. }
      | Missing::McpUnsupported { .. }
      | Missing::LocalMcpProgram { .. }
      | Missing::LocalSecret { .. },
    )
    | None => match sheet.secret {
      ToolSecret::ById(_) => (Tone::Warning, ctx.t_args("team_hub.tool.note.byId", [("name", name)])),
      ToolSecret::None => return None,
    },
  };
  Some(fill::note("team-hub-tool-note", tone, &words))
}
