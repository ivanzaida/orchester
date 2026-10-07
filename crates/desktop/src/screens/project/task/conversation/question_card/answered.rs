//! The question card once it no longer waits (C·17 `UpHyY`, states E and F): *Builder asked you*, *Answered*,
//! the question, then only what was answered, one line each (a check, the key and the label, or the own text
//! in quotes), and who answered when, with *Show all options*, which lists every option read-only. A
//! question whose agent ended before an answer reached it reads *Ended without an answer* with its options.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  question::{Answer, Question, QuestionState},
  run::Answerer,
};

use super::{super::cards::frame, head, rows::key};
use crate::{
  display,
  tasks::timeline::ChoiceCard,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    icon::{Icon, IconSize, icon},
    markdown::MarkdownText,
    pressable::{pressable, surface},
  },
};

pub(super) fn card(ctx: &mut Ctx, card: &ChoiceCard, show_all: &Signal<bool>) -> Element {
  let asked = &card.asked;
  let questions = asked.request.questions();
  let title = ctx.t_args("runs.conversation.question.asked", [("agent", &*card.agent)]);
  let (state, answers) = match &asked.state {
    QuestionState::Answered { answers, .. } => ((Icon::Check, ctx.t("runs.conversation.card.answered")), Some(answers)),
    QuestionState::Interrupted { .. } | QuestionState::Waiting => {
      ((Icon::CirclePause, ctx.t("runs.conversation.card.ended")), None)
    }
  };
  let chip = match questions {
    [question] => question.header(),
    _ => None,
  };
  let all = show_all.get() || answers.is_none();
  let mut body: Vec<Element> = Vec::new();
  for (index, question) in questions.iter().enumerate() {
    let answer = answers.and_then(|answers| answers.get(index));
    body.push(question_record(ctx, card, (index, question), (answer, all)));
  }
  body.extend(by_line(ctx, card, show_all));
  frame(
    format!("conversation-question-card-{}", asked.request.id()),
    false,
    head((Icon::MessageCircleQuestion, &title, chip), (state.0, &state.1)),
    body,
  )
  .spacing(ExtraSpacing::Stack.role())
  .into()
}

/// One question as recorded: its text, then what was answered, or every option when `all`.
fn question_record(
  ctx: &mut Ctx,
  card: &ChoiceCard,
  (index, question): (usize, &Question),
  (answer, all): (Option<&Answer>, bool),
) -> Element {
  let text = MarkdownText::message(
    format!("conversation-question-{}-{index}", card.asked.request.id()),
    question.text(),
  )
  .mount(ctx);
  let lines: Vec<Element> = if all {
    every_option(question, answer)
  } else {
    answer
      .map(|answer| answered_lines(ctx, question, answer))
      .unwrap_or_default()
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(text)
    .child(
      Column::new()
        .id(format!("conversation-question-answered-{index}"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Tight.role())
        .with_children(lines),
    )
    .into()
}

/// What was answered, one line each: a check, the key and the label; the own text in quotes.
fn answered_lines(ctx: &Ctx, question: &Question, answer: &Answer) -> Vec<Element> {
  let picked = answer.choices().iter().filter_map(|choice| {
    let index = question.options().iter().position(|option| option.label() == choice)?;
    Some(line(Some(index), Text::new(choice).variant(TypographyStyle::Body)))
  });
  let own = answer.text().map(|text| {
    let quoted = ctx.t_args("runs.conversation.question.ownAnswer", [("text", text)]);
    line(
      Some(question.options().len()),
      Text::new(&quoted).variant(TypographyStyle::Body),
    )
  });
  picked.chain(own).collect()
}

fn line(index: Option<usize>, label: Text) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Check, IconSize::Event, PaletteColor::TextPrimary))
    .with_children(index.map(|index| key(index, true)))
    .child(label.flex_shrink(1.0).min_width(Dimension::Px(0.0)))
    .into()
}

/// Every option read-only, the picked ones marked.
fn every_option(question: &Question, answer: Option<&Answer>) -> Vec<Element> {
  question
    .options()
    .iter()
    .enumerate()
    .map(|(index, option)| {
      let picked = answer.is_some_and(|answer| answer.choices().iter().any(|choice| choice == option.label()));
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding_vertical(ExtraSpacing::Snug.role())
        .padding_horizontal(ExtraSpacing::Inline.role())
        .spacing(ExtraSpacing::Inline.role())
        .align_items(Alignment::Start)
        .rounded(RadiusSize::Md)
        .child(key(index, picked))
        .child(
          Column::new()
            .flex(1.0)
            .min_width(Dimension::Px(0.0))
            .child(Text::new(option.label()).variant(TypographyStyle::Body))
            .with_children(
              option
                .description()
                .map(|description| Text::new(description).variant(TypographyStyle::Description)),
            ),
        )
        .into()
    })
    .collect()
}

/// *Answered by you · 16:44 · Builder continued* (or through Orchester's MCP server), with *Show all
/// options* at the right; nothing for a question that ended without an answer.
fn by_line(ctx: &Ctx, card: &ChoiceCard, show_all: &Signal<bool>) -> Option<Element> {
  let QuestionState::Answered { by, at, .. } = &card.asked.state else {
    return None;
  };
  let key = match by {
    Answerer::Person => "runs.conversation.question.answeredByYou",
    Answerer::Automation => "runs.conversation.question.answeredByMcp",
  };
  let said: Arc<str> = ctx.t_args(key, [("time", display::clock(*at).as_str()), ("agent", &*card.agent)]);
  let toggle = ctx.t(if show_all.get() {
    "runs.conversation.question.hideOptions"
  } else {
    "runs.conversation.question.showOptions"
  });
  let shown = show_all.clone();
  Some(
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::CornerDownRight, IconSize::Tag, PaletteColor::TextSecondary))
      .child(
        Text::new(&said)
          .id("conversation-question-by")
          .variant(ExtraTypography::Hint.role())
          .color(PaletteColor::TextSecondary)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(Spacer::new().flex(1.0))
      .child(
        pressable(surface().id("conversation-question-all"), move || {
          shown.set(!shown.get_untracked());
        })
        .child(
          Text::new(&toggle)
            .variant(TypographyStyle::Link)
            .color(PaletteColor::TextPrimary)
            .nowrap(),
        ),
      )
      .into(),
  )
}
