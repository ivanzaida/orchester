//! The event line (`1 · Event lines` `VGbU7` in `Orchestrator cards · redesign` `zuvUi`): 32 high, 10 in at its sides,
//! its parts 10 apart, radius 6 with the `hover` fill under the pointer: a 13 icon in its tone; what happened (12.5),
//! in `danger` for what was refused and `text_muted` for what was dismissed or canceled; its target in Geist Mono 11.5
//! `text_muted`; an optional chip (a pill on its tone's muted fill, 1 by 7 inside, its words 11/500, a loader before
//! them while something is under way); then at its end the time in Geist Mono 11 `text_muted` and the one link that
//! acts on it (12/500 `text_secondary`). A line with a detail (a refusal's reason) says it on hover, and a press opens
//! it under the line as a quote. Several lines of one answer are one `border` list, radius 6 ([`list`]); what nests
//! under a line is a quote 46 in from its start: a 2 `border_strong` line at its left, 12 before its words, 3 between
//! them ([`quote`]).

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{CursorIcon, Element, Style, border::Border, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    icon::{Icon, icon},
    pressable::{Press, pressable, pressed},
    tooltip::Tooltip,
  },
};

/// A line's height (`R96KH`).
const HEIGHT: f32 = 32.0;

/// The tone of a line: its icon's color and its words'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tone {
  Success,
  Warning,
  Danger,
  /// Dismissed, canceled: the icon and the words in `text_muted`.
  Muted,
  /// Under way or neutral: the icon in `text_secondary`.
  Plain,
}

impl Tone {
  const fn colors(self) -> (PaletteColor, PaletteColor) {
    match self {
      Self::Success => (PaletteColor::Success, PaletteColor::TextPrimary),
      Self::Warning => (PaletteColor::Warning, PaletteColor::TextPrimary),
      Self::Danger => (PaletteColor::Danger, PaletteColor::Danger),
      Self::Muted => (PaletteColor::TextMuted, PaletteColor::TextMuted),
      Self::Plain => (PaletteColor::TextSecondary, PaletteColor::TextPrimary),
    }
  }
}

/// The tone of a chip: its fill and its words' color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChipTone {
  Warning,
  Success,
  /// `accent_muted` with `text_secondary` words.
  Neutral,
  /// As neutral, with the turning loader before its words.
  Working,
}

/// A chip after a line's target, with what its tooltip says.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Chip {
  pub(super) words: Arc<str>,
  pub(super) tone: ChipTone,
  pub(super) tip: Option<Arc<str>>,
}

impl Chip {
  pub(super) fn new(words: Arc<str>, tone: ChipTone) -> Self {
    Self { words, tone, tip: None }
  }

  #[must_use]
  pub(super) fn tip(mut self, tip: Arc<str>) -> Self {
    self.tip = Some(tip);
    self
  }
}

/// The link at a line's end: its words and what a press does.
#[derive(Clone)]
pub(super) struct LineLink {
  pub(super) label: Arc<str>,
  pub(super) press: Press,
}

/// An event line, named `id`; `outcome`, and `status` when it has one, are what tools read of it.
#[derive(Clone)]
pub(super) struct Line {
  pub(super) id: String,
  pub(super) outcome: &'static str,
  pub(super) status: Option<&'static str>,
  pub(super) glyph: Icon,
  pub(super) tone: Tone,
  pub(super) what: Arc<str>,
  pub(super) target: Option<Arc<str>>,
  pub(super) chip: Option<Chip>,
  pub(super) time: Option<String>,
  pub(super) link: Option<LineLink>,
  /// Why it was refused: on hover, and under the line once pressed.
  pub(super) detail: Option<Arc<str>>,
  /// The other places it opens, beyond its one link: under its detail once pressed.
  pub(super) more: Vec<LineLink>,
}

impl Line {
  pub(super) fn new(id: impl Into<String>, (glyph, tone): (Icon, Tone), what: Arc<str>) -> Self {
    Self {
      id: id.into(),
      outcome: "event",
      status: None,
      glyph,
      tone,
      what,
      target: None,
      chip: None,
      time: None,
      link: None,
      detail: None,
      more: Vec::new(),
    }
  }

