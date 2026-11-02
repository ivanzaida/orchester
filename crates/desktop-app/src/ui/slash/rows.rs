//! What the slash menu's panel is made of (`IQOLN`, `OKWFL`, `G2ajn7`): a section's head (*Commands*, *Skills*) with
//! its count or note at its end, 6 over and 4 under its words, 8 at its sides; a row, 28 high: a 14 icon, the name in
//! a 150 column (a skill's in Geist Mono; once something is typed the matched letters in 600 and the rest in
//! `text_secondary`) and what it does in `text_muted`, the focus on `hover` with *Enter* at its end, and one that
//! cannot be chosen faded to half, saying why in `text_secondary`; with nothing that matches (`search-x`) that Enter
//! sends the text as typed; and the keys under them, with *Type to filter* or what was typed at their end.

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use super::{Pick, SlashField};
use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    kbd::key_cap,
    pressable::{inert, pressable},
  },
};

/// A name's column (`Label` `CXDhk`).
const LABEL_WIDTH: f32 = 150.0;

/// What a row shows; what it does stays with the field.
#[derive(Clone, PartialEq)]
pub(super) struct Face {
  pub(super) pick: Pick,
  /// What names its element after the menu's id: `slash-<name>`.
  pub(super) name: Arc<str>,
  pub(super) icon: Icon,
  pub(super) label: Arc<str>,
  /// What it does, or why it cannot be chosen.
  pub(super) detail: Arc<str>,
  pub(super) runs: bool,
  /// A skill: its name is in Geist Mono.
  pub(super) skill: bool,
}

/// A section's head: its title (`key`) and at its end `end`, its count or the composer's note; `name` names it.
pub(super) fn heading(ctx: &Ctx, (name, key): (&str, &str), end: Option<Arc<str>>) -> Element {
  Row::new()
    .id(format!("slash-section-{name}"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Snug.role())
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs)
        .left(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Text::new(&ctx.t(key)).variant(SlashText::Section.role()).nowrap())
    .child(Spacer::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id(format!("slash-section-{name}-note"))
        .variant(SlashText::Note.role())
        .nowrap()
    }))
    .into()
}

/// A row of the menu `id`: chosen by a click while it can be, faded with why while it cannot.
pub(super) fn row(
  ctx: &Ctx,
  (id, field): (&str, &SlashField),
  (face, letters, filtered): (&Face, Range<usize>, bool),
  focused: bool,
) -> Element {
  let detail_color = if face.runs {
    PaletteColor::TextMuted
  } else {
    PaletteColor::TextSecondary
  };
  let name = &face.name;
  let children: Vec<Element> = vec![
    icon(face.icon, IconSize::Md, PaletteColor::TextSecondary).into(),
    label(face, letters, filtered),
    Text::new(&face.detail)
      .id(format!("{id}-slash-{name}-detail"))
      .variant(SlashText::Detail.role())
      .color(detail_color)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .into(),
  ];
  let enter = focused.then(|| key_cap("slash-enter", &ctx.t("composer.slash.keys.enter")));
  let surface = |button: Button| {
    button
      .id(format!("{id}-slash-{name}"))
      .describe("a11y_name", face.label.clone())
      .describe("focused", focused.to_string())
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .rounded(RadiusSize::Sm)
      .with_children(children)
      .with_children(enter)
  };
  if !face.runs {
    return surface(inert(Button::empty()))
      .describe("state", "unavailable")
      .describe("reason", face.detail.clone())
      .opacity(DISABLED_OPACITY)
      .into();
  }
  let (field, pick) = (field.clone(), face.pick.clone());
  let button = surface(pressable(move || field.run(&pick)))
    .describe("state", "enabled")
    .hovered_style(Style::new().background(ExtraColor::Hover.role()));
  if focused {
    button.background(ExtraColor::Hover.role()).into()
  } else {
    button.into()
  }
}

/// The name in its column: whole in `text_primary` while nothing is typed; once something is, the matched `letters`
/// in bold and the rest in `text_secondary`.
fn label(face: &Face, letters: Range<usize>, filtered: bool) -> Element {
  let words = &*face.label;
  let (plain, bold) = if face.skill {
    (SlashText::SkillLabel, SlashText::SkillMatch)
  } else {
    (SlashText::Label, SlashText::Match)
  };
  let column = Row::new()
    .width(Dimension::Px(LABEL_WIDTH))
    .flex_shrink(0.0)
    .align_items(Alignment::Center);
  if !filtered {
    return column
      .child(
        Text::new(words)
          .variant(plain.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .into();
  }
  let piece = |range: Range<usize>, matched: bool| {
    let words = &words[range];
    (!words.is_empty()).then(|| {
      let text = Text::new(&words.replace(' ', "\u{a0}")).nowrap();
      if matched {
        text.variant(bold.role())
      } else {
        text.variant(plain.role()).color(PaletteColor::TextSecondary)
      }
    })
  };
  column
    .with_children(piece(0..letters.start, false))
    .with_children(piece(letters.clone(), true))
    .with_children(piece(letters.end..words.len(), false))
    .into()
}

/// Nothing matches `typed` (`No result` in `G2ajn7`): `search-x` in `text_muted`, the title and that Enter sends the
/// text as typed.
pub(super) fn nothing(ctx: &Ctx, typed: &str) -> Element {
  let title = ctx.t_args("composer.slash.empty.title", [("query", typed)]);
  let hint = ctx.t_args("composer.slash.empty.hint", [("query", typed)]);
  Row::new()
    .id("slash-nothing")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::SearchX, IconSize::Md, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(ExtraSpacing::Tight.role())
        .child(
          Text::new(&title)
            .id("slash-nothing-title")
            .variant(SlashText::Label.role()),
        )
        .child(
          Text::new(&hint)
            .id("slash-nothing-hint")
            .variant(SlashText::Detail.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// The keys (`Footer` `h3VUn7`): move, choose and close while there are rows, send as text and close otherwise; and at
/// the end *Type to filter*, or what was typed.
pub(super) fn footer(ctx: &Ctx, typed: &str, rows: bool) -> Element {
  let hint = |cap: &str, label: &str| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(key_cap("slash-key", &ctx.t(&format!("composer.slash.keys.{cap}"))))
      .child(
        Text::new(&ctx.t(&format!("composer.slash.hint.{label}")))
          .variant(SlashText::Note.role())
          .nowrap(),
      )
  };
  let hints = if rows {
    vec![hint("move", "move"), hint("enter", "choose"), hint("esc", "close")]
  } else {
    vec![hint("enter", "sendAsText"), hint("esc", "close")]
  };
  let note = if typed.is_empty() {
    ctx.t("composer.slash.typeToFilter")
  } else {
    ctx.t_args("composer.slash.typed", [("query", typed)])
  };
  Row::new()
    .id("slash-footer")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs)
        .left(SpacingSize::Sm),
    )
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(hints)
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&note)
        .id("slash-footer-note")
        .variant(SlashText::Note.role())
        .nowrap(),
    )
    .into()
}
