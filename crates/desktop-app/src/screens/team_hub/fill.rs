//! What the *On this computer* sections of the panels share (`Input` and `Note` in `C4` `SpRvd` … `C8` `lSA4x`,
//! `Note · warning` and the secret row in `Agents · Tools` `u9Hcc`): a box that looks like a field and holds words, the
//! row of a secret's value, and the note under the rows in the color of how the resource stands.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{secret::SecretName, sharing::Missing};

use super::rows::{self, Sub};
use crate::{
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::Press,
  },
};

/// A box that looks like a field and holds `text` after its glyph; it is not typed in. A text that is too long for it
/// ends in an ellipsis.
pub(super) fn input_box(id: &str, (glyph, color): (Icon, PaletteColor), text: Text) -> Row {
  Row::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceInput)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Base, color))
    .child(
      text
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .text_overflow(TextOverflow::Elipsis),
    )
}

/// How a secret's value stands here.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Value {
  Set,
  Missing,
  /// The keychain could not say.
  Unreadable,
}

pub(super) fn value_of(missing: &[Missing], name: &SecretName) -> Value {
  missing
    .iter()
    .find_map(|part| match part {
      Missing::Secret { name: lacking } if lacking == name => Some(Value::Missing),
      Missing::SecretUnreadable { name: unsure } if unsure == name => Some(Value::Unreadable),
      _ => None,
    })
    .unwrap_or(Value::Set)
}

/// One secret a resource takes: its name over *secret value*, and whether it has a value here. A value missing here is
/// given by pressing its box when `ask` says what that does (an admin's edit, whose bar is Save and Cancel), else from
/// the panel's bar.
pub(super) fn secret_row(ctx: &Ctx, name: &SecretName, value: Value, ask: Option<Press>) -> Row {
  let id = format!("team-hub-value-{name}");
  let (sub, shown): (Sub, Element) = match value {
    Value::Set => (
      Sub::quiet(ctx.t("team_hub.mcp.secret.sub")),
      Row::new()
        .id(format!("{id}-value"))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::Success))
        .child(
          Text::new(&ctx.t("team_hub.ready.valueSet"))
            .id(format!("{id}-words"))
            .variant(HubText::Status.role()),
        )
        .into(),
    ),
    Value::Missing => (
      Sub {
        words: ctx.t("team_hub.mcp.secret.sub"),
        color: PaletteColor::Warning,
      },
      missing_value(ctx, &id, ask),
    ),
    Value::Unreadable => (
      Sub {
        words: ctx.t("team_hub.mcp.secret.sub"),
        color: PaletteColor::Danger,
      },
      Row::new()
        .id(format!("{id}-value"))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::CircleAlert, IconSize::Base, PaletteColor::Danger))
        .child(
          Text::new(&ctx.t("team_hub.mcp.secret.unreadable"))
            .id(format!("{id}-words"))
            .variant(HubText::Status.role()),
        )
        .into(),
    ),
  };
  let label = Text::new(&name.to_string())
    .id(format!("{id}-label"))
    .variant(ExtraTypography::PropertyMono.role())
    .color(PaletteColor::TextSecondary);
  rows::keyed(&id, label, Some(sub), shown)
}

/// The box of a value missing here, which `ask` gives when it is pressed.
fn missing_value(ctx: &Ctx, id: &str, ask: Option<Press>) -> Element {
  let field = input_box(
    &format!("{id}-field"),
    (Icon::KeyRound, PaletteColor::TextMuted),
    Text::new(&ctx.t("team_hub.mcp.secret.noValue"))
      .id(format!("{id}-words"))
      .variant(HubText::Value.role())
      .color(PaletteColor::TextMuted),
  );
  let Some(ask) = ask else {
    return Row::new()
      .id(format!("{id}-value"))
      .width(Dimension::Pct(100.0))
      .child(field)
      .into();
  };
  ask
    .button()
    .id(format!("{id}-value"))
    .width(Dimension::Pct(100.0))
    .describe("a11y_name", ctx.t("team_hub.mcp.bar.addValue"))
    .child(field)
    .into()
}

/// How a resource stands, in the note's colors.
#[derive(Clone, Copy)]
pub(super) enum Tone {
  Ready,
  Warning,
  Danger,
  /// A person switched it off: nothing is wrong.
  Off,
}

impl Tone {
  const fn name(self) -> &'static str {
    match self {
      Self::Ready => "success",
      Self::Warning => "warning",
      Self::Danger => "danger",
      Self::Off => "neutral",
    }
  }

  const fn look(self) -> (PaletteColor, Icon, PaletteColor) {
    match self {
      Self::Ready => (PaletteColor::SuccessMuted, Icon::CircleCheck, PaletteColor::Success),
      Self::Warning => (PaletteColor::WarningMuted, Icon::CircleAlert, PaletteColor::Warning),
      Self::Danger => (PaletteColor::DangerMuted, Icon::CircleX, PaletteColor::Danger),
      Self::Off => (PaletteColor::AccentMuted, Icon::Power, PaletteColor::TextSecondary),
    }
  }
}

/// What stands under the rows: `words` in the color of `tone`; its text's id is `<id>-text`.
pub(super) fn note(id: &str, tone: Tone, words: &str) -> Row {
  let (fill, glyph, color) = tone.look();
  Row::new()
    .id(id.to_owned())
    .describe("tone", tone.name())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(fill)
    .rounded(RadiusSize::Md)
    .child(icon(glyph, IconSize::Md, color))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Prose.role())
        .color(PaletteColor::TextPrimary)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}

/// What an *On this computer* section holds when nothing is filled in on a computer (`Nothing here` in `c3jU1R`,
/// `c9YnL`): a check and `words`, in the one outline of the section's rows.
pub(super) fn nothing_here(id: &str, words: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, HubSpacing::RowY.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleCheck, IconSize::Base, PaletteColor::Success))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(ExtraTypography::Prose.role())
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
}
