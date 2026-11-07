//! The pinned action of the profile panel (`Actions` `o8tFHR` in `Agents · Profile` `NDZXC`): the fix of the first
//! missing part, and at its end what comes after it. No bar when nothing is missing or the first missing part is one
//! only the team can change.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::sharing::Missing;
use orchester_workspace::agents::team_profile::TeamProfileSheet;

use super::fix::Fix;
use crate::{
  screens::team_hub::{
    model::{Entry, Listing},
    words::cli_key,
  },
  theme::{HubSpacing, HubText},
  ui::{
    button::{ActionButton, ButtonKind},
    pressable::Press,
  },
};

/// The pinned actions' height (`Actions` `o8tFHR`).
const HEIGHT: f32 = 56.0;

/// What the bar needs of the panel.
pub(super) struct Words<'a> {
  pub listing: &'a Listing,
  pub entry: &'a Entry,
  pub sheet: &'a TeamProfileSheet,
  /// The connection the person chose in the select: the first one while none is.
  pub connection: &'a str,
}

impl Words<'_> {
  fn server(&self, server: orchester_core::id::ResourceId) -> String {
    self
      .listing
      .find(server)
      .map_or_else(String::new, |found| found.name.clone())
  }

  /// What the fix does, in the words of the pinned action.
  pub(super) fn action(&self, ctx: &Ctx, fix: &Fix) -> String {
    match fix {
      Fix::UseConnection => ctx
        .t_args(
          "team_hub.fix.action.use",
          [("connection", self.connection), ("profile", self.entry.name.as_str())],
        )
        .to_string(),
      Fix::AddConnection => ctx
        .t_args("team_hub.fix.action.add", [("cli", &*ctx.t(cli_key(self.sheet.cli)))])
        .to_string(),
      Fix::SwitchOn(server) => ctx
        .t_args("team_hub.fix.action.switchOn", [("server", self.server(*server))])
        .to_string(),
      Fix::SetPath(server) => ctx
        .t_args("team_hub.fix.action.setPath", [("server", self.server(*server))])
        .to_string(),
      Fix::AddValue(name) => ctx
        .t_args("team_hub.fix.action.addValue", [("name", name.to_string())])
        .to_string(),
    }
  }

  /// What stands at the bar's end: what the fix asks of the person, what comes next, or when the profile runs.
  fn hint(&self, ctx: &Ctx, fix: &Fix, missing: &[Missing]) -> String {
    match fix {
      Fix::UseConnection => return ctx.t("team_hub.fix.hint.use").to_string(),
      Fix::AddConnection => return ctx.t("team_hub.fix.hint.add").to_string(),
      _ => {}
    }
    match missing.iter().skip(1).find_map(Fix::of) {
      Some(next) => ctx
        .t_args("team_hub.fix.hint.then", [("what", self.action(ctx, &next))])
        .to_string(),
      None => ctx
        .t_args("team_hub.fix.hint.done", [("profile", self.entry.name.as_str())])
        .to_string(),
    }
  }
}

/// The bar of the first missing part's `fix`, which `press` does; `pending` while it is being taken.
pub(super) fn bar(
  ctx: &Ctx,
  words: &Words<'_>,
  (fix, missing): (&Fix, &[Missing]),
  press: Press,
  pending: bool,
) -> Element {
  let button = ActionButton::new(
    format!("team-hub-fix-{}", fix.name()),
    ButtonKind::Primary,
    words.action(ctx, fix),
  )
  .icon(fix.glyph());
  let button = if pending { button.loading() } else { button.press(press) };
  Row::new()
    .id("team-hub-actions")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEIGHT))
    .padding(Padding::horizontal(HubSpacing::Sections.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfacePanel)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(button)
    .child(Row::new().flex(1.0))
    .child(
      Text::new(&words.hint(ctx, fix, missing))
        .id("team-hub-actions-hint")
        .variant(HubText::Hint.role())
        .nowrap(),
    )
    .into()
}
