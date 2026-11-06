//! A line of the run's diff (`Line 41` `Wd74X` in 09i): 22 high, its old and new numbers in 34 columns of a 76 gutter
//! (the old one at its start, the new one at its end, 8 before the sign) in Geist Mono 12 `text_muted`, its sign in 18
//! (`+` in `success`, `−` in `danger`), then its code in Geist Mono 12, each token in its code color; a removed line on
//! `diff_remove`, an added one on `diff_add`. Under the pointer, and only on the line under it, an 18 **+** on `accent`
//! stands in the old number's column where the line has none (`Add comment` `fAfW6` in `Line 45`), else in the sign's
//! place, so it never covers a number; a press opens the comment box under the line, and a drag over lines of the same
//! side under the last of them. The chosen lines take a 2 `info` edge at their start and their new number in `info`
//! (`r0sh1`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    events::mouse_event::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::{Alignment, layout_kind::Justify, text_style::TextAlign},
  node::{CursorIcon, Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{diff::LineKind, pull_request::feedback::DiffSide};
use orchester_workspace::highlight::TokenKind;

use crate::{
  screens::{
    review::task::Env,
    task::changes::diff::{LineKey, rows::LineRow},
  },
  theme::{ExtraColor, IconSize, token_color},
  ui::icon::{Icon, icon},
};

/// A line's height, its gutter and a number's column in it, its sign, and the **+** under the pointer (`Wd74X`,
/// `QtaVZ`).
const LINE_HEIGHT: f32 = 22.0;
const GUTTER_WIDTH: f32 = 76.0;
const NUMBER_WIDTH: f32 = 34.0;
const SIGN_WIDTH: f32 = 18.0;
const PLUS_SIZE: f32 = 18.0;

/// Where the **+** stands on the line under the pointer.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Plus {
  Hidden,
  /// In the old number's column: the line has no old number.
  Old,
  /// In the sign's place, beside the old number.
  Sign,
}

/// Line `shown` of file `file` of the diff, `width` wide; `hovered`: the pointer is over its row.
pub(super) fn line(
  ctx: &Ctx,
  (env, file): (&Env, usize),
  shown: &LineRow,
  (width, hovered): (Dimension, &Signal<bool>),
) -> Element {
  let Some(path) = env.diff.files().get(file).map(|file| Arc::<str>::from(file.path())) else {
    return Element::new();
  };
  let key = shown.key();
  let chosen = key.is_some_and(|key| {
    env
      .state
      .composing
      .get()
      .0
      .is_some_and(|composing| composing.head == env.head && composing.holds(&path, key))
  });
  let over = hovered.get() && key.is_some_and(|key| env.state.hovers(&path, key));
  let plus = match (over, shown.old) {
    (false, _) => Plus::Hidden,
    (true, None) => Plus::Old,
    (true, Some(_)) => Plus::Sign,
  };
  let look = Look::of(shown.kind);
  let sign: Element = if plus == Plus::Sign {
    Row::new()
      .id("review-line-add")
      .width(Dimension::Px(SIGN_WIDTH))
      .child(add())
      .into()
  } else {
    Text::new(&look.sign.map(|key| ctx.t(key)).unwrap_or_default())
      .variant(TypographyStyle::Mono)
      .color(look.sign_color)
      .text_align(TextAlign::Left)
      .nowrap()
      .width(Dimension::Px(SIGN_WIDTH))
      .into()
  };
  let tokens: Vec<Element> = shown
    .tokens
    .iter()
    .map(|token| {
      Text::new(&token.text)
        .class(token_class(token.kind))
        .variant(TypographyStyle::Mono)
        .color(token_color(token.kind))
        .nowrap()
        .into()
    })
    .collect();
  let side = key.map_or("none", |key| match key.side {
    DiffSide::Old => "old",
    DiffSide::New => "new",
  });
  let mut row = Row::new()
    .id(format!("review-line-{file}-{side}-{}", key.map_or(0, |key| key.number)))
    .describe("kind", look.name)
    .describe("chosen", chosen.to_string())
    .describe("hovered", over.to_string())
    .width(width)
    .height(Dimension::Px(LINE_HEIGHT))
    .align_items(Alignment::Center)
    .child(gutter(shown, (plus == Plus::Old, chosen)))
    .child(sign)
    .with_children(tokens);
  if let Some(background) = look.background {
    row = row.background(background);
  }
  if chosen {
    row = row.border_left(Border::inside(BorderSize::Md, PaletteColor::Info));
  }
  match key {
    Some(key) if !env.read_only => pressable(row, env, (path, key), hovered).into(),
    _ => row.into(),
  }
}

