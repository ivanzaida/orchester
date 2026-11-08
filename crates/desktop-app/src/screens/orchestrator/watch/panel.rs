//! What the /watch pickers' panels share (`R1s1o` in 02 `c8pALl`, `gumeP` in 03 `mQdxa`, `BayQR` and `mCXhw` in
//! `w1Euu`), as the @ menu draws its rows: a `Menu` panel 560 wide; a head with its title in 11/500 `text_muted` and
//! its count at its end; 28 high rows, 8 at their sides and between their parts; at a row's end *Added* in
//! `text_secondary` with a 14 `check` once it is a chip, *Led in «…» ↗* with a 12 `messages-square`, which opens that
//! conversation, and the `Kbd` *Enter* while it has the focus, on `hover`; then the keys. While the catalog is read the
//! head shows over three bone rows; a catalog that cannot be read says so in the picker, with *Try again*.

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
use orchester_core::id::{ChatId, ProjectId};
use orchester_workspace::messages::Message;

use crate::{
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  theme::{ControlHeight, ExtraSpacing, ExtraTypography, IconSize, SlashText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    kbd::key_cap,
    menu::menu_panel,
    message::message_text,
    pressable::Press,
    skeleton::bone,
    slash::matched,
  },
};

/// The panel's width (`R1s1o`, `gumeP`).
const WIDTH: f32 = 560.0;
/// A bone row's icon, title and meta bones (`Mjy9O`, `h8p13`, `RgJYQ` in `BayQR`): the title's and the meta's widths
/// row by row, and their heights.
const ICON_BONE: f32 = 14.0;
const BONE_WIDTHS: [(f32, f32); 3] = [(96.0, 180.0), (64.0, 120.0), (110.0, 150.0)];
const TITLE_BONE: f32 = 10.0;
const META_BONE: f32 = 8.0;

/// Which picker a panel is: it names the panel's elements and its head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Picker {
  Module,
  Task,
}

impl Picker {
  /// What the picker's elements are named with, after the composer's id.
  pub(super) fn prefix(self) -> &'static str {
    match self {
      Self::Module => "watch-module",
      Self::Task => "watch-task",
    }
  }

  fn title(self, ctx: &Ctx) -> Arc<str> {
    match self {
      Self::Module => ctx.t("orchestrator.watch.modules.section"),
      Self::Task => ctx.t("orchestrator.watch.tasks.section"),
    }
  }
}

/// The panel named after composer `id`, holding `content`.
pub(super) fn frame((id, picker): (&str, Picker), content: Vec<Element>) -> Element {
  menu_panel(&format!("{id}-{}-picker", picker.prefix()))
    .width(Dimension::Px(WIDTH))
    .with_children(content)
    .into()
}

/// A row's frame: 28 high, 8 at its sides and between its parts.
pub(in crate::screens::orchestrator) fn line() -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Sm)
}

/// How many rows the head counts: all of them while nothing is typed, else *shown of total*.
pub(super) fn count(ctx: &Ctx, filter: &str, (shown, total): (usize, usize)) -> Arc<str> {
  if filter.is_empty() {
    return Arc::from(shown.to_string());
  }
  ctx.t_args(
    "orchestrator.watch.picker.count",
    [("shown", shown.to_string()), ("total", total.to_string())],
  )
}

/// The picker's head: its title, or `words` for a later section, `name`; with `end` at its end.
pub(super) fn heading(
  ctx: &Ctx,
  (id, picker, name): (&str, Picker, &str),
  words: Option<Arc<str>>,
  end: Option<Arc<str>>,
) -> Element {
  let prefix = picker.prefix();
  Row::new()
    .id(format!("{id}-{prefix}-{name}"))
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
      Text::new(&words.unwrap_or_else(|| picker.title(ctx)))
        .variant(SlashText::Section.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id(format!("{id}-{prefix}-{name}-count"))
        .variant(TypographyStyle::Caption)
        .nowrap()
    }))
    .into()
}

/// While the catalog is read: the head over three bone rows, and *Esc Close*.
pub(super) fn reading(ctx: &Ctx, (id, picker): (&str, Picker)) -> Element {
  let mut content = vec![heading(ctx, (id, picker, "section"), None, None)];
  content.extend(BONE_WIDTHS.iter().map(|(title, meta)| {
    line()
      .child(bone(ICON_BONE, ICON_BONE))
      .child(bone(*title, TITLE_BONE))
      .child(bone(*meta, META_BONE))
      .into()
  }));
  content.push(footer(ctx, (id, picker), false));
  frame((id, picker), content)
}

