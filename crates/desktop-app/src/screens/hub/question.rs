//! An in-place question of the hub (`06c · In-place questions` `P7wAQe`): it opens in place of the bar, row or field
//! that asked it, never in a modal — its title, one sentence, what it would touch, then its action and the way back.
//! A question that loses something (Discard draft, Remove) is in the danger tone; the others are neutral.

use std::sync::Arc;

use lurq::{
  app::theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};

use crate::{
  theme::{ExtraSpacing, ExtraTypography, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
  },
};

/// What a question's answer does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Weight {
  /// It loses something: the danger tone, its action a danger button.
  Loses,
  /// It changes nothing lost: the neutral tone, its action the primary button.
  Neutral,
}

pub(crate) struct Question {
  id: Arc<str>,
  weight: Weight,
  title: Arc<str>,
  sentence: Arc<str>,
  /// What the answer touches, one line each with its glyph.
  lines: Vec<(Icon, Arc<str>)>,
  confirm: ActionButton,
  keep: ActionButton,
  /// Why the last answer did not happen, under the sentence.
  failure: Option<Arc<str>>,
}

impl Question {
  /// The question `id` with its action `confirm` (`<id>-confirm`) and `keep` (`<id>-keep`) labels.
  pub(crate) fn new(
    id: impl Into<Arc<str>>,
    weight: Weight,
    (title, sentence): (Arc<str>, Arc<str>),
    (confirm, keep): ((Icon, Arc<str>), Arc<str>),
  ) -> Self {
    let id = id.into();
    let kind = match weight {
      Weight::Loses => ButtonKind::Danger,
      Weight::Neutral => ButtonKind::Primary,
    };
    Self {
      confirm: ActionButton::new(format!("{id}-confirm"), kind, confirm.1).icon(confirm.0),
      keep: ActionButton::new(format!("{id}-keep"), ButtonKind::Ghost, keep),
      id,
      weight,
      title,
      sentence,
      lines: Vec::new(),
      failure: None,
    }
  }

  pub(crate) fn line(mut self, glyph: Icon, text: impl Into<Arc<str>>) -> Self {
    self.lines.push((glyph, text.into()));
    self
  }

  /// The action's and the way back's own element ids, where a screen named them before the hub.
  pub(crate) fn button_ids(mut self, confirm: &str, keep: &str) -> Self {
    self.confirm = self.confirm.id(confirm);
    self.keep = self.keep.id(keep);
    self
  }

  pub(crate) fn failure(mut self, failure: Option<Arc<str>>) -> Self {
    self.failure = failure;
    self
  }

  /// What the action does; `busy` while it is under way.
  pub(crate) fn on_confirm(mut self, busy: bool, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.confirm = if busy {
      self.confirm.loading()
    } else {
      self.confirm.on_click(handler)
    };
    self
  }

  pub(crate) fn on_keep(mut self, handler: impl Fn() + Send + Sync + 'static) -> Self {
    self.keep = self.keep.on_click(handler);
    self
  }
}

impl From<Question> for Element {
  fn from(question: Question) -> Self {
    let (fill, stroke) = match question.weight {
      Weight::Loses => (PaletteColor::DangerMuted, PaletteColor::Danger),
      Weight::Neutral => (PaletteColor::SurfaceRaised, PaletteColor::Border),
    };
    let lines = question.lines.into_iter().enumerate().map(|(index, (glyph, text))| {
      Row::new()
        .id(format!("{}-line-{index}", question.id))
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted))
        .child(
          Text::new(&text)
            .variant(ExtraTypography::Prose.role())
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
    });
    Column::new()
      .id(question.id.to_string())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, HubSpacing::Row.role()))
      .spacing(ExtraSpacing::Loose.role())
      .background(fill)
      .border_inside(BorderSize::Sm, stroke)
      .rounded(RadiusSize::Lg)
      .describe("weight", match question.weight {
        Weight::Loses => "loses",
        Weight::Neutral => "neutral",
      })
      .child(
        Text::new(&question.title)
          .id(format!("{}-title", question.id))
          .variant(ExtraTypography::FormSection.role()),
      )
      .child(
        Text::new(&question.sentence)
          .id(format!("{}-sentence", question.id))
          .variant(HubText::Status.role())
          .width(Dimension::Pct(100.0)),
      )
      .with_children(lines)
      .with_children(question.failure.map(|failure| {
        Text::new(&failure)
          .id(format!("{}-failure", question.id))
          .variant(ExtraTypography::Note.role())
          .color(PaletteColor::Danger)
          .width(Dimension::Pct(100.0))
      }))
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(question.confirm)
          .child(question.keep),
      )
      .into()
  }
}