/// How a line of a kind looks: its tint, its sign and the sign's color, and its kind's name for tools.
struct Look {
  background: Option<PaletteColor>,
  sign: Option<&'static str>,
  sign_color: PaletteColor,
  name: &'static str,
}

impl Look {
  fn of(kind: LineKind) -> Self {
    let (background, sign, sign_color, name) = match kind {
      LineKind::Context => (None, None, PaletteColor::TextMuted, "context"),
      LineKind::Added => (
        Some(ExtraColor::DiffAdded.role()),
        Some("review_page.diff.added"),
        PaletteColor::Success,
        "added",
      ),
      LineKind::Removed => (
        Some(ExtraColor::DiffRemoved.role()),
        Some("review_page.diff.removed"),
        PaletteColor::Danger,
        "removed",
      ),
    };
    Self {
      background,
      sign,
      sign_color,
      name,
    }
  }
}

/// The line's numbers, the old one at the gutter's start and the new one at its end, 8 before the sign; `plus`: the
/// **+** in the old one's column, which the line leaves empty; a chosen line's new number in `info`.
fn gutter(shown: &LineRow, (plus, chosen): (bool, bool)) -> Row {
  let number = |value: Option<u32>, color: PaletteColor| {
    Text::new(&value.map(|value| value.to_string()).unwrap_or_default())
      .variant(TypographyStyle::Mono)
      .color(color)
      .nowrap()
  };
  let old: Element = if plus {
    Row::new().id("review-line-add").child(add()).into()
  } else {
    number(shown.old, PaletteColor::TextMuted).into()
  };
  let new_color = if chosen {
    PaletteColor::Info
  } else {
    PaletteColor::TextMuted
  };
  Row::new()
    .width(Dimension::Px(GUTTER_WIDTH))
    .height(Dimension::Px(LINE_HEIGHT))
    .padding(Padding::new().right(SpacingSize::Sm))
    .align_items(Alignment::Center)
    .child(
      Row::new()
        .width(Dimension::Px(NUMBER_WIDTH))
        .align_items(Alignment::Center)
        .child(old),
    )
    .child(
      Row::new()
        .width(Dimension::Px(NUMBER_WIDTH))
        .justify(Justify::End)
        .child(number(shown.new, new_color)),
    )
}

/// `row` under the pointer shows its **+**, and the line the pointer was over before stops showing its own; a press
/// opens the comment box under it, a drag takes the lines it passes.
fn pressable(row: Row, env: &Env, (path, key): (Arc<str>, LineKey), hovered: &Signal<bool>) -> Row {
  let (entered, left) = (env.state.clone(), env.state.clone());
  let (enter, leave) = (hovered.clone(), hovered.clone());
  let (pressed, dragged, released) = (env.state.clone(), env.state.clone(), env.state.clone());
  let (head, over_head) = (env.head.clone(), env.head.clone());
  let over_path = path.clone();
  row
    .cursor(CursorIcon::Pointer)
    .on_mouse_enter(move || {
      entered.hover((over_path.clone(), key), &enter);
      dragged.drag_over((&over_path, key), &over_head);
    })
    .on_mouse_leave(move || left.leave(&leave))
    .on_mouse_down(move |event: MouseEvent| {
      event.stop_propagation();
      pressed.press((path.clone(), key), &head);
    })
    .on_mouse_up(move |_: MouseEvent| released.release())
}

/// The **+** on the line under the pointer: 18 on `accent`, radius 4 (`Plus` `QtaVZ`).
fn add() -> Row {
  Row::new()
    .width(Dimension::Px(PLUS_SIZE))
    .height(Dimension::Px(PLUS_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::Accent)
    .rounded(RadiusSize::Sm)
    .child(icon(Icon::Plus, IconSize::Sm, PaletteColor::TextInverse))
}

/// Names the token's kind for tools (`lurq_read_tree`) and tests.
const fn token_class(kind: TokenKind) -> &'static str {
  match kind {
    TokenKind::Plain => "code-plain",
    TokenKind::Keyword => "code-keyword",
    TokenKind::String => "code-string",
    TokenKind::Comment => "code-comment",
    TokenKind::Constant => "code-constant",
    TokenKind::Function => "code-function",
    TokenKind::Variable => "code-variable",
  }
}
