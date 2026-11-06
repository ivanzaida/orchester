//! The /watch-module picker's panel (`Module picker` `R1s1o` in 02 `c8pALl`), as the @ menu draws its rows: a `Menu`
//! panel 560 wide. Under the head *Modules*, with how many modules it shows at its end (*2 of 4* once something is
//! typed), a 28 high row for each: a 14 `boxes` in `text_secondary`, the name in 12 `text_primary` with the letters
//! that match what was typed in 600, what it takes in in 11 `text_muted` (*12 tasks · 2 running · 1 waits for you*),
//! and at its end *Added* in `text_secondary` with a 14 `check` once it is a chip, *Led in «…» ↗* with a 12
//! `messages-square` in `warning` when another conversation leads it, which opens that conversation, and the `Kbd`
//! *Enter* while it has the focus, on `hover`. A module with nothing left to watch is faded to half and says so. Under
//! the head *Also* the tasks without a module (`inbox`) and everything in the project (`folder-kanban`, *What no other
//! subscription takes*, its *Led in* in `text_muted`); then the keys. While the catalog is read the head shows over
//! three bone rows (`BayQR` in `w1Euu`); a catalog that cannot be read says so in the picker, with *Try again*
//! (`mCXhw`).

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{WatchTarget, watch::EndReason},
};
use orchester_workspace::{
  mentions::watch::{Leader, modules::ModuleEntry},
  messages::Message,
};

use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, SlashText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    kbd::key_cap,
    menu::menu_panel,
    message::message_text,
    pressable::Press,
    skeleton::bone,
    slash::{Choice, matched},
  },
};

/// The panel's width (`R1s1o`).
const WIDTH: f32 = 560.0;
/// A bone row's icon, title and meta bones (`Mjy9O`, `h8p13`, `RgJYQ` in `BayQR`): the title's and the meta's widths
/// row by row, and their heights.
const ICON_BONE: f32 = 14.0;
const BONE_WIDTHS: [(f32, f32); 3] = [(96.0, 180.0), (64.0, 120.0), (110.0, 150.0)];
const TITLE_BONE: f32 = 10.0;
const META_BONE: f32 = 8.0;

/// What the picker shows now.
pub(super) struct Shown<'a> {
  /// What names the composer: the picker's elements are named after it.
  pub(super) id: &'a str,
  pub(super) project: ProjectId,
  /// The words typed after the chips.
  pub(super) filter: &'a str,
  /// Its rows, in order, with the chip each becomes.
  pub(super) offered: &'a [(&'a ModuleEntry, Choice)],
  pub(super) focused: Option<usize>,
  /// How many open modules the project has.
  pub(super) total: usize,
}

/// The panel with its rows and the keys.
pub(super) fn panel(ctx: &Ctx, shown: &Shown<'_>) -> Element {
  let modules = shown
    .offered
    .iter()
    .filter(|(entry, _)| matches!(entry.target, WatchTarget::Module(_)))
    .count();
  let count = if shown.filter.is_empty() {
    Arc::from(modules.to_string())
  } else {
    ctx.t_args(
      "orchestrator.watch.modules.count",
      [("shown", modules.to_string()), ("total", shown.total.to_string())],
    )
  };
  let mut content = vec![heading(ctx, (shown.id, "section"), Some(count))];
  if modules == 0 && !shown.filter.is_empty() {
    content.push(nothing(ctx, shown));
  }
  for at in 0..shown.offered.len() {
    if at == modules {
      content.push(heading(ctx, (shown.id, "also"), None));
    }
    content.push(row(ctx, shown, at));
  }
  content.push(footer(ctx, shown.id, true));
  frame(shown.id, content)
}

/// While the catalog is read: the head over three bone rows, and *Esc Close*.
pub(super) fn reading(ctx: &Ctx, id: &str) -> Element {
  let mut content = vec![heading(ctx, (id, "section"), None)];
  content.extend(BONE_WIDTHS.iter().map(|(title, meta)| {
    line()
      .child(bone(ICON_BONE, ICON_BONE))
      .child(bone(*title, TITLE_BONE))
      .child(bone(*meta, META_BONE))
      .into()
  }));
  content.push(footer(ctx, id, false));
  frame(id, content)
}

