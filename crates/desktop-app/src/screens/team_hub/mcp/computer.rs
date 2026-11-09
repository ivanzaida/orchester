//! *On this computer* (`On this computer` `JYO4l` in `C4` `SpRvd` … `C8` `lSA4x`): the path of the program (set once,
//! for every project of the team server), the value of each secret the server takes, whether the server is on here, and
//! under the rows how it stands. Nothing here is sent to the team.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  sharing::{Missing, ProgramProblem},
  time::Timestamp,
};
use orchester_workspace::agents::team_mcp::{McpRuns, TeamMcpSheet};

use super::words;
use crate::{
  screens::team_hub::{
    actions::Busy,
    fill::{self, Tone},
    model::{Entry, Listing},
    rows::{self, Sub},
    words as list_words,
  },
  theme::{HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::Press,
    toggle::{Toggle, ToggleKind},
  },
};

/// What the rows stand on and what their controls do.
pub(super) struct View<'a> {
  pub listing: &'a Listing,
  pub entry: &'a Entry,
  pub sheet: &'a TeamMcpSheet,
  pub missing: &'a [Missing],
  pub busy: Busy,
  /// The moment the panel is drawn at, which a day is said against.
  pub now: Timestamp,
  /// What *Choose…* does.
  pub choose: Press,
  /// What the switch does when it is flipped to the value it is given.
  pub switch: Arc<dyn Fn(bool) + Send + Sync>,
}

pub(super) fn section(ctx: &Ctx, view: &View<'_>) -> Column {
  let mut rows = Vec::new();
  rows.extend(path_row(ctx, view));
  for name in &view.sheet.secrets {
    rows.push(fill::secret_row(ctx, name, fill::value_of(view.missing, name)));
  }
  rows.push(here_row(ctx, view));
  rows::on_this_computer(ctx, rows, Some(note(ctx, view).into()))
}

/// The program's path: not set (with *Choose…*), found, or set with its file gone. A remote server has none.
fn path_row(ctx: &Ctx, view: &View<'_>) -> Option<Row> {
  let McpRuns::Program { program, .. } = &view.sheet.runs else {
    return None;
  };
  let id = "team-hub-path";
  let (sub, value) = match (&view.sheet.program, words::program_problem(view.missing)) {
    (Some(set), Some(ProgramProblem::FileGone(_))) => (
      Sub {
        words: ctx.t("team_hub.mcp.path.gone"),
        color: PaletteColor::Danger,
      },
      fill::input_box(
        &format!("{id}-value"),
        (Icon::FileX, PaletteColor::Danger),
        Text::new(&set.path.display().to_string())
          .id(format!("{id}-value-text"))
          .variant(HubText::PanelMono.role())
          .nowrap(),
      )
      .border(Border::inside(BorderSize::Sm, PaletteColor::Danger)),
    ),
    (Some(set), _) => (
      Sub::quiet(ctx.t_args(
        "team_hub.mcp.path.found",
        [("date", list_words::date(ctx, view.now, set.set_at).to_string())],
      )),
      Row::new()
        .id(format!("{id}-value"))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::Success))
        .child(
          Text::new(&set.path.display().to_string())
            .id(format!("{id}-value-text"))
            .variant(HubText::Value.role())
            .nowrap()
            .min_width(Dimension::Px(0.0))
            .text_overflow(TextOverflow::Elipsis),
        ),
    ),
    (None, _) => (
      Sub::quiet(ctx.t_args("team_hub.mcp.path.sub", [("program", program.as_str())])),
      Row::new()
        .id(format!("{id}-value"))
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(fill::input_box(
          &format!("{id}-field"),
          (Icon::File, PaletteColor::TextMuted),
          Text::new(&ctx.t("team_hub.mcp.path.notSet"))
            .id(format!("{id}-value-text"))
            .variant(HubText::Value.role())
            .color(PaletteColor::TextMuted),
        ))
        .child(choose(ctx, view)),
    ),
  };
  Some(rows::row(id, &ctx.t("team_hub.mcp.path.label"), Some(sub), value))
}

fn choose(ctx: &Ctx, view: &View<'_>) -> Element {
  let button = ActionButton::new(
    "team-hub-choose-path",
    ButtonKind::Secondary,
    ctx.t("team_hub.mcp.path.choose"),
  )
  .icon(Icon::FolderOpen);
  match view.busy {
    Busy::Idle => button.press(view.choose.clone()),
    Busy::Taking => button.loading(),
    Busy::Reading => button.disabled(),
  }
  .into()
}

/// *On here*: the person's own choice to let runs on this computer use the server; nobody has decided until a person
/// switches it on or off.
fn here_row(ctx: &Ctx, view: &View<'_>) -> Row {
  let (on, words) = match view.sheet.switch {
    Some(true) => (true, ctx.t("team_hub.mcp.here.on")),
    Some(false) => (false, ctx.t("team_hub.mcp.here.off")),
    None => (false, ctx.t("team_hub.mcp.here.undecided")),
  };
  let toggle = Toggle::new("team-hub-here-switch", ToggleKind::Switch, on, words).label_style(HubText::Status.role());
  let toggle = match view.busy {
    Busy::Idle => {
      let switch = view.switch.clone();
      toggle.on_change(move |now| switch(now))
    }
    Busy::Taking | Busy::Reading => toggle,
  };
  rows::row("team-hub-here", &ctx.t("team_hub.mcp.here.label"), None, toggle)
}

/// What stands under the rows, in the color of how the server stands.
fn note(ctx: &Ctx, view: &View<'_>) -> Row {
  let name = view.entry.name.as_str();
  let (tone, words) = match view.missing.first() {
    None => (Tone::Ready, ctx.t_args("team_hub.mcp.note.ready", [("name", name)])),
    Some(Missing::McpOff { .. }) if view.sheet.switch == Some(false) => {
      (Tone::Off, ctx.t_args("team_hub.mcp.note.off", [("name", name)]))
    }
    Some(Missing::McpOff { .. }) => (
      Tone::Warning,
      ctx.t_args("team_hub.mcp.note.undecided", [("name", name)]),
    ),
    Some(Missing::McpProgram {
      problem: ProgramProblem::NotSet,
      ..
    }) => (Tone::Warning, ctx.t_args("team_hub.mcp.note.setPath", [("name", name)])),
    Some(Missing::McpProgram {
      problem: ProgramProblem::FileGone(path),
      ..
    }) => (
      Tone::Danger,
      ctx.t_args(
        "team_hub.mcp.note.gone",
        [("path", path.display().to_string()), ("name", name.to_owned())],
      ),
    ),
    Some(Missing::Secret { name: secret }) => (
      Tone::Warning,
      ctx.t_args("team_hub.mcp.note.secret", [("name", secret.to_string())]),
    ),
    Some(Missing::SecretUnreadable { name: secret }) => (
      Tone::Danger,
      ctx.t_args("team_hub.mcp.note.unreadable", [("name", secret.to_string())]),
    ),
    Some(Missing::McpUnsupported { .. }) => (
      Tone::Danger,
      ctx.t_args("team_hub.mcp.note.unsupported", [("name", name)]),
    ),
    // Parts of a profile, a reviewer or a local project: an MCP server of the team has none of them.
    Some(
      part @ (Missing::Connection { .. }
      | Missing::ConnectionSignIn { .. }
      | Missing::DecisionConnection { .. }
      | Missing::LocalMcpProgram { .. }
      | Missing::LocalSecret { .. }),
    ) => (Tone::Danger, list_words::missing(ctx, view.listing, view.entry, part)),
  };
  fill::note("team-hub-mcp-note", tone, &words)
}
