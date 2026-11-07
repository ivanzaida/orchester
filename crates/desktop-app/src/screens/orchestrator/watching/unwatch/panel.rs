//! The `/unwatch` picker's panel (`Unwatch picker` `eK1DB` in `Parts · /unwatch` `NtsXc`; v2 as `r7kka` in `P1`
//! `QXPDY`, ORC-889), drawn as the /watch pickers draw their rows (`super::super::super::watch::panel`): a `Menu` panel
//! 560 wide. At its top its own search field, *Search what this conversation watches*; under the head *Watched here*
//! with how many subscriptions it shows at its end (*1 of 3* once something is typed), a 28 high row for each in a
//! list that scrolls after 8: its 14 icon in `text_secondary`, its name in 12 `text_primary` with the letters that
//! match what was typed in 600, its kind and since when in 11 `text_muted` filling the row, and at its end what is
//! still open there in 11: in `warning` while something waits for the person or is stopped at a limit, `text_muted`
//! otherwise, *nothing open* when nothing is. A row that is a chip already adds *Added* with its check. Then a `border`
//! line and *All of them* (`eye-off`, *stop watching everything here*, *cancels nothing*), and the keys. The focused
//! row is on `hover`.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  messages::Message,
  orchestrator::watching::{Going, WatchedSubscription},
};

use super::super::answer::{since, target_name};
use crate::{
  screens::orchestrator::watch::panel::{PICK_KEYS, keys, line, list, matched_label},
  theme::{ExtraColor, ExtraSpacing, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    menu::{menu_panel, separator},
    message::message_text,
    slash::{Choice, PickerSearch},
  },
};

/// The panel's width, as the other pickers in a window (`docs/design/orchestrator-watch.md`, *Pickers*).
const WIDTH: f32 = 560.0;

/// What the picker shows now.
pub(super) struct Shown<'a> {
  /// What names the composer: the picker's elements are named after it.
  pub(super) id: &'a str,
  /// The words typed in the picker's search.
  pub(super) filter: &'a str,
  /// The subscriptions whose name holds them, in order, with the chip each becomes.
  pub(super) offered: &'a [(&'a WatchedSubscription, Choice)],
  /// *All of them*, the row after them.
  pub(super) all: &'a Choice,
  /// The keys of the choices that are chips already.
  pub(super) taken: &'a [Arc<str>],
  /// The focused row among the subscriptions and then *All of them*.
  pub(super) focused: Option<usize>,
  /// How many subscriptions the conversation has.
  pub(super) total: usize,
}

/// The panel with its search, its rows, *All of them* and the keys.
pub(super) fn panel(ctx: &mut Ctx, shown: &Shown<'_>, search: &PickerSearch) -> Element {
  let count = if shown.filter.is_empty() {
    Arc::from(shown.total.to_string())
  } else {
    ctx.t_args(
      "orchestrator.watch.unwatch.count",
      [
        ("shown", shown.offered.len().to_string()),
        ("total", shown.total.to_string()),
      ],
    )
  };
  let mut content = vec![search_field(ctx, shown.id, search), heading(ctx, shown.id, Some(count))];
  if shown.offered.is_empty() {
    content.push(said(
      &format!("{}-unwatch-nothing", shown.id),
      &ctx.t_args("orchestrator.watch.unwatch.nothing", [("query", shown.filter)]),
    ));
  }
  let rows: Vec<Element> = shown
    .offered
    .iter()
    .enumerate()
    .map(|(at, (watched, choice))| {
      let (open, warn) = open_work(ctx, watched);
      let end = Text::new(&open)
        .id(format!("{}-unwatch-{}-open", shown.id, choice.key))
        .variant(TypographyStyle::Caption)
        .color(if warn {
          PaletteColor::Warning
        } else {
          PaletteColor::TextMuted
        })
        .flex_shrink(0.0)
        .nowrap();
      let meta = ctx.t_args(
        "orchestrator.watch.unwatch.meta",
        [
          ("kind", message_text(ctx, &watched.target.kind)),
          ("since", since(ctx, watched.since)),
        ],
      );
      let row = row(ctx, (shown, search), (at, choice), (&meta, end.into()));
      row.describe("target", target_name(watched.target.target)).into()
    })
    .collect();
  if !rows.is_empty() {
    content.push(list(ctx, &format!("{}-unwatch-list", shown.id), rows));
  }
  content.push(separator(ctx));
  let cancels = Text::new(&ctx.t("orchestrator.watch.unwatch.cancelsNothing"))
    .variant(TypographyStyle::Caption)
    .flex_shrink(0.0)
    .nowrap();
  let all_meta = ctx.t("orchestrator.watch.unwatch.allMeta");
  let all = row(
    ctx,
    (shown, search),
    (shown.offered.len(), shown.all),
    (&all_meta, cancels.into()),
  );
  content.push(all.into());
  content.push(footer(ctx, shown.id));
  frame(shown.id, content)
}

