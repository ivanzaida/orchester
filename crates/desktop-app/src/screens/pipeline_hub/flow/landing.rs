//! Where a dragged card would land and what that means (`Drop here` `y0SHa` in P3 `K4B56`): between two places a line
//! with an `accent` node and *05 Review goes here · runs after 01 Research, before 02 Plan*; on a card, an `accent`
//! outline on the card and *05 Review runs in parallel with 03 Implement* under its stage. A place the card cannot
//! land says why in `warning` and drops nothing; a place that changes nothing shows nothing.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::resource::{MoveError, PipelineEdit, StepKey};
use orchester_workspace::pipelines::flow::PipelineFlow;

use super::{
  rail::{self, Node},
  reorder::{Cards, Move, tried},
  state::{Drag, Drop},
};
use crate::theme::{ExtraSpacing, FlowText};

/// The line's bar (`Bar` `QBV3r`).
const BAR: f32 = 2.0;

/// What a drag over the flow shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Landing {
  pub at: Drop,
  /// What it says, as the title and what follows it; a refusal is one sentence, in `warning`.
  pub words: Words,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Words {
  Goes { title: String, hint: String },
  Parallel(String),
  Refused(String),
}

/// What dropping `drag` where it is over does, or nothing to show when it changes nothing or is over nothing.
pub(super) fn of(
  ctx: &Ctx,
  (flow, current, cards): (&PipelineFlow, &PipelineEdit, &Cards),
  drag: &Drag,
) -> Option<Landing> {
  let at = drag.over.clone()?;
  let change = match &at {
    Drop::Before(before) => Move::Before(before.clone()),
    Drop::OnCard(target) => Move::Beside(target.clone()),
  };
  let words = match tried(current, &drag.key, &change) {
    Ok(false) => return None,
    Err(error) => Words::Refused(refusal(ctx, cards, drag, &error)),
    Ok(true) => match &at {
      Drop::Before(before) => Words::Goes {
        title: ctx
          .t_args(
            "pipeline_hub.flow.move.goesHere",
            [
              ("number", super::words::number(drag.card.number)),
              ("name", drag.card.name.to_string()),
            ],
          )
          .to_string(),
        hint: between(ctx, (flow, cards), (&drag.key, before.as_ref())),
      },
      Drop::OnCard(target) => Words::Parallel(
        ctx
          .t_args(
            "pipeline_hub.flow.move.parallel",
            [
              ("number", super::words::number(drag.card.number)),
              ("name", drag.card.name.to_string()),
              ("with", with(ctx, flow, cards, target).to_string()),
            ],
          )
          .to_string(),
      ),
    },
  };
  Some(Landing { at, words })
}

fn refusal(ctx: &Ctx, cards: &Cards, drag: &Drag, error: &MoveError) -> String {
  ctx
    .t_args(
      "pipeline_hub.flow.move.refused",
      [
        ("name", drag.card.name.to_string()),
        ("reason", cards.reason(ctx, error, Some(&drag.key)).to_string()),
      ],
    )
    .to_string()
}

/// The steps that run together with `target`, as *04 and 05*.
fn with(ctx: &Ctx, flow: &PipelineFlow, cards: &Cards, target: &StepKey) -> std::sync::Arc<str> {
  let together: Vec<StepKey> = flow
    .stages
    .iter()
    .find(|stage| stage.cards.iter().any(|card| &card.key == target))
    .map(|stage| stage.cards.iter().map(|card| card.key.clone()).collect())
    .unwrap_or_else(|| vec![target.clone()]);
  cards.group(ctx, &together)
}

