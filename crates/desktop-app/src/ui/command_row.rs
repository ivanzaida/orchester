//! A command or an address with Copy at its end (`Command row` `PEnj5`): the value in Geist Mono 11.5 at 1.4 on
//! `code_bg` inside a `border` stroke, radius 6, 6 over and under it, 10 before it and 4 after Copy, and a 24-high
//! ghost Copy beside it that says Copied, or that it could not copy, for a moment. A command breaks into lines only
//! at its spaces, never inside a word such as `--header` (a path too long for a line also after its separators),
//! and a line takes every word that fits without the space after it; its own line breaks are kept. What it shows may
//! differ from what Copy puts on the clipboard: a command shows its token masked, and Copy copies the full text the
//! caller supplies. A faded row (a server that does not listen) shows its value in `text_muted`, and its Copy is
//! unavailable. A one-line row (a path) ends in an ellipsis when it is too long.

use std::{sync::Arc, time::Duration};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};

use super::{
  button::{ActionButton, ButtonKind},
  icon::{Icon, icon},
};
use crate::theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, mono_space};

/// How long Copy says what happened before it offers to copy again.
const COPY_FEEDBACK: Duration = Duration::from_secs(2);

/// Puts the row's full text somewhere; `true` once it is there.
pub type Copy = Arc<dyn Fn() -> bool + Send + Sync>;

#[derive(Clone)]
pub struct CommandRow {
  id: Arc<str>,
  shown: Arc<str>,
  copy_label: Arc<str>,
  copy: Copy,
  faded: bool,
  one_line: bool,
  prompt: Prompt,
}

/// The mark before a command to run, as a terminal shows one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Prompt {
  None,
  /// A chevron (`Command row` `PEnj5`).
  Chevron,
  /// A shell's `$` (`Prompt` `BtAzY` in Add project's commands, `Command` `l4sxi`).
  Dollar,
}

/// The shell's prompt mark: a symbol, not words, so it is not translated.
const DOLLAR: &str = "$";

/// Rows are compared by what they show; Copy goes to the same place.
impl PartialEq for CommandRow {
  fn eq(&self, other: &Self) -> bool {
    (
      &self.id,
      &self.shown,
      &self.copy_label,
      self.faded,
      self.one_line,
      self.prompt,
    ) == (
      &other.id,
      &other.shown,
      &other.copy_label,
      other.faded,
      other.one_line,
      other.prompt,
    )
  }
}

impl DevtoolsInspectable for CommandRow {}

impl CommandRow {
  /// `shown` in the box (its element id `id`, the box `<id>-box`, Copy `<id>-copy`); Copy, named `copy_label`,
  /// runs `copy`.
  pub fn new(
    id: impl Into<Arc<str>>,
    shown: impl Into<Arc<str>>,
    copy_label: impl Into<Arc<str>>,
    copy: impl Fn() -> bool + Send + Sync + 'static,
  ) -> Self {
    Self {
      id: id.into(),
      shown: shown.into(),
      copy_label: copy_label.into(),
      copy: Arc::new(copy),
      faded: false,
      one_line: false,
      prompt: Prompt::None,
    }
  }

  /// The value in `text_muted` and Copy unavailable: what it names is not in use now.
  pub fn faded(mut self) -> Self {
    self.faded = true;
    self
  }

  /// A prompt mark before the command, as a terminal shows one: a command to run.
  pub fn prompt(mut self) -> Self {
    self.prompt = Prompt::Chevron;
    self
  }

  /// A shell's `$` before the command: a command to run in a terminal (`BtAzY` in `l4sxi`).
  pub fn dollar_prompt(mut self) -> Self {
    self.prompt = Prompt::Dollar;
    self
  }

  /// The value on one line, its end cut with an ellipsis when it is too long: a path.
  pub fn one_line(mut self) -> Self {
    self.one_line = true;
    self
  }

  pub fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("command-row-{}", self.id);
    ctx.mount_keyed::<CommandRowView>(&key, self)
  }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Copied {
  #[default]
  Not,
  Done,
  Failed,
}

impl DevtoolsInspectable for Copied {}

struct CommandRowView {
  copied: Signal<Copied>,
  reset: Timeout,
}

impl Component for CommandRowView {
  type Props = CommandRow;