  #[must_use]
  pub(super) const fn outcome(mut self, outcome: &'static str) -> Self {
    self.outcome = outcome;
    self
  }

  #[must_use]
  pub(super) const fn status(mut self, status: &'static str) -> Self {
    self.status = Some(status);
    self
  }

  #[must_use]
  pub(super) fn target(mut self, target: Option<Arc<str>>) -> Self {
    self.target = target;
    self
  }

  #[must_use]
  pub(super) fn chip(mut self, chip: Option<Chip>) -> Self {
    self.chip = chip;
    self
  }

  #[must_use]
  pub(super) fn time(mut self, time: Option<String>) -> Self {
    self.time = time;
    self
  }

  #[must_use]
  pub(super) fn link(mut self, link: Option<LineLink>) -> Self {
    self.link = link;
    self
  }

  #[must_use]
  pub(super) fn detail(mut self, detail: Option<Arc<str>>) -> Self {
    self.detail = detail;
    self
  }

  #[must_use]
  pub(super) fn more(mut self, more: Vec<LineLink>) -> Self {
    self.more = more;
    self
  }

  /// The line, mounted so a pressed detail stays open while the chat changes.
  pub(super) fn render(self, ctx: &mut Ctx) -> Element {
    let key = format!("line-{}", self.id);
    ctx.mount_keyed::<LineView>(&key, LineProps(self))
  }
}

#[derive(Clone)]
struct LineProps(Line);

/// Compared by what it shows; the link's press is the screen's.
impl PartialEq for LineProps {
  fn eq(&self, other: &Self) -> bool {
    let (one, two) = (&self.0, &other.0);
    (one.status, &one.id, one.outcome, one.glyph) == (two.status, &two.id, two.outcome, two.glyph)
      && (
        one.tone,
        &one.what,
        &one.target,
        &one.chip,
        &one.time,
        one.link.as_ref().map(|link| &link.label),
        &one.detail,
      ) == (
        two.tone,
        &two.what,
        &two.target,
        &two.chip,
        &two.time,
        two.link.as_ref().map(|link| &link.label),
        &two.detail,
      )
      && one
        .more
        .iter()
        .map(|link| &link.label)
        .eq(two.more.iter().map(|link| &link.label))
  }
}

impl DevtoolsInspectable for LineProps {}

struct LineView {
  /// The detail is open under the line.
  open: Signal<bool>,
}

impl Component for LineView {
  type Props = LineProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let LineProps(line) = ctx.props::<LineProps>().clone();
    let Some(detail) = line.detail.clone() else {
      return Element::from(row(ctx, &line));
    };
    let open = self.open.get();
    let toggle = self.open.clone();
    let id = line.id.clone();
    let row = pressable(move || toggle.update(|open| *open = !*open))
      .id(format!("{id}-line"))
      .describe("open", open.to_string())
      .width(Dimension::Pct(100.0))
      .child(row(ctx, &line));
    let row = Tooltip::new(format!("{id}-why"), detail.clone()).wrap(ctx, row);
    let more = (open && !line.more.is_empty()).then(|| more(&id, line.more.clone()));
    Element::from(
      Column::new()
        .width(Dimension::Pct(100.0))
        .child(row)
        .with_children(open.then(|| quote(&format!("{id}-reason"), &detail)))
        .with_children(more),
    )
  }
}

/// The other places line `id` opens, in a row under its detail, 46 in.
fn more(id: &str, links: Vec<LineLink>) -> Element {
  let links = links
    .into_iter()
    .enumerate()
    .map(|(index, link)| link_element(&format!("{id}-more-{index}"), link));
  nested(
    &format!("{id}-more"),
    Column::new().width(Dimension::Pct(100.0)).child(
      Row::new()
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .with_children(links),
    ),
  )
}