/// The catalog cannot be read: the danger mark, which project's tasks, `why` and *Try again*, which reads it again.
pub(super) fn unreadable(
  ctx: &Ctx,
  (id, picker, project, why): (&str, Picker, &str, &Message),
  retry: Arc<dyn Fn() + Send + Sync>,
) -> Element {
  let prefix = picker.prefix();
  let reason = message_text(ctx, why);
  let again = ActionButton::new(
    format!("{id}-{prefix}-retry"),
    ButtonKind::Ghost,
    ctx.t("orchestrator.watch.picker.tryAgain"),
  )
  .on_click(move || retry());
  let words = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&ctx.t_args("orchestrator.watch.picker.unreadable", [("project", project)]))
        .id(format!("{id}-{prefix}-unreadable-title"))
        .variant(TypographyStyle::Label),
    )
    .child(
      Text::new(&ctx.t_args("orchestrator.watch.picker.unreadableNote", [("reason", reason)]))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0)),
    );
  let unreadable = Row::new()
    .id(format!("{id}-{prefix}-unreadable"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
    .child(words)
    .child(again);
  let content = vec![
    heading(ctx, (id, picker, "section"), None, None),
    unreadable.into(),
    footer(ctx, (id, picker), false),
  ];
  frame((id, picker), content)
}

/// Nothing matches what was typed: `words`, named `{id}-{prefix}-nothing`.
pub(super) fn nothing((id, picker): (&str, Picker), words: &str) -> Element {
  line()
    .id(format!("{id}-{}-nothing", picker.prefix()))
    .child(
      Text::new(words)
        .variant(TypographyStyle::Caption)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .into()
}

/// `label` with the letters that match `filter` in 600 `text_primary`: a name in 12 `text_primary`, or a task's key
/// in Geist Mono 12 `text_secondary` (`mono`).
pub(in crate::screens::orchestrator) fn matched_label(label: &str, filter: &str, mono: bool) -> Element {
  let letters: Range<usize> = matched(label, filter).unwrap_or(0..0);
  let piece = |range: Range<usize>, bold: bool| {
    let words = &label[range];
    (!words.is_empty()).then(|| {
      let text = Text::new(&words.replace(' ', "\u{a0}")).nowrap();
      match (mono, bold) {
        (false, false) => text.variant(SlashText::Label.role()),
        (false, true) => text.variant(SlashText::Match.role()),
        (true, false) => text
          .variant(ExtraTypography::PropertyMono.role())
          .color(PaletteColor::TextSecondary),
        (true, true) => text.variant(SlashText::SkillMatch.role()),
      }
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

/// A note at a row's end in 11 `color`.
pub(super) fn caption(words: &str, color: PaletteColor) -> Text {
  Text::new(words)
    .variant(TypographyStyle::Caption)
    .color(color)
    .flex_shrink(0.0)
    .nowrap()
}

/// *Added* in `text_secondary` with its 14 `check`: the row is a chip already.
pub(super) fn added(ctx: &Ctx) -> Element {
  Row::new()
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(caption(
      &ctx.t("orchestrator.watch.picker.added"),
      PaletteColor::TextSecondary,
    ))
    .child(icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary))
    .into()
}

/// Where a *Led in* opens, and what names it.
pub(super) struct LedIn<'a> {
  /// The element's id.
  pub(super) id: String,
  pub(super) chat: ChatId,
  /// That conversation's title; `None` when the project has no conversation with its id now.
  pub(super) title: Option<&'a str>,
  pub(super) project: ProjectId,
  pub(super) navigation: &'a Navigation,
}

/// *Led in «title» ↗* in `color`, which opens the conversation that leads the row.
pub(super) fn led_in(ctx: &Ctx, led: LedIn<'_>, color: PaletteColor) -> Element {
  let words = match led.title {
    Some(title) => ctx.t_args("orchestrator.watch.picker.ledIn", [("title", title)]),
    None => ctx.t("orchestrator.watch.picker.ledInUnnamed"),
  };
  let route = Route::Project(led.project, ProjectPage::Orchestrator(OrchestratorChat::Chat(led.chat)));
  Press::Open(led.navigation.to(route))
    .button()
    .id(led.id)
    .describe("chat", led.chat.to_string())
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::MessagesSquare, IconSize::Sm, color.clone()))
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

/// The keys (`Keys` `T0Gkl`, `HXrVq`): *↑ ↓ Move*, *Enter Add*, *Backspace Remove the last* and
/// *Esc Close* while there are rows (`rows`), *Esc Close* alone otherwise.
pub(super) fn footer(ctx: &Ctx, (id, picker): (&str, Picker), rows: bool) -> Element {
  let prefix = picker.prefix();
  let hints: &[(&str, &str)] = if rows {
    &[
      ("move", "move"),
      ("enter", "add"),
      ("backspace", "removeLast"),
      ("esc", "close"),
    ]
  } else {
    &[("esc", "close")]
  };
  keys(ctx, (&format!("{id}-{prefix}"), &format!("{prefix}-key")), hints, None)
}

/// A picker's keys under a `border` line, named `{id}-keys`: each cap (`orchestrator.watch.keys.*`, named `cap_id`)
/// with its words (`orchestrator.watch.hint.*`), and `note` at the end in 11 `text_muted`.
pub(in crate::screens::orchestrator) fn keys(
  ctx: &Ctx,
  (id, cap_id): (&str, &str),
  hints: &[(&str, &str)],
  note: Option<Arc<str>>,
) -> Element {
  let hint = |(cap, label): &(&str, &str)| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(key_cap(cap_id, &ctx.t(&format!("orchestrator.watch.keys.{cap}"))))
      .child(
        Text::new(&ctx.t(&format!("orchestrator.watch.hint.{label}")))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      )
  };
  let note: Vec<Element> = note.map_or_else(Vec::new, |note| {
    vec![
      Spacer::new().flex(1.0).into(),
      Text::new(&note)
        .id(format!("{id}-keys-note"))
        .variant(TypographyStyle::Caption)
        .nowrap()
        .into(),
    ]
  });
  Row::new()
    .id(format!("{id}-keys"))
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
    .with_children(hints.iter().map(hint))
    .with_children(note)
    .into()
}

/// *Enter* at the focused row's end.
pub(super) fn enter(ctx: &Ctx, picker: Picker) -> Element {
  key_cap(
    &format!("{}-enter", picker.prefix()),
    &ctx.t("orchestrator.watch.keys.enter"),
  )
}
