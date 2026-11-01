//! Parts of Settings › Updates: what a press asks for ([`Parts`]), its quiet actions (*Check now*, *Pause*, *What's
//! new*: `Button/ghost`, a 14 icon and a 12/500 `text_secondary` label), a link in 12 `text_secondary` with its 12
//! `text_muted` mark, 6 apart (`Download 0.4.0 ↗` `S0Jl30`, `Check now` `dfK1H`), a download's bar (`Progress`
//! `vDmIX`), and the release notes under the row (`What's new` `RcNn4`: a `surface_panel` box, radius 6, 10 over and
//! under and 12 at its sides, its title in 12/500 and the notes in 12 `text_secondary`, 6 apart).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use super::{UpdateAction, UpdateDispatch};
use crate::{
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    markdown::{LinkHandler, MarkdownView},
    meter::{MeterTone, meter},
    pressable::{Focus, Press},
  },
};

/// Where the section's presses go, what *What's new* toggles, and what a link in the release notes does.
#[derive(Clone)]
pub(super) struct Parts {
  pub dispatch: UpdateDispatch,
  pub toggle_notes: Arc<dyn Fn() + Send + Sync>,
  /// Kept by the section, so the notes are laid out again only when they change.
  pub on_link: LinkHandler,
}

impl Parts {
  /// A press that asks for `action`.
  pub fn on(&self, action: UpdateAction) -> Press {
    let dispatch = self.dispatch.clone();
    Press::run(move || dispatch(action.clone()))
  }

  /// A quiet action of the update row, `settings-update-<name>`, its `glyph` before the words of `key`.
  pub fn ghost(&self, ctx: &Ctx, (name, glyph, key): (&str, Icon, &str), action: UpdateAction) -> Element {
    ActionButton::new(
      format!("settings-update-{name}"),
      ButtonKind::Ghost,
      ctx.t(key).to_string(),
    )
    .icon(glyph)
    .press(self.on(action))
    .into()
  }

  /// *What's new*, with `chevron-down` while the notes are closed and `chevron-up` while they show (`XIBIl`).
  pub fn notes_toggle(&self, ctx: &Ctx, open: bool) -> Element {
    let toggle = self.toggle_notes.clone();
    ActionButton::new(
      "settings-update-whats-new",
      ButtonKind::Ghost,
      ctx.t("settings.general.updates.whatsNew").to_string(),
    )
    .icon(if open { Icon::ChevronUp } else { Icon::ChevronDown })
    .held(open)
    .on_click(move || toggle())
    .into()
  }
}

/// A link with the element id `id`: `words` and an `arrow-up-right` after them.
pub(super) fn link(id: &str, words: &str, press: Press) -> Element {
  Focus::Link
    .apply(press.button())
    .id(id.to_owned())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", words.to_owned())
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .child(icon(Icon::ArrowUpRight, IconSize::Sm, PaletteColor::TextMuted))
    .into()
}

/// *Check now* at the end of *Last checked*: a `refresh-cw` before the words, as a link (`dfK1H`).
pub(super) fn leading_link(id: &str, (glyph, words): (Icon, &str), press: Press) -> Element {
  Focus::Link
    .apply(press.button())
    .id(id.to_owned())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .describe("a11y_name", words.to_owned())
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
    .child(
      Text::new(words)
        .id(format!("{id}-text"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

/// How far a download has come, `received` of `total` bytes, in `info`.
pub(super) fn progress(id: &str, (received, total): (u64, u64)) -> Element {
  meter(id, (received, total), MeterTone::Progress)
}

/// The release notes of `version` (`What's new in 0.4.0`), read as Markdown.
pub(super) fn notes(ctx: &mut Ctx, (version, text): (&str, &str), parts: &Parts) -> Element {
  let title = ctx.t_args("settings.general.updates.notesTitle", [("version", version.to_owned())]);
  let body: Element = if text.trim().is_empty() {
    Text::new(&ctx.t("settings.general.updates.noNotes"))
      .id("settings-update-notes-empty")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextSecondary)
      .into()
  } else {
    MarkdownView::new("settings-update-notes-text", text.to_owned(), parts.on_link.clone())
      .style(TypographyStyle::Description, PaletteColor::TextSecondary)
      .render(ctx)
  };
  Column::new()
    .id("settings-update-notes")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .rounded(RadiusSize::Md)
    .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Snug.role())
    .child(
      Text::new(&title)
        .id("settings-update-notes-title")
        .variant(TypographyStyle::Label)
        .color(PaletteColor::TextPrimary),
    )
    .child(body)
    .into()
}
