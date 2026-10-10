//! One question of a waiting card: its number among several and its header, what it asks, the agent's
//! options (radios, or checkboxes where several may be picked, each with its description under it) and the
//! person's own answer.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::question::Question;
use orchester_workspace::tasks::{question::Draft, timeline::ChoiceCard};

use super::super::{Place, decisions::Decisions};
use crate::{
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    chip::TagChip,
    field::FieldInput,
    markdown::MarkdownView,
    toggle::{Toggle, ToggleKind},
  },
};

/// What a question of the card is shown from: the card, its questions, what is kept and picked, and the
/// first option, which takes the focus when the tab opens on the card.
pub(super) struct Shown<'a> {
  pub(super) id: &'a str,
  pub(super) place: &'a Place,
  pub(super) card: &'a ChoiceCard,
  pub(super) questions: &'a Arc<[Question]>,
  pub(super) draft: &'a Draft,
  /// Each question's own answer, as its field holds it.
  pub(super) own: &'a Arc<[Signal<String>]>,
  pub(super) decisions: &'a Decisions,
  pub(super) first: &'a ElementRef,
}

/// Question `index`.
pub(super) fn question(ctx: &mut Ctx, shown: &Shown<'_>, index: usize) -> Element {
  let (id, question, count) = (shown.id, &shown.questions[index], shown.questions.len());
  let numbered = (count > 1 || question.header().is_some()).then(|| {
    let number = (count > 1).then(|| {
      Text::new(&ctx.t_args(
        "cards.choice.number",
        [
          ("number", (index + 1).to_string().as_str()),
          ("count", count.to_string().as_str()),
        ],
      ))
      .variant(ExtraTypography::Note.role())
      .nowrap()
    });
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .with_children(number)
      .with_children(
        question
          .header()
          .map(|header| TagChip::new(format!("{id}-header-{index}"), header)),
      )
  });
  let text = MarkdownView::new(format!("{id}-text-{index}"), question.text(), shown.place.links.clone()).render(ctx);
  let label = ctx.t_args(
    if question.multi_select() {
      "cards.choice.optionsSeveral"
    } else {
      "cards.choice.options"
    },
    [("agent", &*shown.card.agent)],
  );
  let field = FieldInput::new(
    format!("{id}-own-{index}"),
    ctx.t("cards.choice.ownName"),
    shown.own[index].clone(),
  )
  .placeholder(ctx.t("cards.choice.ownPlaceholder"))
  .render(ctx);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(numbered)
    .child(text)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .child(Text::new(&label).variant(ExtraTypography::Note.role()))
        .child(options(shown, index)),
    )
    .child(field)
    .into()
}

/// The agent's options for question `index`.
fn options(shown: &Shown<'_>, index: usize) -> Column {
  let (id, question) = (shown.id, &shown.questions[index]);
  let draft = shown.draft;
  let kind = if question.multi_select() {
    ToggleKind::Checkbox
  } else {
    ToggleKind::Radio
  };
  let picks = draft.of(index).cloned().unwrap_or_default();
  let rows = question.options().iter().enumerate().map(|(option, offered)| {
    let (decisions, own) = (shown.decisions.clone(), shown.own[index].clone());
    let (run, key) = (shown.place.run(), shown.place.key.clone());
    let (all, multi) = (shown.questions.clone(), question.multi_select());
    let mut toggle = Toggle::new(
      format!("{id}-option-{index}-{option}"),
      kind,
      picks.picked(option),
      offered.label(),
    )
    .ring_room()
    .on_change(move |_| {
      // A picked option of a single-choice question replaces the own answer: the agent takes one.
      if !multi {
        own.set(String::new());
      }
      decisions.change_draft((run, &key), &all, |draft| {
        draft.pick(&all, (index, option));
        if !multi {
          draft.type_own(&all, index, "");
        }
      });
    });
    if (index, option) == (0, 0) {
      toggle = toggle.element_ref(shown.first.clone());
    }
    Column::new()
      .spacing(ExtraSpacing::Tight.role())
      .child(toggle)
      // Under the option's words: past the 16 control and the 8 after it.
      .with_children(offered.description().map(|description| {
        Row::new()
          .padding_left(SpacingSize::Xl)
          .child(Text::new(description).variant(TypographyStyle::Caption))
      }))
  });
  Column::new()
    .id(format!("{id}-options-{index}"))
    .spacing(ExtraSpacing::Loose.role())
    .with_children(rows)
}
