//! The find bar (`Find bar` `XeDdW` in 13b, `qnJbO` in 13c): over the chat at its top right, on `surface_raised` in a
//! `border_strong` stroke, radius 10, 4 inside and between its parts, over the popover's shadow. A 260 search field
//! with where the current place is among those found (*14 of 17*, *1 of 1,000+* past a thousand), the previous and
//! next place, a 1 × 16 `border` rule and the cross that closes it, small icon buttons. While a new query is searched
//! for the count is a turning loader; with nothing found the field takes the `danger` stroke, *No matches* is in
//! `danger`, previous and next cannot be pressed, and under the field what was searched (`Note` `O51i7`: a 12 `info`
//! and 11.5 `text_muted`, 6 apart, 2 over, 4 under and 6 at the sides).

use lurq::{
  app::{
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, ShadowStyle, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::tokens::grouped;

use super::TranscriptSearch;
use crate::{
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    field::{FieldInput, UnitLook},
    icon::{Icon, icon},
    icon_button::IconButton,
  },
};

/// The find field's width (`er9VW`) and the rule's height (`H8wkzE`).
const FIELD_WIDTH: f32 = 260.0;
const RULE_HEIGHT: f32 = 16.0;
/// Past this many places the count says *1,000+*; previous and next still reach every one.
const COUNTED: usize = 1_000;

/// What the bar says of the search: how many places it found and which is current, whether a new query is still being
/// searched for, and how many entries the run's transcript has.
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct Count {
  pub(in super::super) found: usize,
  pub(in super::super) current: Option<usize>,
  pub(in super::super) searching: bool,
  pub(in super::super) entries: usize,
}

/// The bar of `search`, saying `count`.
pub(in super::super) fn bar(ctx: &mut Ctx, search: &TranscriptSearch, count: Count) -> Element {
  let typed = !search.query.get().trim().is_empty();
  let nothing = typed && !count.searching && count.found == 0;
  let mut field = FieldInput::new(
    "conversation-search-field",
    ctx.t("conversation.search.field"),
    search.query.clone(),
  )
  .search()
  .placeholder(ctx.t("conversation.search.placeholder"))
  .width(Dimension::Px(FIELD_WIDTH))
  .controller(search.field.clone());
  if let Some((words, look)) = unit(ctx, typed, count) {
    field = field.unit(words).unit_look(look);
  }
  if nothing {
    field = field.invalid();
  }
  let found = count.found;
  let step = |id: &str, glyph: Icon, label: &str, forward: bool| {
    let button = IconButton::new(id.to_owned(), glyph, ctx.t(label)).small();
    if found == 0 {
      return button;
    }
    let search = search.clone();
    button.on_click(move || search.step(found, forward))
  };
  let previous = step(
    "conversation-search-previous",
    Icon::ChevronUp,
    "conversation.search.previous",
    false,
  );
  let next = step(
    "conversation-search-next",
    Icon::ChevronDown,
    "conversation.search.next",
    true,
  );
  let close = {
    let search = search.clone();
    IconButton::new("conversation-search-close", Icon::X, ctx.t("conversation.search.close"))
      .small()
      .on_click(move || search.close())
  };
  let field = field.render(ctx);
  let row = Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .on_key_down(keys(search, found))
    .child(field)
    .child(previous.render(ctx))
    .child(next.render(ctx))
    .child(
      Row::new()
        .width(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
        .height(Dimension::Px(RULE_HEIGHT))
        .background(PaletteColor::Border),
    )
    .child(close.render(ctx));
  search.take_focus(ctx);
  Column::new()
    .id("conversation-search-bar")
    .describe("found", count.found.to_string())
    .padding(SpacingSize::Xs)
    .spacing(SpacingSize::Xs)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(RadiusSize::Lg)
    .box_shadow(ShadowStyle::Md)
    .child(row)
    .with_children(nothing.then(|| searched(ctx, count.entries)))
    .into()
}

/// What the field says after the query: nothing before anything is typed, *Searching…* while a new query is searched
/// for, *No matches*, or where the current place is among those found.
fn unit(ctx: &Ctx, typed: bool, count: Count) -> Option<(String, UnitLook)> {
  if !typed {
    return None;
  }
  if count.searching {
    return Some((ctx.t("conversation.search.searching").to_string(), UnitLook::Busy));
  }
  let Some(current) = count.current.filter(|_| count.found > 0) else {
    return Some((ctx.t("conversation.search.none").to_string(), UnitLook::Refused));
  };
  let separator = ctx.t("conversation.search.groupSeparator");
  let number = |value: usize| grouped(u64::try_from(value).unwrap_or(u64::MAX), &separator);
  let at = number(current + 1);
  let words = if count.found > COUNTED {
    ctx.t_args(
      "conversation.search.countMany",
      [("at", at.as_str()), ("count", number(COUNTED).as_str())],
    )
  } else {
    ctx.t_args(
      "conversation.search.count",
      [("at", at.as_str()), ("count", number(count.found).as_str())],
    )
  };
  Some((words.to_string(), UnitLook::Count))
}

/// Enter goes to the next place, Shift Enter to the previous one, Esc closes the bar.
fn keys(search: &TranscriptSearch, found: usize) -> impl Fn(KeyboardEvent) + Send + Sync + 'static + use<> {
  let search = search.clone();
  move |event: KeyboardEvent| match event.key.as_str() {
    "Enter" => {
      search.step(found, !event.shift);
      event.prevent_default();
    }
    "Escape" => {
      search.close();
      event.prevent_default();
    }
    _ => {}
  }
}

/// *Searched all 1,240 entries: messages, tool calls, output* (`O51i7`), under a field that found nothing.
fn searched(ctx: &Ctx, entries: usize) -> Element {
  let separator = ctx.t("conversation.search.groupSeparator");
  let entries = grouped(u64::try_from(entries).unwrap_or(u64::MAX), &separator);
  Row::new()
    .id("conversation-search-note")
    .padding(
      Padding::new()
        .top(ExtraSpacing::Tight.role())
        .right(ExtraSpacing::Snug.role())
        .bottom(SpacingSize::Xs)
        .left(ExtraSpacing::Snug.role()),
    )
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&ctx.t_args("conversation.search.searched", [("count", entries.as_str())]))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .into()
}
