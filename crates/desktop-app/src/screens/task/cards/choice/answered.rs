//! A question with suggested answers once it no longer waits (`Question card · answered` `I6cZ5j`): quiet,
//! *Answered · 16:44*, each question with what was answered (a check and the option, or the person's own
//! words in quotes), and who answered when: the person, or automation through Orchester's MCP server. A
//! question whose agent stopped waiting before an answer reached it reads *Ended without an answer*, with the
//! options it offered; one whose run was interrupted while it waited says so.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  question::{Answer, QuestionState},
  run::Answerer,
  time::Timestamp,
};
use orchester_workspace::{display, tasks::timeline::ChoiceCard};

use super::super::{
  Place,
  frame::{Stands, frame, head, note},
};
use crate::{
  theme::{CardsText, ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    markdown::MarkdownView,
  },
};

/// Where the request stands, the answers given, and who gave them when.
struct Outcome<'a> {
  stands: Stands,
  answers: Option<&'a [Answer]>,
  context: Arc<str>,
}

pub(super) fn card(ctx: &mut Ctx, id: &str, (place, card): (&Place, &ChoiceCard)) -> Element {
  let Outcome {
    stands,
    answers,
    context,
  } = outcome(ctx, card);
  let title = super::title(ctx, card);
  let mut frame = frame(id.to_owned(), false)
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Loose.role())
    .child(head(
      id,
      (Icon::MessageCircleQuestion, IconSize::Md, PaletteColor::TextSecondary),
      (&title, CardsText::Title.role()),
      stands.into_element(id),
    ));
  for (index, question) in card.asked.request.questions().iter().enumerate() {
    let text = MarkdownView::new(format!("{id}-text-{index}"), question.text(), place.links.clone()).render(ctx);
    let lines: Vec<Element> = match answers.and_then(|answers| answers.get(index)) {
      Some(answer) => answer_lines(ctx, &format!("{id}-answer-{index}"), answer),
      None => question
        .options()
        .iter()
        .map(|option| Text::new(option.label()).variant(TypographyStyle::Caption).into())
        .collect(),
    };
    frame = frame.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .child(text)
        .with_children(lines),
    );
  }
  frame.child(note(&format!("{id}-context"), &context)).into()
}

fn outcome<'a>(ctx: &Ctx, card: &'a ChoiceCard) -> Outcome<'a> {
  let asked = display::clock(card.asked.request.asked_at());
  let (answers, by, at) = match &card.asked.state {
    QuestionState::Answered { answers, by, at } => (answers, *by, *at),
    QuestionState::Interrupted { at } => return ended(ctx, card, (&asked, *at)),
    // Shown here only while its run was interrupted.
    QuestionState::Waiting => {
      return Outcome {
        stands: Stands::interrupted(ctx.t("cards.state.interrupted")),
        answers: None,
        context: ctx.t_args(
          "cards.choice.lost",
          [("asked", asked.as_str()), ("agent", &*card.agent)],
        ),
      };
    }
  };
  let time = display::clock(at);
  let key = match by {
    Answerer::Person => "cards.choice.byYou",
    Answerer::Automation if card.asked.by_orchestrator => "cards.choice.byOrchestrator",
    Answerer::Automation => "cards.choice.byAutomation",
  };
  Outcome {
    stands: Stands {
      glyph: Icon::CircleCheck,
      words: ctx.t_args("cards.state.answeredAt", [("time", time.as_str())]),
      color: PaletteColor::Success,
    },
    answers: Some(answers),
    context: ctx.t_args(
      key,
      [
        ("asked", asked.as_str()),
        ("time", time.as_str()),
        ("agent", &*card.agent),
      ],
    ),
  }
}

/// *Ended without an answer*: the agent stopped waiting at `at`.
fn ended<'a>(ctx: &Ctx, card: &ChoiceCard, (asked, at): (&str, Timestamp)) -> Outcome<'a> {
  let time = display::clock(at);
  Outcome {
    stands: Stands {
      glyph: Icon::ClockAlert,
      words: ctx.t("cards.state.ended"),
      color: PaletteColor::TextMuted,
    },
    answers: None,
    context: ctx.t_args(
      "cards.choice.interrupted",
      [("asked", asked), ("time", time.as_str()), ("agent", &*card.agent)],
    ),
  }
}

/// What was answered: a check and each option picked, then the person's own words in quotes.
fn answer_lines(ctx: &Ctx, id: &str, answer: &Answer) -> Vec<Element> {
  let line = |index: usize, words: &str| -> Element {
    Row::new()
      .id(format!("{id}-{index}"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Check, IconSize::Sm, PaletteColor::Success))
      .child(
        Text::new(words)
          .variant(TypographyStyle::Description)
          .color(PaletteColor::TextPrimary),
      )
      .into()
  };
  let own = answer
    .text()
    .map(|text| ctx.t_args("cards.choice.own", [("text", text)]));
  answer
    .choices()
    .iter()
    .map(String::as_str)
    .chain(own.as_deref())
    .enumerate()
    .map(|(index, words)| line(index, words))
    .collect()
}
