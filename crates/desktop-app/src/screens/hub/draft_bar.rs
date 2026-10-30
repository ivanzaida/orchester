//! An opened item's draft bar (`Draft bar` `W0sRO`): one bar on every editor of the hub — what it is now (*Draft v5 ·
//! saved 16:40 · Based on Published v4*, *Draft written by automation · Published v3*, *Published v1*) over what
//! changed or what to do next, then Discard draft and Publish. A question about the draft opens in its place. A shared
//! resource's bars take the same shape (`Missing bar` `Pf5rK` in 04, `Widens bar` `V097I` in 02d): what is missing on
//! this computer, or what a teammate's version adds with its list and its actions on a foot line under the words.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::icon::{Icon, icon},
};

/// Where the item stands, which the bar's glyph says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Standing {
  /// A draft saved, not published yet: a dashed circle in `warning` (`IFCep`).
  Draft,
  /// The draft is older than the latest version, or cannot be published: a warning.
  Attention,
  /// Nothing to publish: the latest version is current.
  Published,
  /// Items being chosen, e.g. what an import creates (`F3ukWF` in 02c).
  Choosing,
  /// What it names is missing on this computer: a plug with a spark in `danger` (`JnKum` in 04).
  Missing,
  /// A teammate's version widens what agents may do here: a shield in `warning` (`A4mB5` in 02d).
  Widens,
}

impl Standing {
  fn glyph(self) -> (Icon, PaletteColor) {
    match self {
      Self::Draft => (Icon::CircleDashed, PaletteColor::Warning),
      Self::Attention => (Icon::TriangleAlert, PaletteColor::Warning),
      Self::Published => (Icon::CircleCheck, PaletteColor::Success),
      Self::Choosing => (Icon::ListChecks, PaletteColor::TextSecondary),
      Self::Missing => (Icon::PlugZap, PaletteColor::Danger),
      Self::Widens => (Icon::ShieldAlert, PaletteColor::Warning),
    }
  }
}

pub(crate) struct DraftBar {
  id: Arc<str>,
  standing: Standing,
  status: Arc<str>,
  sub: Option<Arc<str>>,
  /// What the bar lists under its words, e.g. what a version adds.
  list: Option<Element>,
  actions: Vec<Element>,
  /// A line under the words with the actions at its end, in place of the actions beside them.
  foot: Option<Arc<str>>,
}

impl DraftBar {
  pub(crate) fn new(id: impl Into<Arc<str>>, standing: Standing, status: impl Into<Arc<str>>) -> Self {
    Self {
      id: id.into(),
      standing,
      status: status.into(),
      sub: None,
      list: None,
      actions: Vec::new(),
      foot: None,
    }
  }

  pub(crate) fn sub(mut self, sub: impl Into<Arc<str>>) -> Self {
    self.sub = Some(sub.into());
    self
  }

  pub(crate) fn list(mut self, list: impl Into<Element>) -> Self {
    self.list = Some(list.into());
    self
  }

  pub(crate) fn action(mut self, action: impl Into<Element>) -> Self {
    self.actions.push(action.into());
    self
  }

  /// The actions sit on a line of their own under the words, after `words` (`Actions` `CtBkw` in 02d).
  pub(crate) fn foot(mut self, words: impl Into<Arc<str>>) -> Self {
    self.foot = Some(words.into());
    self
  }
}

impl From<DraftBar> for Element {
  fn from(bar: DraftBar) -> Self {
    let (glyph, color) = bar.standing.glyph();
    let id = bar.id;
    let words = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Hair.role())
      .child(
        Text::new(&bar.status)
          .id(format!("{id}-status"))
          .variant(HubText::BarStatus.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(bar.sub.map(|sub| {
        Text::new(&sub)
          .id(format!("{id}-sub"))
          .variant(HubText::Hint.role())
          .width(Dimension::Pct(100.0))
      }))
      .with_children(bar.list);
    let (beside, foot) = match bar.foot {
      None => (bar.actions, None),
      Some(words) => (Vec::new(), Some(foot(&id, &words, bar.actions))),
    };
    let line = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(if foot.is_some() {
        Alignment::Start
      } else {
        Alignment::Center
      })
      .child(icon(glyph, IconSize::Md, color).id(format!("{id}-icon")))
      .child(words)
      .with_children(beside);
    Column::new()
      .id(id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(SpacingSize::Sm)
          .bottom(SpacingSize::Sm)
          .right(SpacingSize::Sm)
          .left(HubSpacing::Row.role()),
      )
      .spacing(ExtraSpacing::Loose.role())
      .background(PaletteColor::SurfaceRaised)
      .border_inside(BorderSize::Sm, PaletteColor::Border)
      .rounded(RadiusSize::Lg)
      .describe("standing", format!("{:?}", bar.standing).to_lowercase())
      .child(line)
      .with_children(foot)
      .into()
  }
}

/// `words` in 11.5 `text_muted`, indented past the glyph, and `actions` at the line's end.
fn foot(id: &str, words: &str, actions: Vec<Element>) -> Row {
  Row::new()
    .id(format!("{id}-foot"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SpacingSize::Xl))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(words)
        .id(format!("{id}-foot-words"))
        .variant(crate::theme::ExtraTypography::Note.role())
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(actions)
}