/// What the conversation watches cannot be shown: the search, the head, `why` in 11 `text_muted`, and the keys.
pub(super) fn unshown(ctx: &mut Ctx, (id, search): (&str, &PickerSearch), why: Option<&Message>) -> Element {
  let mut content = vec![search_field(ctx, id, search), heading(ctx, id, None)];
  let words = match why {
    Some(why) => ctx.t_args(
      "orchestrator.watch.unwatch.unreadable",
      [("reason", message_text(ctx, why))],
    ),
    None => ctx.t("orchestrator.watch.unwatch.reading"),
  };
  content.push(said(&format!("{id}-unwatch-unshown"), &words));
  content.push(footer(ctx, id));
  frame(id, content)
}

fn frame(id: &str, content: Vec<Element>) -> Element {
  menu_panel(&format!("{id}-unwatch-picker"))
    .width(Dimension::Px(WIDTH))
    .with_children(content)
    .into()
}

/// The picker's own search field, `{id}-unwatch-search`.
fn search_field(ctx: &mut Ctx, id: &str, search: &PickerSearch) -> Element {
  let words = ctx.t("orchestrator.watch.unwatch.search");
  search.field(ctx, &format!("{id}-unwatch-search"), (words, None))
}

/// The head *Watched here*, with `count` at its end.
fn heading(ctx: &Ctx, id: &str, count: Option<Arc<str>>) -> Element {
  Row::new()
    .id(format!("{id}-unwatch-section"))
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
    .child(
      Text::new(&ctx.t("orchestrator.watch.unwatch.section"))
        .variant(SlashText::Section.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(count.map(|count| {
      Text::new(&count)
        .id(format!("{id}-unwatch-section-count"))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
    .into()
}

/// A row named `id` that only says `words`, in 11 `text_muted`.
fn said(id: &str, words: &str) -> Element {
  line()
    .id(id.to_owned())
    .child(
      Text::new(words)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

/// Row `at` of `choice`: its icon, name, `meta` and `end`; *Added* with its check once it is a chip; on `hover` while
/// it has the focus, which `search` keeps in view.
fn row(
  ctx: &Ctx,
  (shown, search): (&Shown<'_>, &PickerSearch),
  (at, choice): (usize, &Choice),
  (meta, end): (&str, Element),
) -> Row {
  let focused = shown.focused == Some(at);
  let added = shown.taken.contains(&choice.key);
  let added = added.then(|| {
    Row::new()
      .flex_shrink(0.0)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("orchestrator.watch.unwatch.added"))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextSecondary)
          .nowrap(),
      )
      .child(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary))
  });
  let row = line()
    .id(format!("{}-unwatch-{}", shown.id, choice.key))
    .describe("a11y_name", choice.label.clone())
    .describe("focused", focused.to_string())
    .describe("added", added.is_some().to_string())
    .child(icon(choice.icon, IconSize::Md, PaletteColor::TextSecondary))
    .child(matched_label(&choice.label, shown.filter, false))
    .child(
      Text::new(meta)
        .variant(TypographyStyle::Caption)
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(end)
    .with_children(added);
  if focused {
    search.keep_in_view(row.background(ExtraColor::Hover.role()))
  } else {
    row
  }
}

/// What is still open in `watched`'s tasks (*1 waits for you · 2 running*), and whether it takes the warning tone:
/// something waits for the person or is stopped at a limit.
fn open_work(ctx: &Ctx, watched: &WatchedSubscription) -> (Arc<str>, bool) {
  let count = |of: fn(&Going) -> bool| watched.tasks.iter().filter(|task| of(&task.going)).count();
  let waits = count(|going| matches!(going, Going::Waits(_)));
  let stopped = count(|going| matches!(going, Going::StoppedAtLimit { .. }));
  let parts: Vec<Arc<str>> = [
    ("waits", waits),
    ("stopped", stopped),
    ("readyToMerge", count(|going| *going == Going::ReadyToMerge)),
    ("running", count(|going| *going == Going::Running)),
    ("queued", count(|going| matches!(going, Going::Queued { .. }))),
  ]
  .into_iter()
  .filter(|(_, count)| *count > 0)
  .map(|(key, count)| {
    let form = if count == 1 { "one" } else { "other" };
    ctx.t_args(
      &format!("orchestrator.watch.unwatch.open.{key}.{form}"),
      [("count", count.to_string())],
    )
  })
  .collect();
  let warn = waits + stopped > 0;
  let Some((first, rest)) = parts.split_first() else {
    let none = if watched.rest.is_some() { "idle" } else { "nothing" };
    return (ctx.t(&format!("orchestrator.watch.unwatch.open.{none}")), false);
  };
  let joined = rest.iter().fold(first.clone(), |all, next| {
    ctx.t_args(
      "orchestrator.watch.unwatch.open.join",
      [("rest", all), ("next", next.clone())],
    )
  });
  (joined, warn)
}

/// The keys: *↑ ↓ Move*, *Enter Pick and close*, *Shift Enter Add another*, *Esc Close*.
fn footer(ctx: &Ctx, id: &str) -> Element {
  keys(
    ctx,
    (&format!("{id}-unwatch"), &format!("{id}-unwatch-key")),
    &PICK_KEYS,
  )
}
