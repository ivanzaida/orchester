//! What the @ menu's panel is made of (`@ menu` `qhBhC`): its sections in a fixed order — Tasks (`square-check`, the
//! key in Geist Mono 12 in a 64 column, the title, the workflow status in 11 `text_muted`), Runs (`play`, *KON-12 ·
//! #9* in a 96 column, the task's title in `text_muted`, the state as a status) and Files (`file-code`, the folder in
//! Geist Mono 12 `text_muted` and the name in `text_primary`, the letters that match what was typed in 600, and
//! *changed in this run* in 11 `text_muted` at the end of a file the run changed) — each under its title and count, 28
//! high rows with the focus on `hover` and *Enter* at the focused row's end; a separator between them, and the keys
//! under them. With nothing that matches it says so (`search-x`) and keeps only *Esc Close*. While the files are
//! listed, or when they cannot be, a line says so in their place.

use std::ops::Range;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  mentions::{
    catalog::{Offer, RunChoice, Section, TaskChoice},
    files::FileChoice,
  },
  messages::Message,
};

use super::{Shown, menu::MenuProps};
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, ExtraTypography, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    kbd::key_cap,
    menu::separator,
    message::message_text,
    pressable::pressable,
    status::{Status, StatusChip, workflow_key},
  },
};

/// The column of a task's key (`Key` `A8Rq3`) and of a run's (`Key` `UZe0Z`).
const TASK_KEY_WIDTH: f32 = 64.0;
const RUN_KEY_WIDTH: f32 = 96.0;

/// Where the repository's files stand as the menu read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Files {
  /// Listed: the offer holds the ones that match.
  Listed,
  /// Being listed for the first time.
  Listing,
  Unlisted(Message),
}

/// The sections that match, with the keys under them; or that nothing matches.
pub(super) fn rows(
  ctx: &mut Ctx,
  props: &MenuProps,
  (offer, query, files): (&Offer, &str, &Files),
  focused: usize,
) -> Vec<Element> {
  let note = files_note(ctx, files);
  if offer.is_empty() {
    let Some(note) = note else {
      let title = ctx.t_args("composer.mention.empty.title", [("query", query)]);
      let hint = ctx.t_args("composer.mention.empty.hint", [("project", &*props.project)]);
      return vec![nothing((&title, &hint)), footer(ctx, (offer, query))];
    };
    return vec![note, footer(ctx, (offer, query))];
  }
  let mut rows: Vec<Element> = Vec::new();
  let tasks = offer.tasks.shown.len();
  if tasks > 0 {
    rows.push(heading(ctx, "tasks", &offer.tasks));
    for (index, task) in offer.tasks.shown.iter().enumerate() {
      let face = task_row(ctx, task, index == focused);
      let name = format!("task-{}", task.key);
      rows.push(row(props, (&name, index, index == focused), face));
    }
  }
  let runs = offer.runs.shown.len();
  if runs > 0 {
    if !rows.is_empty() {
      rows.push(separator(ctx));
    }
    rows.push(heading(ctx, "runs", &offer.runs));
    for (index, run) in offer.runs.shown.iter().enumerate() {
      let at = tasks + index;
      let face = run_row(ctx, props, run, at == focused);
      let name = format!("run-{}-{}", run.key, run.number.get());
      rows.push(row(props, (&name, at, at == focused), face));
    }
  }
  if !rows.is_empty() && (!offer.files.shown.is_empty() || note.is_some()) {
    rows.push(separator(ctx));
  }
  if !offer.files.shown.is_empty() {
    rows.push(heading(ctx, "files", &offer.files));
    for (index, file) in offer.files.shown.iter().enumerate() {
      let at = tasks + runs + index;
      let face = file_row(ctx, file, at == focused);
      let name = format!("file-{}", file.path);
      rows.push(row(props, (&name, at, at == focused), face));
    }
  }
  rows.extend(note);
  rows.push(footer(ctx, (offer, query)));
  rows
}

/// What the menu says in the Files section's place while the files are listed or when they cannot be.
fn files_note(ctx: &Ctx, files: &Files) -> Option<Element> {
  let words = match files {
    Files::Listed => return None,
    Files::Listing => ctx.t("composer.mention.files.reading"),
    Files::Unlisted(reason) => {
      let reason = message_text(ctx, reason);
      ctx.t_args("composer.mention.files.unreadable", [("reason", reason)])
    }
  };
  Some(
    Row::new()
      .id("mention-files-note")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(ControlHeight::Md.value()))
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::FileCode, IconSize::Md, PaletteColor::TextMuted))
      .child(
        Text::new(&words)
          .variant(TypographyStyle::Caption)
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .nowrap()
          .text_overflow(TextOverflow::Elipsis),
      )
      .into(),
  )
}