/// What a card that lands just before `before` (at the end without one) runs after and before, with the places of the
/// other steps as the flow shows them.
fn between(ctx: &Ctx, (flow, cards): (&PipelineFlow, &Cards), (moved, before): (&StepKey, Option<&StepKey>)) -> String {
  let stages: Vec<Vec<StepKey>> = flow
    .stages
    .iter()
    .map(|stage| {
      stage
        .cards
        .iter()
        .filter(|card| &card.key != moved)
        .map(|card| card.key.clone())
        .collect::<Vec<_>>()
    })
    .filter(|keys| !keys.is_empty())
    .collect();
  let at = before.map_or(stages.len(), |before| {
    stages
      .iter()
      .position(|keys| keys.contains(before))
      .unwrap_or(stages.len())
  });
  let previous = at.checked_sub(1).and_then(|index| stages.get(index));
  let next = stages.get(at);
  let name = |keys: &Vec<StepKey>| cards.group(ctx, keys).to_string();
  let key = match (previous, next) {
    (Some(previous), Some(next)) => {
      return ctx
        .t_args(
          "pipeline_hub.flow.move.between",
          [("previous", name(previous)), ("next", name(next))],
        )
        .to_string();
    }
    (None, Some(next)) => {
      return ctx
        .t_args("pipeline_hub.flow.move.first", [("next", name(next))])
        .to_string();
    }
    (Some(previous), None) if flow.deliver.is_some() => {
      return ctx
        .t_args("pipeline_hub.flow.move.beforeDeliver", [("previous", name(previous))])
        .to_string();
    }
    (Some(previous), None) => (previous, "pipeline_hub.flow.move.last"),
    (None, None) => return String::new(),
  };
  ctx.t_args(key.1, [("previous", name(key.0))]).to_string()
}

/// The line between two places of the flow, with its node on the rail.
pub(super) fn line(ctx: &Ctx, words: &Words) -> Element {
  let refused = matches!(words, Words::Refused(_));
  let color = if refused {
    PaletteColor::Warning
  } else {
    PaletteColor::Accent
  };
  let bar = Row::new()
    .id("pipeline-hub-flow-landing-bar")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(BAR))
    .rounded(crate::theme::ExtraRadius::Full.role())
    .background(color);
  let text: Vec<Element> = match words {
    Words::Goes { title, hint } => vec![
      title_text(title).into(),
      wrapping(hint, FlowText::Hint, PaletteColor::TextMuted),
    ],
    Words::Refused(text) | Words::Parallel(text) => {
      let tone = if refused {
        PaletteColor::Warning
      } else {
        PaletteColor::TextPrimary
      };
      vec![wrapping(text, FlowText::Hint, tone)]
    }
  };
  let cell = Column::new()
    .id("pipeline-hub-flow-landing")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(SpacingSize::Xs))
    .spacing(ExtraSpacing::Snug.role())
    .child(bar)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .with_children(text),
    );
  rail::row(
    ctx,
    "pipeline-hub-flow-row-landing",
    &Node::Landing,
    (cell.into(), ExtraSpacing::Loose.role()),
  )
}

impl Words {
  /// What stands under a card dropped on, and whether it refuses; none for the words of a line between two places.
  pub(super) fn under_card(&self) -> Option<(&str, bool)> {
    match self {
      Self::Parallel(words) => Some((words, false)),
      Self::Refused(reason) => Some((reason, true)),
      Self::Goes { .. } => None,
    }
  }
}

/// What dropping on a card does, or why a move was refused, under its stage: wraps to the panel's width, so a long
/// reason is whole.
pub(super) fn card_words(text: &str, refused: bool) -> Element {
  let tone = if refused {
    PaletteColor::Warning
  } else {
    PaletteColor::TextPrimary
  };
  Row::new()
    .id("pipeline-hub-flow-landing-parallel")
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().top(ExtraSpacing::Snug.role()))
    .child(wrapping(text, FlowText::Landing, tone))
    .into()
}

fn title_text(text: &str) -> Text {
  Text::new(text)
    .variant(FlowText::Landing.role())
    .color(PaletteColor::TextPrimary)
    .nowrap()
}

/// `text` that takes the room the row has and wraps within it, instead of running past the panel's edge.
fn wrapping(text: &str, style: FlowText, tone: PaletteColor) -> Element {
  Text::new(text)
    .id("pipeline-hub-flow-landing-words")
    .variant(style.role())
    .color(tone)
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .into()
}
