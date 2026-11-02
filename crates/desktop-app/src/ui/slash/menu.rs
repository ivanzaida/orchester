//! The slash menu's panel (`Slash menu` `IQOLN` in `07` `jAiyu`, `v7Xwo` in `06` `yKWQx`): a `Menu` panel 640 wide,
//! its left edge at the composer's box and its bottom 8 over it. Under the head *Commands* — with, at its end, how many
//! of the commands match once something is typed, or the composer's own note — the commands, 28 high: a 14 icon, the
//! name in a 150 column (the matched letters in 12/600, the rest in `text_secondary`, once something is typed) and what
//! it does in `text_muted`; the focus on `hover` with *Enter* at the row's end; a command that cannot run faded to
//! half, saying why in `text_secondary`. The keys under them, and at their end *Type to filter* or what was typed. With
//! nothing that matches (`07d` `G2ajn7`) it says so (`search-x`), and that Enter sends the text as typed.

use std::{ops::Range, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Button, Column, Placement, Popup, Row, Spacer, Text, TextOverflow},
  core::ElementRef,
  layout::Alignment,
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use parking_lot::Mutex;

use super::{Offered, Run, Shown, SlashCommand, SlashField, matched, query};
use crate::{
  theme::{ControlHeight, DISABLED_OPACITY, ExtraColor, ExtraSpacing, IconSize, SlashText},
  ui::{
    icon::{Icon, icon},
    kbd::key_cap,
    menu::menu_panel,
    pressable::{inert, pressable},
  },
};

/// The menu's width (`IQOLN`) and its names' column (`Label` `CXDhk`).
const WIDTH: f32 = 640.0;
const LABEL_WIDTH: f32 = 150.0;

/// The slash menu of the composer named `id`, over its box `anchor`, offering `commands`; `note` is what the head says
/// at its end while nothing is typed (the run's agent and step in a run's Conversation); `send` sends the draft as
/// typed when nothing matches, where the field's own Enter does not.
pub struct SlashMenuProps {
  pub id: &'static str,
  pub field: SlashField,
  pub anchor: ElementRef,
  pub commands: Vec<SlashCommand>,
  pub note: Option<Arc<str>>,
  pub send: Option<Run>,
}

/// What a row shows; what it runs stays with the field.
#[derive(Clone, PartialEq)]
struct Face {
  name: &'static str,
  icon: Icon,
  label: Arc<str>,
  detail: Arc<str>,
  runs: bool,
}

#[derive(Clone)]
struct Props {
  id: &'static str,
  field: SlashField,
  anchor: ElementRef,
  faces: Vec<Face>,
  note: Option<Arc<str>>,
}

/// Compared by what it shows; the anchor is the composer's for as long as it is shown.
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id && self.field == other.field && self.faces == other.faces && self.note == other.note
  }
}

impl DevtoolsInspectable for Props {}

pub fn menu(ctx: &mut Ctx, menu: SlashMenuProps) -> Element {
  let faces = menu
    .commands
    .iter()
    .map(|command| Face {
      name: command.name,
      icon: command.icon,
      label: command.label.clone(),
      detail: command.detail.clone(),
      runs: command.run.is_some(),
    })
    .collect();
  *menu.field.offered.lock() = Offered {
    commands: menu.commands,
    send: menu.send,
  };
  let props = Props {
    id: menu.id,
    field: menu.field,
    anchor: menu.anchor,
    faces,
    note: menu.note,
  };
  ctx.mount_keyed::<SlashMenu>(&format!("{}-slash", props.id), props)
}

struct SlashMenu {
  /// What was typed after the `/` the menu showed last: its focus goes back to the first row that can run when it
  /// changes.
  last: Mutex<Option<String>>,
}

impl Component for SlashMenu {
  type Props = Props;

