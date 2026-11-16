//! UI driver (`UI driver` `JNNox`): how agents see and use the service through the `ui_*` tools, said at the
//! heading's end (`S3nHcx`), as a card per driver side by side (`Picker` `AHbLE`), then *Opens at*, where `ui_open`
//! starts, in a 260 field (`MFUDY`) while a driver is chosen. Only *none*, *native* and *mcp* are offered: the design's
//! *browser* and *cdp* wait for the in-app Browser's automation (`docs/architecture/agent-testing.md`). A card chosen,
//! and the field left, saves.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::Alignment,
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::service::UiDriver;
use orchester_workspace::agent_testing::editor::order::Field;

use super::{
  super::super::kit::Group,
  Commit, View,
  fields::{field, input, placeholder_in, with_error},
  sections::{DriverChoice, Sections},
};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize, ServicesText},
  ui::{
    icon::{Icon, icon},
    pressable::{Focus, pressable},
  },
};

/// Where `ui_open` starts (`Input` `p7rgg`).
const START_WIDTH: f32 = 260.0;
/// What *Opens at* shows while it is empty: a place written with placeholders, not words (`HkK1U`).
const START_EXAMPLE: &str = "{{url}}/";

/// A driver offered: its name, which is also its id's part, its glyph and the key of what it drives.
type Driver = (UiDriver, &'static str, Icon, &'static str);

/// The drivers offered.
const DRIVERS: [Driver; 3] = [
  (
    UiDriver::None,
    "none",
    Icon::Ban,
    "project_settings.services.editor.driver.none",
  ),
  (
    UiDriver::Native,
    "native",
    Icon::AppWindow,
    "project_settings.services.editor.driver.native",
  ),
  (
    UiDriver::Mcp,
    "mcp",
    Icon::Plug,
    "project_settings.services.editor.driver.mcp",
  ),
];

/// The UI driver group.
pub(super) fn ui_driver_group(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let chosen = sections.driver.get().0;
  let cards = DRIVERS.map(|driver| card(ctx, sections, driver, chosen == driver.0, commit));
  let picker = Row::new()
    .id("settings-service-driver")
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .with_children(cards);
  let opens = (chosen != UiDriver::None).then(|| opens_row(ctx, sections, view, commit));
  Group::new(
    "settings-service-driver-group",
    ctx.t("project_settings.services.editor.driver.title"),
  )
  .end(
    Text::new(&ctx.t("project_settings.services.editor.driver.note"))
      .id("settings-service-driver-note")
      .variant(ExtraTypography::Note.role()),
  )
  .row(picker)
  .rows(opens)
  .into()
}

/// A driver's card: its glyph and name over what it drives; chosen, on `accent_muted` in a `border_focus` stroke with a
/// check at its name's end (`Driver · browser` `cKQAD`).
fn card(ctx: &Ctx, sections: &Sections, (driver, name, glyph, key): Driver, chosen: bool, commit: &Commit) -> Element {
  let (driver_signal, commit) = (sections.driver.clone(), commit.clone());
  let id = format!("settings-service-driver-{name}");
  let tone = if chosen {
    PaletteColor::TextPrimary
  } else {
    PaletteColor::TextSecondary
  };
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, tone.clone()))
    .child(
      Text::new(name)
        .id(format!("{id}-name"))
        .variant(ServicesText::DriverName.role())
        .color(tone),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(chosen.then(|| icon(Icon::CircleCheck, IconSize::Base, PaletteColor::TextPrimary)));
  let words = ctx.t(key);
  let (fill, stroke) = if chosen {
    (Some(PaletteColor::AccentMuted), PaletteColor::BorderFocus)
  } else {
    (None, PaletteColor::Border)
  };
  let frame = Focus::Keyboard
    .apply(pressable(move || {
      driver_signal.set(DriverChoice(driver));
      commit();
    }))
    .id(id.clone())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .border(Border::inside(BorderSize::Sm, stroke))
    .rounded(RadiusSize::Md)
    .cursor(CursorIcon::Pointer)
    .describe("chosen", if chosen { "true" } else { "false" })
    .describe("a11y_name", name)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(head)
        .child(
          Text::new(&words)
            .id(format!("{id}-words"))
            .variant(ExtraTypography::Note.role())
            .width(Dimension::Pct(100.0)),
        ),
    );
  match fill {
    Some(fill) => frame.background(fill).into(),
    None => frame.into(),
  }
}

/// *Opens at*: where `ui_open` starts, its placeholders filled per run.
fn opens_row(ctx: &mut Ctx, sections: &Sections, view: &View, commit: &Commit) -> Element {
  let label = ctx.t("project_settings.services.editor.driver.opensAt");
  let error = view
    .problems()
    .and_then(|problems| problems.ui_start.clone())
    .or_else(|| placeholder_in(view, Field::UiStart));
  let control = input(("settings-service-opens-at", label.clone()), &sections.ui_start, commit)
    .width(Dimension::Px(START_WIDTH))
    .placeholder(START_EXAMPLE);
  let control = with_error(ctx, control, error.as_ref()).render(ctx);
  field(
    "settings-service-opens-at",
    label,
    ctx.t("project_settings.services.editor.driver.opensAtHint"),
  )
  .render(ctx, control)
}