/// The catalog cannot be read: the danger mark, which project's tasks, `why` and *Try again*, which reads it again.
pub(super) fn unreadable(
  ctx: &Ctx,
  (id, project, why): (&str, &str, &Message),
  retry: Arc<dyn Fn() + Send + Sync>,
) -> Element {
  let reason = message_text(ctx, why);
  let again = ActionButton::new(
    format!("{id}-watch-module-retry"),
    ButtonKind::Ghost,
    ctx.t("orchestrator.watch.modules.tryAgain"),
  )
  .on_click(move || retry());
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&ctx.t_args("orchestrator.watch.modules.unreadable", [("project", project)]))
        .id(format!("{id}-watch-module-unreadable-title"))
        .variant(ExtraTypography::NoticeHeading.role()),
    )
    .child(
      Text::new(&ctx.t_args("orchestrator.watch.modules.unreadableNote", [("reason", reason)]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0)),
    );
  let unreadable = Row::new()
    .id(format!("{id}-watch-module-unreadable"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
    .child(words)
    .child(again);
  let content = vec![
    heading(ctx, (id, "section"), None),
    unreadable.into(),
    footer(ctx, id, false),
  ];
  frame(id, content)
}

fn frame(id: &str, content: Vec<Element>) -> Element {
  menu_panel(&format!("{id}-watch-module-picker"))
    .width(Dimension::Px(WIDTH))
    .with_children(content)
    .into()
}

/// A row's frame: 28 high, 8 at its sides and between its parts.
fn line() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
}

