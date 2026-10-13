//! The rows of the panel's list: a project's header (`Needs you group` `o3xekF`: its initial in a tile, its
//! name, *this tab's project*, how many wait in it) and an item (`Needs you item` `bAeM6`: what it is and since
//! when it waits, what it asks, its task, where its run stands, and its action), collapsed or, answered in
//! place, expanded (`Item · Approval · expanded` `YDJsC`: the item's own frame in a `warning` stroke around
//! the card that answers it). An item that no longer waits keeps its place, quiet, until the panel closes.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::tasks::needs_you::WaitingItem;

use super::words;
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, NeedsYouRadius, NeedsYouText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    icon_button::IconButton,
    unavailable::reason_line,
  },
};

/// A project's initial tile (`n4MhD`).
const INITIAL_TILE: f32 = 20.0;

type Press = Arc<dyn Fn() + Send + Sync>;

/// A project's header: its initial, its name, whether the active tab shows it, and how many wait in it.
pub(super) fn group(ctx: &Ctx, id: &str, (name, this_tab): (&str, bool), count: usize) -> Row {
  let initial: String = name
    .chars()
    .next()
    .map(|first| first.to_uppercase().collect())
    .unwrap_or_default();
  let tile = Row::new()
    .width(Dimension::Px(INITIAL_TILE))
    .height(Dimension::Px(INITIAL_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(NeedsYouRadius::Initial.role())
    .child(Text::new(&initial).variant(NeedsYouText::Initial.role()));
  let note = this_tab.then(|| {
    Text::new(&ctx.t("needs_you.thisTab"))
      .id(format!("{id}-this-tab"))
      .variant(TypographyStyle::Caption)
      .nowrap()
  });
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Md)
        .left(SpacingSize::Md)
        .right(SpacingSize::Md)
        .bottom(SpacingSize::Xs),
    )
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(tile)
    .child(
      Text::new(name)
        .id(format!("{id}-name"))
        .variant(NeedsYouText::Project.role())
        .nowrap(),
    )
    .with_children(note)
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id(format!("{id}-count"))
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(icon(Icon::Hand, IconSize::Sm, PaletteColor::Warning))
        .child(Text::new(&count.to_string()).variant(NeedsYouText::Tally.role())),
    )
}

/// What it is and since when it waits: *Approval · waiting 6 min*.
fn top(ctx: &Ctx, id: &str, item: &WaitingItem, clock: bool) -> Row {
  let (glyph, word) = words::kind(ctx, &item.kind);
  Row::new()
    .id(format!("{id}-top"))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::Warning))
    .child(
      Text::new(&word)
        .id(format!("{id}-kind"))
        .variant(NeedsYouText::Kind.role())
        .nowrap(),
    )
    .child(Text::new(&ctx.t("needs_you.separator")).variant(ExtraTypography::Note.role()))
    .child(
      Text::new(&words::waiting(ctx, item.since, clock))
        .id(format!("{id}-waiting"))
        .variant(ExtraTypography::Note.role())
        .nowrap(),
    )
}

/// What it asks and its task, 3 apart (`NczSN`, `uTGip`).
fn ask(ctx: &Ctx, id: &str, item: &WaitingItem) -> [Element; 2] {
  [
    Text::new(&words::title(ctx, item))
      .id(format!("{id}-title"))
      .variant(NeedsYouText::ItemTitle.role())
      .width(Dimension::Pct(100.0))
      .into(),
    Text::new(&words::task(ctx, item))
      .id(format!("{id}-task"))
      .variant(NeedsYouText::Task.role())
      .width(Dimension::Pct(100.0))
      .into(),
  ]
}

/// What a collapsed item's action does: `Press` answers a press, with its glyph; `Unavailable` cannot be
/// pressed, for its reason, which shows under the item's task.
pub(super) enum Action {
  Press(Icon, Press),
  Unavailable(Arc<str>),
}

/// The item collapsed: its body, and its action at the end.
pub(super) fn collapsed(ctx: &Ctx, id: &str, item: &WaitingItem, action: Action) -> Element {
  let place = words::place(ctx, item).map(|place| -> Element {
    Text::new(&place)
      .id(format!("{id}-where"))
      .variant(TypographyStyle::Caption)
      .width(Dimension::Pct(100.0))
      .into()
  });
  let button = ActionButton::new(
    format!("{id}-action"),
    ButtonKind::Secondary,
    words::action(ctx, &item.kind),
  );
  let (button, reason) = match action {
    Action::Press(glyph, press) => (button.icon(glyph).on_click(move || press()), None),
    Action::Unavailable(reason) => {
      let line: Element = reason_line(&format!("{id}-reason"), &reason).into();
      (button.unavailable(reason), Some(line))
    }
  };
  let body = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(top(ctx, id, item, false))
    .with_children(ask(ctx, id, item))
    .with_children(place)
    .with_children(reason);
  Row::new()
    .id(id.to_owned())
    .describe("state", "collapsed")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Md, SpacingSize::Sm))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .rounded(ExtraRadius::Tile.role())
    .child(body)
    .child(Row::new().flex_shrink(0.0).child(button))
    .into()
}

/// The item expanded around `card`, which answers it: in a `warning` stroke while it `waits`, quiet once it no
/// longer does; *Collapse* folds it, *Open conversation* opens its card in the task's conversation.
pub(super) fn expanded(
  ctx: &mut Ctx,
  id: &str,
  (item, waits): (&WaitingItem, bool),
  (collapse, conversation): (Press, Press),
  card: Element,
) -> Element {
  let collapse = IconButton::new(
    format!("{id}-collapse"),
    Icon::ChevronUp,
    ctx.t("needs_you.action.collapse"),
  )
  .small()
  .on_click(move || collapse())
  .render(ctx);
  let open = ActionButton::new(
    format!("{id}-conversation"),
    ButtonKind::Ghost,
    ctx.t("needs_you.action.openConversation"),
  )
  .icon(Icon::ArrowUpRight)
  .on_click(move || conversation());
  let frame = Column::new()
    .id(id.to_owned())
    .describe("state", if waits { "expanded" } else { "settled" })
    .width(Dimension::Pct(100.0))
    .padding(SpacingSize::Md)
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfacePanel)
    .rounded(ExtraRadius::Tile.role());
  let frame = if waits {
    frame.border(Border::inside(BorderSize::Sm, PaletteColor::Warning))
  } else {
    frame
  };
  frame
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .align_items(Alignment::Center)
        .child(top(ctx, id, item, true))
        .child(Spacer::new().flex(1.0))
        .child(collapse),
    )
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .with_children(ask(ctx, id, item)),
    )
    .child(card)
    .child(Row::new().child(open))
    .into()
}