/// A section's title and, at its end, how many rows it shows of how many there are.
fn heading<T>(ctx: &Ctx, name: &str, section: &Section<T>) -> Element {
  let count = if section.shown.len() < section.total {
    ctx.t_args(
      "composer.mention.count",
      [
        ("shown", section.shown.len().to_string()),
        ("total", section.total.to_string()),
      ],
    )
  } else {
    section.total.to_string().into()
  };
  Row::new()
    .id(format!("mention-section-{name}"))
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
      Text::new(&ctx.t(&format!("composer.mention.section.{name}")))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&count)
        .id(format!("mention-section-{name}-count"))
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}

/// Row `index`, which the pointer chooses too: `face` on `hover` while it has the focus (`focused`) or the pointer.
fn row(props: &MenuProps, (name, index, focused): (&str, usize, bool), face: Vec<Element>) -> Element {
  let field = props.field.clone();
  let choose = move || {
    let shown = field.shown.lock().clone();
    if let Some(Shown { replaces, rows }) = shown
      && let Some(mention) = rows.get(index)
    {
      field.insert(replaces, mention);
    }
  };
  let button = pressable(choose)
    .id(format!("{}-mention-{name}", props.id))
    .describe("focused", focused.to_string())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ControlHeight::Md.value()))
    .padding(Padding::horizontal(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Sm)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .with_children(face);
  if focused {
    button.background(ExtraColor::Hover.role()).into()
  } else {
    button.into()
  }
}

/// A task's `square-check`, key, title and workflow status, and *Enter* while it has the focus.
fn task_row(ctx: &Ctx, task: &TaskChoice, focused: bool) -> Vec<Element> {
  vec![
    icon(Icon::SquareCheck, IconSize::Md, PaletteColor::TextSecondary).into(),
    key(&task.key.to_string(), TASK_KEY_WIDTH),
    title(&task.title, PaletteColor::TextPrimary),
    Text::new(&ctx.t(workflow_key(task.status)))
      .variant(TypographyStyle::Caption)
      .nowrap()
      .into(),
  ]
  .into_iter()
  .chain(focused.then(|| enter(ctx)))
  .collect()
}

/// A run's `play`, its task's key and its number, its task's title (*· this run* for the run the Conversation shows)
/// and its state, and *Enter* while it has the focus.
fn run_row(ctx: &mut Ctx, props: &MenuProps, run: &RunChoice, focused: bool) -> Vec<Element> {
  let number = run.number.get().to_string();
  let shown = ctx.t_args(
    "composer.mention.run",
    [("key", run.key.to_string()), ("number", number)],
  );
  let own = props
    .own
    .as_ref()
    .is_some_and(|(key, number)| key == &run.key && *number == Some(run.number));
  let words = if own {
    ctx.t_args("composer.mention.thisRun", [("title", &*run.title)])
  } else {
    run.title.clone()
  };
  let id = format!("{}-mention-run-{}-state", props.id, run.number.get());
  let state = StatusChip::new(id, Status::of_phase(run.phase)).render(ctx);
  vec![
    icon(Icon::Play, IconSize::Md, PaletteColor::TextSecondary).into(),
    key(&shown, RUN_KEY_WIDTH),
    title(&words, PaletteColor::TextMuted),
    state,
  ]
  .into_iter()
  .chain(focused.then(|| enter(ctx)))
  .collect()
}

/// A file's `file-code` and its path (`Path` `trApW`), *changed in this run* for one the run changed, and *Enter* while
/// it has the focus.
fn file_row(ctx: &Ctx, file: &FileChoice, focused: bool) -> Vec<Element> {
  let changed = file.changed.then(|| {
    Text::new(&ctx.t("composer.mention.changedInRun"))
      .variant(TypographyStyle::Caption)
      .nowrap()
      .flex_shrink(0.0)
      .into()
  });
  vec![
    icon(Icon::FileCode, IconSize::Md, PaletteColor::TextSecondary).into(),
    path(file),
  ]
  .into_iter()
  .chain(changed)
  .chain(focused.then(|| enter(ctx)))
  .collect()
}