/// A section's head, *Modules* or *Also* (`name`), with `end` at its end.
fn heading(ctx: &Ctx, (id, name): (&str, &str), end: Option<Arc<str>>) -> Element {
  Row::new()
    .id(format!("{id}-watch-module-{name}"))
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
      Text::new(&ctx.t(&format!("orchestrator.watch.modules.{name}")))
        .variant(SlashText::Section.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id(format!("{id}-watch-module-{name}-count"))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
    .into()
}

/// No module's name holds what was typed.
fn nothing(ctx: &Ctx, shown: &Shown<'_>) -> Element {
  line()
    .id(format!("{}-watch-module-nothing", shown.id))
    .child(
      Text::new(&ctx.t_args("orchestrator.watch.modules.nothing", [("query", shown.filter)]))
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

/// Row `at`: its icon, name, what it takes in and how it stands, *Enter* while it has the focus.
fn row(ctx: &Ctx, shown: &Shown<'_>, at: usize) -> Element {
  let (entry, choice) = &shown.offered[at];
  let focused = shown.focused == Some(at);
  let mut children: Vec<Element> = vec![
    icon(choice.icon, IconSize::Md, PaletteColor::TextSecondary).into(),
    name(&choice.label, shown.filter),
    Text::new(&meta(ctx, entry))
      .variant(TypographyStyle::Caption)
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .into(),
  ];
  children.extend(state(ctx, shown, (entry, &choice.key)));
  if focused {
    children.push(key_cap(
      "watch-module-enter",
      &ctx.t("orchestrator.watch.keys.enter"),
    ));
  }
  let row = line()
    .id(format!("{}-watch-module-{}", shown.id, choice.key))
    .describe("a11y_name", choice.label.clone())
    .describe("focused", focused.to_string())
    .describe("state", state_name(entry))
    .with_children(children);
  let row = if focused {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  if entry.faded.is_some() {
    row.opacity(DISABLED_OPACITY).into()
  } else {
    row.into()
  }
}

/// How a row stands, for tools and tests.
fn state_name(entry: &ModuleEntry) -> &'static str {
  match (&entry.faded, &entry.leader) {
    (Some(_), _) => "nothing-left",
    (None, Leader::JustAdded) => "added",
    (None, Leader::Elsewhere { .. }) => "led-elsewhere",
    (None, Leader::Here { .. }) => "led-here",
    (None, Leader::Nobody) => "free",
  }
}

/// The name in 12 `text_primary`; once something is typed, the letters it matches in 600.
fn name(label: &str, filter: &str) -> Element {
  let letters: Range<usize> = matched(label, filter).unwrap_or(0..0);
  let piece = |range: Range<usize>, bold: bool| {
    let words = &label[range];
    (!words.is_empty()).then(|| {
      let style = if bold { SlashText::Match } else { SlashText::Label };
      Text::new(&words.replace(' ', "\u{a0}"))
        .variant(style.role())
        .nowrap()
    })
  };
  Row::new()
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .with_children(piece(0..letters.start, false))
    .with_children(piece(letters.clone(), true))
    .with_children(piece(letters.end..label.len(), false))
    .into()
}

/// What the row's target takes in: its tasks, how many run and wait for the person; *all done* for a module with
/// nothing left to watch; what the whole project's subscription means.
fn meta(ctx: &Ctx, entry: &ModuleEntry) -> Arc<str> {
  if entry.target == WatchTarget::Project {
    return ctx.t("orchestrator.watch.modules.projectMeta");
  }
  let counts = entry.counts;
  let counted = |key: &str, count: usize| {
    let form = if count == 1 { "one" } else { "other" };
    ctx.t_args(
      &format!("orchestrator.watch.modules.{key}.{form}"),
      [("count", count.to_string())],
    )
  };
  let tasks = counted("tasks", counts.tasks);
  match entry.faded {
    Some(EndReason::ModuleClosed { canceled, .. }) if canceled > 0 => {
      return ctx.t_args("orchestrator.watch.modules.allClosed", [("tasks", tasks)]);
    }
    Some(_) => return ctx.t_args("orchestrator.watch.modules.allDone", [("tasks", tasks)]),
    None => {}
  }
  let running = if counts.running == 0 {
    ctx.t("orchestrator.watch.modules.running.none")
  } else {
    ctx.t_args(
      "orchestrator.watch.modules.running.some",
      [("count", counts.running.to_string())],
    )
  };
  if counts.waiting == 0 {
    return ctx.t_args(
      "orchestrator.watch.modules.meta",
      [("tasks", tasks), ("running", running)],
    );
  }
  let waiting = counted("waiting", counts.waiting);
  ctx.t_args(
    "orchestrator.watch.modules.metaWaiting",
    [("tasks", tasks), ("running", running), ("waiting", waiting)],
  )
}

/// What the row's end says of it: nothing left to watch, *Added* with its check, *Led in «…» ↗* or *Watched here*.
fn state(ctx: &Ctx, shown: &Shown<'_>, (entry, key): (&ModuleEntry, &str)) -> Option<Element> {
  let caption = |key: &str, color: PaletteColor| {
    Text::new(&ctx.t(&format!("orchestrator.watch.modules.{key}")))
      .variant(TypographyStyle::Caption)
      .color(color)
      .flex_shrink(0.0)
      .nowrap()
  };
  if entry.faded.is_some() {
    return Some(caption("nothingLeft", PaletteColor::TextMuted).into());
  }
  match &entry.leader {
    Leader::Nobody => None,
    Leader::Here { .. } => Some(caption("ledHere", PaletteColor::TextMuted).into()),
    Leader::JustAdded => Some(
      Row::new()
        .flex_shrink(0.0)
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(caption("added", PaletteColor::TextSecondary))
        .child(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary))
        .into(),
    ),
    Leader::Elsewhere { chat, title, .. } => Some(led_in(ctx, shown, (entry, key), (*chat, title.as_deref()))),
  }
}

/// *Led in «title» ↗*, which opens conversation `chat`: in `warning`, a conflict for the command, except on the whole
/// project, which every more specific subscription takes from.
fn led_in(ctx: &Ctx, shown: &Shown<'_>, (entry, key): (&ModuleEntry, &str), (chat, title): (ChatId, Option<&str>)) -> Element {
  let color = if entry.target == WatchTarget::Project {
    PaletteColor::TextMuted
  } else {
    PaletteColor::Warning
  };
  let words = match title {
    Some(title) => ctx.t_args("orchestrator.watch.modules.ledIn", [("title", title)]),
    None => ctx.t("orchestrator.watch.modules.ledInUnnamed"),
  };
  let route = Route::Project(shown.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)));
  Press::Open(Navigation::of(ctx).to(route))
    .button()
    .id(format!("{}-watch-module-{key}-led-in", shown.id))
    .describe("chat", chat.to_string())
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::MessagesSquare, IconSize::Sm, color))
    .child(
      Text::new(&words)
        .variant(TypographyStyle::Caption)
        .color(color)
        .min_width(Dimension::Px(0.0))
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

/// The keys (`Keys` `T0Gkl`): *↑ ↓ Move*, *Enter Add*, *Backspace Remove the last*, *Ctrl Enter Watch* and *Esc
/// Close* while there are rows (`rows`), *Esc Close* alone otherwise.
fn footer(ctx: &Ctx, id: &str, rows: bool) -> Element {
  let hint = |cap: &str, label: &str| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(key_cap(
        "watch-module-key",
        &ctx.t(&format!("orchestrator.watch.keys.{cap}")),
      ))
      .child(
        Text::new(&ctx.t(&format!("orchestrator.watch.hint.{label}")))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      )
  };
  let watch = if cfg!(target_os = "macos") {
    "cmdEnter"
  } else {
    "ctrlEnter"
  };
  let hints = if rows {
    vec![
      hint("move", "move"),
      hint("enter", "add"),
      hint("backspace", "removeLast"),
      hint(watch, "watch"),
      hint("esc", "close"),
    ]
  } else {
    vec![hint("esc", "close")]
  };
  Row::new()
    .id(format!("{id}-watch-module-keys"))
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .right(SpacingSize::Sm)
        .bottom(SpacingSize::Xs)
        .left(SpacingSize::Sm),
    )
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .with_children(hints)
    .into()
}
