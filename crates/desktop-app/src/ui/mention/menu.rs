//! The @ menu (`@ menu` `qhBhC` in 08 `gOs7u`, 08b `nUjD7`, 08c `GdUNZ`; `Am5KT`, `vnuo8`, `u7Q7h` in a run's
//! Conversation): a `Menu` panel 560 wide that opens 8 over the composer's box at its start, as the composer sits at
//! the bottom of the window. Its sections come in a fixed order — Tasks (`square-check`, the key in Geist Mono 12 in a
//! 64 column, the title, the workflow status in 11 `text_muted`) and Runs (`play`, *KON-12 · #9* in a 96 column, the
//! task's title in `text_muted`, the state as a status) — each under its title and count, 28 high rows with the focus
//! on `hover` and *Enter* at the focused row's end; a separator between them, and the keys under them. With nothing
//! that matches it says so (`search-x`) and keeps only *Esc Close*. The tasks and runs are read when it opens.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Spacer, Text, TextOverflow},
  core::ElementRef,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{project::TaskKey, run::RunNumber};
use orchester_workspace::{
  background,
  mentions::catalog::{Catalog, Offer, RunChoice, Section, TaskChoice},
  messages::Message,
};
use parking_lot::Mutex;

use super::{MentionField, Shown};
use crate::{
  theme::{ControlHeight, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    kbd::key_cap,
    menu::{menu_panel, separator},
    message::message_text,
    pressable::pressable,
    status::{Status, StatusChip, workflow_key},
  },
};

/// The menu's width (`qhBhC`).
const WIDTH: f32 = 560.0;
/// The column of a task's key (`Key` `A8Rq3`) and of a run's (`Key` `UZe0Z`).
const TASK_KEY_WIDTH: f32 = 64.0;
const RUN_KEY_WIDTH: f32 = 96.0;

/// The menu of the composer named `id`, over its box `anchor`, for the project at `root` called `project`; `own`, the
/// task and run a run's Conversation shows, leads. `revision` moves when the project's tasks change.
#[derive(Clone)]
pub struct MenuProps {
  pub id: &'static str,
  pub field: MentionField,
  pub anchor: ElementRef,
  pub root: PathBuf,
  pub project: Arc<str>,
  pub own: Option<(TaskKey, Option<RunNumber>)>,
  pub revision: u64,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for MenuProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.field == other.field
      && self.root == other.root
      && self.project == other.project
      && self.own == other.own
      && self.revision == other.revision
  }
}

impl DevtoolsInspectable for MenuProps {}

pub fn menu(ctx: &mut Ctx, props: MenuProps) -> Element {
  ctx.mount_keyed::<MentionMenu>(&format!("{}-mentions", props.id), props)
}

struct MentionMenu {
  /// The `@` and the words typed after it the menu showed last: its focus goes back to the first row when they change.
  last: Mutex<Option<(usize, String)>>,
}

impl Component for MentionMenu {
  type Props = MenuProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { last: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MenuProps>().clone();
    let field = &props.field;
    if field.refocus.get() {
      field.refocus.set(false);
      field.controller.focus(ctx);
    }
    let text = field.draft.get();
    let typed = field.typed(&text);
    let closed = field.closed.get();
    if closed.is_some() && typed.map(|typed| typed.at) != closed {
      field.closed.set(None);
    }
    let open = field.focused.get() && typed.is_some_and(|typed| Some(typed.at) != closed);
    let catalog = read(ctx, (props.root.clone(), props.revision, open));
    let (Some(typed), true) = (typed, open) else {
      *field.shown.lock() = None;
      return Element::new();
    };
    let content = match catalog {
      Read::Reading => {
        *field.shown.lock() = None;
        return Element::new();
      }
      Read::Failed(reason) => {
        *field.shown.lock() = None;
        let reason = message_text(ctx, &reason);
        let words = ctx.t_args("composer.mention.unreadable", [("reason", reason)]);
        vec![nothing((&ctx.t("composer.mention.unreadableTitle"), &words))]
      }
      Read::Ready(catalog) => {
        let own = props.own.as_ref().map(|(key, number)| (key, *number));
        let offer = catalog.offer(typed.query, own);
        let focused = self.focused(field, (typed.at, typed.query), offer.mentions().len());
        *field.shown.lock() = Some(Shown {
          replaces: typed.range(),
          rows: offer.mentions(),
        });
        rows(ctx, &props, (&offer, typed.query), focused)
      }
    };
    let panel = menu_panel(&format!("{}-mention-menu", props.id))
      .width(Dimension::Px(WIDTH))
      .with_children(content);
    Popup::new(props.anchor.clone(), panel)
      .open_when(true)
      .dismiss_on_outside_click(false)
      .dismiss_on_escape(false)
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(0.0, -ctx.theme().spacing_value(SpacingSize::Sm).to_px())
      .into()
  }
}

impl MentionMenu {
  /// The row with the focus among `rows` for what was typed after the `@` at `at`: the first once that changed.
  fn focused(&self, field: &MentionField, (at, query): (usize, &str), rows: usize) -> usize {
    let mut last = self.last.lock();
    if last.as_ref().is_none_or(|(was_at, was)| *was_at != at || was != query) {
      *last = Some((at, query.to_owned()));
      if field.focus.get_untracked() != 0 {
        field.focus.set(0);
      }
      return 0;
    }
    field.focus.get().min(rows.saturating_sub(1))
  }
}

/// The project's tasks and runs as the menu reads them.
enum Read {
  Reading,
  Ready(Catalog),
  Failed(Message),
}

/// The tasks and runs of the project at `root`, read off the UI thread while the menu is `open`, again when it opens
/// and when the project's tasks change (`revision`).
fn read(ctx: &mut Ctx, (root, revision, open): (PathBuf, u64, bool)) -> Read {
  let state = ctx
    .future((root, revision, open), |(root, _, open)| async move {
      if !open {
        return Ok(None);
      }
      background::run(move || Catalog::read_in(&root)).await?.map(Some)
    })
    .state()
    .get();
  let pending = state.is_pending();
  // A failed re-read keeps the last data; it must not stay answerable (rows to insert) as if current.
  match (state.error, state.data.flatten()) {
    (Some(error), _) if !pending => Read::Failed(error),
    (None, Some(catalog)) => Read::Ready(catalog),
    _ => Read::Reading,
  }
}

/// The sections that match, with the keys under them; or that nothing matches.
fn rows(ctx: &mut Ctx, props: &MenuProps, (offer, query): (&Offer, &str), focused: usize) -> Vec<Element> {
  if offer.is_empty() {
    let title = ctx.t_args("composer.mention.empty.title", [("query", query)]);
    let hint = ctx.t_args("composer.mention.empty.hint", [("project", &*props.project)]);
    return vec![nothing((&title, &hint)), footer(ctx, (offer, query))];
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
  if !offer.runs.shown.is_empty() {
    if tasks > 0 {
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
  rows.push(footer(ctx, (offer, query)));
  rows
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

fn key(words: &str, width: f32) -> Element {
  Row::new()
    .width(Dimension::Px(width))
    .flex_shrink(0.0)
    .child(
      Text::new(words)
        .variant(crate::theme::ExtraTypography::PropertyMono.role())
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
fn nothing((title, hint): (&str, &str)) -> Element {
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