/// The file's path in Geist Mono 12, its folder in `text_muted` and its name in `text_primary`, the letters that match
/// what was typed in 600 `text_primary` (the slash menu's matched letters of a name in Geist Mono). A path too long
/// for the row cuts its folder's first piece, so the name stays whole.
fn path(file: &FileChoice) -> Element {
  let path = file.path.as_str();
  let name_at = file.name_at();
  let mut cuts: Vec<usize> = vec![0, name_at, path.len()];
  cuts.extend(file.letters.iter().flat_map(|letters| [letters.start, letters.end]));
  cuts.sort_unstable();
  cuts.dedup();
  let matched = |range: &Range<usize>| {
    file
      .letters
      .iter()
      .any(|letters| letters.start <= range.start && range.end <= letters.end)
  };
  let pieces = cuts.windows(2).map(|cut| {
    let range = cut[0]..cut[1];
    let text = Text::new(&path[range.clone()].replace(' ', "\u{a0}")).nowrap();
    let text = if range.start == 0 && range.end <= name_at {
      text.min_width(Dimension::Px(0.0)).text_overflow(TextOverflow::Elipsis)
    } else {
      text.flex_shrink(0.0)
    };
    if matched(&range) {
      text.variant(SlashText::SkillMatch.role())
    } else if range.end <= name_at {
      text
        .variant(ExtraTypography::PropertyMono.role())
        .color(PaletteColor::TextMuted)
    } else {
      text.variant(ExtraTypography::PropertyMono.role())
    }
  });
  Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .align_items(Alignment::Center)
    .with_children(pieces)
    .into()
}

fn key(words: &str, width: f32) -> Element {
  Row::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .child(
      Text::new(words)
        .variant(ExtraTypography::PropertyMono.role())
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

fn title(words: &str, color: PaletteColor) -> Element {
  Text::new(words)
    .variant(TypographyStyle::Description)
    .color(color)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .into()
}

fn enter(ctx: &Ctx) -> Element {
  key_cap("mention-enter", &ctx.t("composer.mention.keys.enter"))
}

/// Nothing to choose (`No result` `zzJ78`): `search-x` in `text_muted`, the title and what to do.
pub(super) fn nothing((title, hint): (&str, &str)) -> Element {
  Row::new()
    .id("mention-nothing")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(ExtraSpacing::Inset.role())
        .right(SpacingSize::Md)
        .bottom(ExtraSpacing::Inset.role())
        .left(SpacingSize::Sm),
    )
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Start)
    .child(icon(Icon::SearchX, IconSize::Lg, PaletteColor::TextMuted))
    .child(
      Column::new()
        .flex(1.0)
        .min_width(Dimension::Px(0.0))
        .spacing(SpacingSize::Xs)
        .child(
          Text::new(title)
            .id("mention-nothing-title")
            .variant(TypographyStyle::Label),
        )
        .child(
          Text::new(hint)
            .id("mention-nothing-hint")
            .variant(TypographyStyle::Description)
            .color(PaletteColor::TextMuted)
            .width(Dimension::Pct(100.0)),
        ),
    )
    .into()
}

/// The keys (`Footer` `b5jY6`): *↑ ↓ Move*, *Enter Tab Insert*, *Esc Close* while there are rows, *Esc Close* alone
/// otherwise; and at the end *Type to filter*, or how many rows match what was typed.
fn footer(ctx: &Ctx, (offer, query): (&Offer, &str)) -> Element {
  let hint = |caps: &[&str], label: &str| {
    Row::new()
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .with_children(
        caps
          .iter()
          .map(|cap| key_cap("mention-key", &ctx.t(&format!("composer.mention.keys.{cap}")))),
      )
      .child(
        Text::new(&ctx.t(&format!("composer.mention.hint.{label}")))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      )
  };
  let note = match (query.is_empty(), offer.matches()) {
    (true, _) => ctx.t("composer.mention.typeToFilter"),
    (false, 1) => ctx.t_args("composer.mention.matches.one", [("query", query)]),
    (false, count) => ctx.t_args(
      "composer.mention.matches.other",
      [("count", count.to_string().as_str()), ("query", query)],
    ),
  };
  let hints: Vec<Row> = if offer.is_empty() {
    vec![hint(&["esc"], "close")]
  } else {
    vec![
      hint(&["move"], "move"),
      hint(&["enter", "tab"], "insert"),
      hint(&["esc"], "close"),
    ]
  };
  Row::new()
    .id("mention-footer")
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
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(&note)
        .id("mention-footer-note")
        .variant(TypographyStyle::Caption)
        .nowrap(),
    )
    .into()
}