  fn create(_ctx: &mut Ctx) -> Self {
    Self { last: Mutex::new(None) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let field = &props.field;
    let text = field.draft.get();
    let typed = query(&text);
    if typed.is_none() && field.closed.get() {
      field.closed.set(false);
    }
    let (Some(typed), true, false) = (typed, field.focused.get(), field.closed.get()) else {
      *field.shown.lock() = None;
      *self.last.lock() = None;
      return Element::new();
    };
    let rows: Vec<(&Face, Range<usize>)> = props
      .faces
      .iter()
      .filter_map(|face| matched(&face.label, typed).map(|letters| (face, letters)))
      .collect();
    let focused = self.focused(field, typed, &rows);
    *field.shown.lock() = Some(Shown {
      rows: rows.iter().map(|(face, _)| (face.name, face.runs)).collect(),
      focused,
    });
    let content = if rows.is_empty() {
      vec![nothing(ctx, typed), footer(ctx, typed, false)]
    } else {
      let count = (!typed.is_empty()).then(|| {
        ctx.t_args(
          "composer.slash.count",
          [
            ("shown", rows.len().to_string()),
            ("total", props.faces.len().to_string()),
          ],
        )
      });
      let mut content = vec![heading(ctx, count.or_else(|| props.note.clone()))];
      content.extend(rows.iter().enumerate().map(|(at, (face, letters))| {
        let choose = (props.field.clone(), face.name);
        row(
          ctx,
          (props.id, choose),
          (face, letters.clone(), !typed.is_empty()),
          focused == Some(at),
        )
      }));
      content.push(footer(ctx, typed, true));
      content
    };
    let panel = menu_panel(&format!("{}-slash-menu", props.id))
      .width(Dimension::Px(WIDTH))
      .with_children(content);
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    Popup::new(props.anchor.clone(), panel)
      .open_when(true)
      .dismiss_on_outside_click(false)
      .dismiss_on_escape(false)
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(0.0, gap)
      .into()
  }
}

impl SlashMenu {
  /// The row with the focus among `rows` for `typed`: the first that can run once that changed. The focus is read
  /// tracked on every path, so ↑ ↓ draw the menu again.
  fn focused(&self, field: &SlashField, typed: &str, rows: &[(&Face, Range<usize>)]) -> Option<usize> {
    let focus = field.focus.get();
    let mut last = self.last.lock();
    if last.as_deref() != Some(typed) {
      *last = Some(typed.to_owned());
      let first = rows.iter().position(|(face, _)| face.runs);
      if focus != first {
        field.focus.set(first);
      }
      return first;
    }
    focus.filter(|&at| rows.get(at).is_some_and(|(face, _)| face.runs))
  }
}

/// *Commands*, and at its end `end`: how many match, or the composer's note.
fn heading(ctx: &Ctx, end: Option<Arc<str>>) -> Element {
  Row::new()
    .id("slash-section-commands")
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
      Text::new(&ctx.t("composer.slash.commands"))
        .variant(SlashText::Section.role())
        .nowrap(),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(end.map(|end| {
      Text::new(&end)
        .id("slash-section-commands-note")
        .variant(SlashText::Note.role())
        .nowrap()
    }))
    .into()
}

/// A command's row: run by a click while it can run, faded with why while it cannot.
fn row(
  ctx: &Ctx,
  (id, (field, name)): (&str, (SlashField, &'static str)),
  (face, letters, filtered): (&Face, Range<usize>, bool),
  focused: bool,
) -> Element {
  let detail_color = if face.runs {
    PaletteColor::TextMuted
  } else {
    PaletteColor::TextSecondary
  };
  let children: Vec<Element> = vec![
    icon(face.icon, IconSize::Md, PaletteColor::TextSecondary).into(),
    label(&face.label, letters, filtered),
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
  let button = surface(pressable(move || field.run(name)))
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
fn label(words: &str, letters: Range<usize>, filtered: bool) -> Element {
  let column = Row::new()
    .width(Dimension::Px(LABEL_WIDTH))
    .flex_shrink(0.0)
    .align_items(Alignment::Center);
  if !filtered {
    return column
      .child(Text::new(words).variant(SlashText::Label.role()).nowrap())
      .into();
  }
  let piece = |range: Range<usize>, matched: bool| {
    let words = &words[range];
    (!words.is_empty()).then(|| {
      let text = Text::new(&words.replace(' ', "\u{a0}")).nowrap();
      if matched {
        text.variant(SlashText::Match.role())
      } else {
        text.variant(SlashText::Label.role()).color(PaletteColor::TextSecondary)
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
fn nothing(ctx: &Ctx, typed: &str) -> Element {
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

/// The keys (`Footer` `h3VUn7`): *↑ ↓ Move*, *Enter Choose*, *Esc Close* while there are rows, *Enter Send as text*
/// and *Esc Close* otherwise; and at the end *Type to filter*, or what was typed.
fn footer(ctx: &Ctx, typed: &str, rows: bool) -> Element {
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