  fn create(ctx: &mut Ctx) -> Self {
    let copied = ctx.signal(Copied::Not);
    let back = copied.clone();
    let reset = ctx.create_timeout(COPY_FEEDBACK, move || back.set(Copied::Not));
    Self { copied, reset }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<CommandRow>().clone();
    let (glyph, label) = match self.copied.get() {
      Copied::Not => (Icon::Copy, props.copy_label.to_string()),
      Copied::Done => (Icon::Check, ctx.t("ui.copied").to_string()),
      Copied::Failed => (Icon::CircleAlert, ctx.t("ui.copyFailed").to_string()),
    };
    let button = ActionButton::new(format!("{}-copy", props.id), ButtonKind::Ghost, label)
      .icon(glyph)
      .small()
      .in_form();
    let button = if props.faded {
      button.disabled()
    } else {
      let (copied, reset, copy) = (self.copied.clone(), self.reset.clone(), props.copy.clone());
      button.on_click(move || {
        copied.set(if copy() { Copied::Done } else { Copied::Failed });
        reset.restart();
      })
    };
    let color = if props.faded {
      PaletteColor::TextMuted
    } else {
      PaletteColor::TextPrimary
    };
    // Each word keeps the space after it, which lurq counts when it fills a line: the words may reach one space
    // into the gap before Copy, where that space shows as nothing, so a line breaks as if it took no room.
    let theme = ctx.theme();
    let gap = theme.spacing_value(SpacingSize::Sm).to_px();
    let space = mono_space(theme, ExtraTypography::MonoKey.role());
    let gap = if props.one_line { gap } else { (gap - space).max(0.0) };
    let value = if props.one_line {
      Text::new(&props.shown)
        .variant(ExtraTypography::MonoKey.role())
        .color(color)
        .width(Dimension::Px(0.0))
        .flex(1.0)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .into()
    } else {
      words(&props.id, &props.shown, &color)
    };
    Row::new()
      .id(format!("{}-box", props.id))
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Snug.role())
          .bottom(ExtraSpacing::Snug.role())
          .left(ExtraSpacing::Loose.role())
          .right(SpacingSize::Xs),
      )
      .spacing(Dimension::Px(gap))
      .align_items(Alignment::Center)
      .background(ExtraColor::CodeBackground.role())
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .with_children(match props.prompt {
        Prompt::None => None,
        Prompt::Chevron => Some(Element::from(
          icon(Icon::ChevronRight, IconSize::Sm, PaletteColor::TextMuted).flex_shrink(0.0),
        )),
        // The row's gap leaves out the space the words keep after them; the mark keeps it, so it stands a gap
        // before the command.
        Prompt::Dollar => Some(
          Row::new()
            .flex_shrink(0.0)
            .padding(Padding::new().right(Dimension::Px(space)))
            .child(Text::new(DOLLAR).variant(ExtraTypography::MonoKey.role()))
            .into(),
        ),
      })
      .child(
        Row::new()
          .id(props.id.to_string())
          .flex(1.0)
          .min_width(Dimension::Px(0.0))
          .child(value),
      )
      .child(button)
  }
}

/// `text` line by line, each line laid out word by word so it breaks only at its spaces; a word keeps the space
/// after it, so the words read as the text itself. A word longer than [`LONG_WORD`] characters (a path) may also
/// break after its `\` and `/`, so it never runs past the box.
fn words(id: &str, text: &str, color: &PaletteColor) -> Element {
  let lines = text.split('\n').enumerate().map(|(number, line)| {
    let count = line.split(' ').count();
    Row::new()
      .width(Dimension::Pct(100.0))
      .wrap()
      .with_children(line.split(' ').enumerate().flat_map(|(index, word)| {
        let word = if index + 1 < count {
          format!("{word} ")
        } else {
          word.to_owned()
        };
        pieces(&word)
          .into_iter()
          .enumerate()
          .map(|(piece, text)| {
            let piece_id = if piece == 0 {
              format!("{id}-word-{number}-{index}")
            } else {
              format!("{id}-word-{number}-{index}-{piece}")
            };
            Text::new(&text)
              .id(piece_id)
              .variant(ExtraTypography::MonoKey.role())
              .color(color.clone())
              .nowrap()
          })
          .collect::<Vec<_>>()
      }))
  });
  Column::new().width(Dimension::Pct(100.0)).with_children(lines).into()
}

/// A word longer than this many characters may break after its path separators.
const LONG_WORD: usize = 32;

/// `word` as the pieces a line may break between: itself, or a long word cut after each `\` and `/`.
fn pieces(word: &str) -> Vec<String> {
  if word.chars().count() <= LONG_WORD {
    return vec![word.to_owned()];
  }
  let mut pieces = Vec::new();
  let mut piece = String::new();
  for character in word.chars() {
    piece.push(character);
    if matches!(character, '\\' | '/') {
      pieces.push(std::mem::take(&mut piece));
    }
  }
  if !piece.is_empty() {
    pieces.push(piece);
  }
  pieces
}
