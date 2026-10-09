//! An agent's question with suggested answers (`Question card` `tKYgO`, its options and own answer as the
//! Needs-you panel answers a question in place, `Z4HeE3` in `zwuZt`): *Builder asks you*, then each question
//! with the agent's options, as radios, or as checkboxes where several may be picked, and *Or write your own
//! answer…* ([`question`]); *Send answer* once every question has an answer, which resumes the agent at once.
//! What the person picked and typed is the tab's (`Decisions`), so it stays while the card scrolls out of the
//! chat. Once it no longer waits it says what was answered, by whom and when, or that the agent stopped
//! waiting first ([`answered`]).

mod answered;
mod question;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  core::ElementRef,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  question::{Question, QuestionState},
  run::Answerer,
};
use orchester_workspace::{
  display,
  tasks::{
    question::{Answering, Draft},
    timeline::ChoiceCard,
  },
};

use super::{
  Place,
  decisions::{ChoiceDraft, Decisions, Given, Picked, Refused, Stage, refusal},
  frame::{Stands, frame, head, note},
};
use crate::{
  theme::{CardsText, ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

#[derive(Clone, PartialEq)]
struct ChoiceProps {
  place: Place,
  card: ChoiceCard,
}

impl lurq::app::component::DevtoolsInspectable for ChoiceProps {}

pub(super) fn card(ctx: &mut Ctx, place: Place, card: &ChoiceCard) -> Element {
  ctx.mount_keyed::<ChoiceView>(
    &format!("cards-choice-{}", card.asked.request.id()),
    ChoiceProps {
      place,
      card: card.clone(),
    },
  )
}

struct ChoiceView {
  /// The first option, which takes the focus when the tab opens on the card.
  first: ElementRef,
}

impl Component for ChoiceView {
  type Props = ChoiceProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      first: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ChoiceProps { place, card } = ctx.props::<ChoiceProps>().clone();
    let decisions = Decisions::of(ctx);
    let focus = decisions.take_focus(&place.key);
    let id = format!("cards-choice-{}", card.asked.request.id());
    // Waiting while its run was interrupted, it no longer waits either.
    if !matches!(card.asked.state, QuestionState::Waiting) || decisions.lost() {
      return answered::card(ctx, &id, (&place, &card));
    }
    let questions: Arc<[Question]> = card.asked.request.questions().into();
    let kept = decisions.draft(&place.key, &questions);
    let draft = synced(&kept, &questions);
    let stands = Stands::waiting(ctx.t("cards.state.waitingAnswer")).into_element(&id);
    let mut body = frame(id.clone(), true)
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .child(head(
        &id,
        (Icon::MessageCircleQuestion, IconSize::Md, PaletteColor::Warning),
        (&title(ctx, &card), CardsText::Title.role()),
        stands,
      ));
    for index in 0..questions.len() {
      let shown = question::Shown {
        id: &id,
        place: &place,
        card: &card,
        questions: &questions,
        draft: (&kept, &draft),
        first: &self.first,
      };
      body = body.child(question::question(ctx, &shown, index));
    }
    if focus {
      ctx.focus(&self.first);
    }
    body.child(footer(ctx, (&id, &place, &card), &draft, &decisions)).into()
  }
}

/// *Builder asks you*, or how many questions it asks.
fn title(ctx: &Ctx, card: &ChoiceCard) -> Arc<str> {
  match card.asked.request.questions().len() {
    1 => ctx.t_args("cards.question.asks", [("agent", &*card.agent)]),
    count => ctx.t_args(
      "cards.question.asksSeveral",
      [("agent", &*card.agent), ("count", count.to_string().as_str())],
    ),
  }
}

/// What is picked, with each own answer as typed taken in: typing picks the own answer's row, replacing a
/// single-choice question's pick; erasing it unpicks the row and brings no earlier pick back.
fn synced(kept: &ChoiceDraft, questions: &[Question]) -> Draft {
  let mut draft = kept.picked.get().0;
  let mut changed = false;
  for (index, own) in kept.own.iter().enumerate() {
    let text = own.get();
    if draft.of(index).is_some_and(|picks| picks.text() != text) {
      draft.type_own(questions, index, &text);
      changed = true;
    }
  }
  if changed {
    kept.picked.set(Picked(draft.clone()));
  }
  draft
}

/// When it was asked and what an answer does, or why it cannot be sent yet; *Send answer*; that it was sent,
/// or why it did not go through.
fn footer(
  ctx: &mut Ctx,
  (id, place, card): (&str, &Place, &ChoiceCard),
  draft: &Draft,
  decisions: &Decisions,
) -> Element {
  let questions = card.asked.request.questions();
  let missing = draft.missing();
  let time = display::clock(card.asked.request.asked_at());
  let context = match (&place.step, missing) {
    (_, Some(_)) if questions.len() == 1 => ctx.t_args("cards.choice.missingOne", [("agent", &*card.agent)]),
    (_, Some(index)) => ctx.t_args("cards.choice.missing", [("number", (index + 1).to_string().as_str())]),
    (Some(step), None) => ctx.t_args(
      "cards.choice.waiting",
      [("time", time.as_str()), ("step", &**step), ("agent", &*card.agent)],
    ),
    (None, None) => ctx.t_args(
      "cards.choice.waitingNoStep",
      [("time", time.as_str()), ("agent", &*card.agent)],
    ),
  };
  let stage = decisions.outcome(&place.key).map(|outcome| outcome.stage);
  let button = ActionButton::new(format!("{id}-send"), ButtonKind::Primary, ctx.t("cards.choice.send"))
    .icon(Icon::Reply)
    .in_form();
  let button = match (&stage, place.open(ctx)) {
    (Some(Stage::Sending), _) => button.loading(),
    (Some(Stage::Taken(_) | Stage::Refused(Refused::Ended | Refused::Answered)), _) | (_, None) => button.disabled(),
    _ if missing.is_some() || decisions.busy() => button.disabled(),
    (_, Some(open)) => {
      let command = Answering {
        run: card.asked.request.run(),
        request: card.asked.request.id().to_owned(),
        answers: draft.given(questions),
      }
      .command(Answerer::Person);
      let (decisions, key) = (decisions.clone(), place.key.clone());
      button.on_click(move || decisions.answer(&key, Given::Answers, (&open, command.clone())))
    }
  };
  let after = match stage {
    Some(Stage::Taken(_)) => Some(
      note(
        &format!("{id}-sent"),
        &ctx.t_args("cards.choice.sent", [("agent", &*card.agent)]),
      )
      .into(),
    ),
    Some(Stage::Refused(refused)) => Some(refusal(ctx, id, &refused, &card.agent)),
    Some(Stage::Sending) | None => None,
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(
          note(&format!("{id}-context"), &context)
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(Row::new().flex_shrink(0.0).child(button)),
    )
    .with_children(after)
    .into()
}