/// The line's row.
fn row(ctx: &mut Ctx, line: &Line) -> Row {
  let id = line.id.as_str();
  let (glyph_color, words_color) = line.tone.colors();
  let what = Text::new(&line.what)
    .id(format!("{id}-title"))
    .variant(OrchestratorText::Line.role())
    .color(words_color)
    .nowrap()
    .text_overflow(TextOverflow::Elipsis)
    .flex_shrink(1.0)
    .min_width(Dimension::Px(0.0));
  let target = line.target.as_ref().map(|target| {
    Text::new(target)
      .id(format!("{id}-target"))
      .variant(ExtraTypography::MonoKey.role())
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .flex_shrink(1.0)
      .min_width(Dimension::Px(0.0))
  });
  let chip = line
    .chip
    .clone()
    .map(|chip| chip_element(ctx, &format!("{id}-chip"), &chip));
  let time = line.time.as_ref().map(|time| {
    Text::new(time)
      .id(format!("{id}-time"))
      .variant(ExtraTypography::MonoNote.role())
      .flex_shrink(0.0)
      .nowrap()
  });
  let link = line.link.clone().map(|link| link_element(&format!("{id}-open"), link));
  Row::new()
    .id(id.to_owned())
    .describe("outcome", line.outcome)
    .describe("status", line.status.unwrap_or_default())
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Loose.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(icon(line.glyph, IconSize::Base, glyph_color).flex_shrink(0.0))
    .child(what)
    .with_children(target)
    .with_children(chip)
    .child(Spacer::new().flex(1.0))
    .with_children(time)
    .with_children(link)
}

/// A chip named `id`: its words on its tone's fill, with its tooltip when it has one.
pub(super) fn chip_element(ctx: &mut Ctx, id: &str, chip: &Chip) -> Element {
  let (fill, color) = match chip.tone {
    ChipTone::Warning => (PaletteColor::WarningMuted, PaletteColor::Warning),
    ChipTone::Success => (PaletteColor::SuccessMuted, PaletteColor::Success),
    ChipTone::Neutral | ChipTone::Working => (PaletteColor::AccentMuted, PaletteColor::TextSecondary),
  };
  let pill = Row::new()
    .id(id.to_owned())
    .flex_shrink(0.0)
    .padding(Padding::symmetric(ExtraSpacing::Snug.role(), ExtraSpacing::Hair.role()))
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .background(fill)
    .rounded(ExtraRadius::Full.role())
    .with_children(
      (chip.tone == ChipTone::Working).then(|| icon(Icon::LoaderCircle, IconSize::Xs, PaletteColor::TextSecondary)),
    )
    .child(
      Text::new(&chip.words)
        .id(format!("{id}-label"))
        .variant(TypographyStyle::FieldLabel)
        .color(color)
        .nowrap(),
    );
  match &chip.tip {
    Some(tip) => Tooltip::new(id.to_owned(), tip.clone()).wrap(ctx, pill),
    None => pill.into(),
  }
}

/// The link at a line's end, named `id`.
pub(super) fn link_element(id: &str, link: LineLink) -> Element {
  pressed(link.press)
    .id(id.to_owned())
    .describe("a11y_name", link.label.clone())
    .flex_shrink(0.0)
    .rounded(ExtraRadius::Link.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .cursor(CursorIcon::Pointer)
    .child(
      Text::new(&link.label)
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap(),
    )
    .into()
}

/// Lines of one answer as one `border` list, radius 6, a `border` line between them; `id` names it.
pub(super) fn list(id: &str, lines: Vec<Element>) -> Element {
  let last = lines.len().saturating_sub(1);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .clip()
    .with_children(lines.into_iter().enumerate().map(|(index, line)| {
      let row = Column::new().width(Dimension::Pct(100.0)).child(line);
      if index < last {
        row.border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      } else {
        row
      }
    }))
    .into()
}

/// `words` nested under a line, 46 in, as a quote in 12 `text_muted`; `id` names it.
pub(super) fn quote(id: &str, words: &str) -> Element {
  nested(
    id,
    Column::new().width(Dimension::Pct(100.0)).child(
      Text::new(words)
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0)),
    ),
  )
}

/// `content` as a quote 46 in from a line's start; `id` names it.
pub(super) fn nested(id: &str, content: Column) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(OrchestratorSpacing::Nested.role())
        .right(ExtraSpacing::Loose.role())
        .bottom(SpacingSize::Xs),
    )
    .child(
      content
        .padding(
          Padding::new()
            .left(SpacingSize::Md)
            .top(ExtraSpacing::Tight.role())
            .bottom(ExtraSpacing::Tight.role()),
        )
        .border_left(Border::inside(BorderSize::Md, PaletteColor::BorderStrong)),
    )
    .into()
}
