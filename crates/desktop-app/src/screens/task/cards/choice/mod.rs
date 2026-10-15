//! An agent's question with suggested answers (`Question card` `tKYgO`, its options and own answer as the
//! Needs-you panel answers a question in place, `Z4HeE3` in `zwuZt`): *Builder asks you*, then each question
//! with the agent's options, as radios, or as checkboxes where several may be picked, and *Or write your own
//! answer…* ([`question`]); *Send answer* once every question has an answer, which resumes the agent at once.
//! In the panel each question's text sits in a raised box, *Open conversation* and *Send answer* share one
//! row, and a line under it says that Enter sends, which it does from an own answer. What the person picked
//! and typed is the tab's (`Decisions`), so it stays while the card scrolls out of the chat. Once it no longer
//! waits it says what was answered, by whom and when, or that the agent stopped waiting first ([`answered`]).

mod answered;
mod question;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::KeyboardEvent,
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::question::{Question, QuestionState};
use orchester_workspace::{
  display,
  tasks::{
    question::{Answering, Draft},
    timeline::ChoiceCard,
  },
};
use parking_lot::Mutex;

use super::{
  Place, Press,
  decisions::{Decisions, Given, Refused, Stage, refusal},
  frame::{Stands, head, held, note, waiting_frame},
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

/// Whether `card` waits for an answer, while its run `lost` its runner or not: waiting while its run was
/// interrupted, it no longer waits either.
pub(super) fn waits(card: &ChoiceCard, lost: bool) -> bool {
  matches!(card.asked.state, QuestionState::Waiting) && !lost
}

/// The own answers' fields, each question's: what they hold, and their inputs.
#[derive(Clone)]
struct Own {
  fields: Arc<[Signal<String>]>,
  inputs: Arc<[ElementRef]>,
}

struct ChoiceView {
  /// The first option, which takes the focus when the tab opens on the card.
  first: ElementRef,
  /// The own answers' fields, as kept when the card was first shown.
  own: Mutex<Option<Own>>,
}

impl Component for ChoiceView {
  type Props = ChoiceProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      first: ElementRef::new(),
      own: Mutex::new(None),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ChoiceProps { place, card } = ctx.props::<ChoiceProps>().clone();
    let decisions = Decisions::of(ctx);
    let focus = decisions.take_focus(&place.key);
    let id = format!("cards-choice-{}", card.asked.request.id());
    if !waits(&card, decisions.lost()) {
      return answered::card(ctx, &id, (&place, &card));
    }
    let questions: Arc<[Question]> = card.asked.request.questions().into();
    let draft = decisions.draft((place.run(), &place.key), &questions);
    let own = self.own_fields(ctx, (&place, &questions), &draft);
    let stands = Stands::waiting(ctx.t("cards.state.waitingAnswer")).into_element(&id);
    let title = title(ctx, &card);
    let padding = Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md);
    let mut body = waiting_frame(&place, id.clone(), padding, || {
      head(
        &id,
        (Icon::MessageCircleQuestion, IconSize::Md, PaletteColor::Warning),
        (&title, CardsText::Title.role()),
        stands,
      )
      .into()
    })
    .spacing(ExtraSpacing::Loose.role());
    for index in 0..questions.len() {
      let shown = question::Shown {
        id: &id,
        place: &place,
        card: &card,
        questions: &questions,
        draft: &draft,
        own: &own.fields,
        input: own.inputs[index].clone(),
        decisions: &decisions,
        first: &self.first,
      };
      body = body.child(question::question(ctx, &shown, index));
    }
    if focus {
      ctx.focus(&self.first);
    }
    let (footer, send) = footer(ctx, (&id, &place, &card), &draft, &decisions);
    let body = body.child(footer);
    if !place.in_panel {
      return body.into();
    }
    body.on_key_down(enter_sends(own.inputs, send)).into()
  }
}

impl ChoiceView {
  /// The own answers' fields: what is kept of them the first time the card is shown; then the fields keep
  /// what is typed, and tell it to what is kept as it is typed.
  ///
  /// Whatever sets a field's value (typing, a paste, a tool), it reaches what is kept: typing picks the own
  /// answer's row, replacing a single-choice question's pick; erasing it unpicks the row and brings no earlier
  /// pick back.
  fn own_fields(&self, ctx: &mut Ctx, (place, questions): (&Place, &Arc<[Question]>), draft: &Draft) -> Own {
    let mut own = self.own.lock();
    if let Some(own) = &*own {
      return own.clone();
    }
    let decisions = Decisions::of(ctx);
    let fields: Arc<[Signal<String>]> = (0..questions.len())
      .map(|index| Signal::new(draft.of(index).map(|picks| picks.text().to_owned()).unwrap_or_default()))
      .collect();
    for (index, field) in fields.iter().enumerate() {
      let (decisions, run, key, all) = (decisions.clone(), place.run(), place.key.clone(), questions.clone());
      ctx.watch(field, move |text: &String| {
        decisions.change_draft((run, &key), &all, |draft| draft.type_own(&all, index, text));
      });
    }
    let kept = Own {
      fields,
      inputs: (0..questions.len()).map(|_| ElementRef::new()).collect(),
    };
    *own = Some(kept.clone());
    kept
  }
}

/// Enter in one of the own answers sends the answer, when it can be sent; never a line break, also while an
/// input method composes, whose Enter only confirms. An option takes Enter itself, which flips it.
fn enter_sends(inputs: Arc<[ElementRef]>, send: Option<Press>) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  move |event: KeyboardEvent| {
    let plain = !(event.shift || event.ctrl || event.alt || event.meta);
    if event.key != "Enter" || !plain || !inputs.iter().any(ElementRef::focused) {
      return;
    }
    event.prevent_default();
    if event.composing {
      return;
    }
    if let Some(send) = &send {
      send();
    }
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

/// When it was asked and what an answer does, or why it cannot be sent yet; *Send answer*; that it was sent,
/// or why it did not go through. In the panel *Open conversation* comes first in the answers row, and the
/// line under it says what an answer does and that Enter sends (`v2Yya1`, `tc9Wu` in `Z4HeE3`). With it, what
/// sends the answer while it can be sent.
fn footer(
  ctx: &mut Ctx,
  (id, place, card): (&str, &Place, &ChoiceCard),
  draft: &Draft,
  decisions: &Decisions,
) -> (Element, Option<Press>) {
  let context = context(ctx, (place, card), draft);
  let stage = decisions
    .outcome((place.run(), &place.key))
    .map(|outcome| outcome.stage);
  let send = sender(ctx, (place, card), draft, (decisions, stage.as_ref()));
  let button = ActionButton::new(format!("{id}-send"), ButtonKind::Primary, ctx.t("cards.choice.send"))
    .icon(Icon::Reply)
    .in_form();
  let button = match (&stage, &send) {
    (Some(Stage::Sending), _) => button.loading(),
    (_, Some(send)) => {
      let send = send.clone();
      button.on_click(move || send())
    }
    (_, None) => button.disabled(),
  };
  let sending = stage == Some(Stage::Sending);
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
  let answers: Vec<Element> = match &place.conversation {
    Some(conversation) if place.in_panel => {
      let conversation = conversation.clone();
      let open = ActionButton::new(
        format!("{id}-conversation"),
        ButtonKind::Ghost,
        ctx.t("cards.choice.openConversation"),
      )
      .icon(Icon::ArrowUpRight)
      .on_click(move || conversation());
      vec![
        Row::new()
          .id(format!("{id}-answers"))
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(open)
          .child(Spacer::new().flex(1.0))
          .child(button)
          .into(),
        note(&format!("{id}-context"), &context).into(),
      ]
    }
    _ => vec![
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Loose.role())
        .align_items(Alignment::Center)
        .child(
          note(&format!("{id}-context"), &context)
            .flex(1.0)
            .min_width(Dimension::Px(0.0)),
        )
        .child(Row::new().flex_shrink(0.0).child(button))
        .into(),
    ],
  };
  let footer = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(answers)
    .with_children((decisions.busy() && !sending).then(|| held(ctx, id)))
    .with_children(after)
    .into();
  (footer, send)
}

/// What sending the answer does, while it can be sent: every question has an answer, no other answer of the
/// tab is on its way, this one was neither sent nor refused for good, and the project is open.
fn sender(
  ctx: &mut Ctx,
  (place, card): (&Place, &ChoiceCard),
  draft: &Draft,
  (decisions, stage): (&Decisions, Option<&Stage>),
) -> Option<Press> {
  let blocked = matches!(
    stage,
    Some(Stage::Sending | Stage::Taken(_) | Stage::Refused(Refused::Ended | Refused::Answered))
  );
  if blocked || draft.missing().is_some() || decisions.busy() {
    return None;
  }
  let open = place.open(ctx)?;
  let answering = Answering {
    run: card.asked.request.run(),
    request: card.asked.request.id().to_owned(),
    answers: draft.given(card.asked.request.questions()),
  };
  let (decisions, run, key) = (decisions.clone(), place.run(), place.key.clone());
  Some(Arc::new(move || {
    let command = |by| answering.clone().command(by);
    decisions.answer((run, &key), Given::Answers, (&open, command));
  }))
}

/// Why it cannot be sent yet; else when it was asked and what an answer does, or in the panel what an answer
/// does and that Enter sends.
fn context(ctx: &Ctx, (place, card): (&Place, &ChoiceCard), draft: &Draft) -> Arc<str> {
  let questions = card.asked.request.questions();
  let time = display::clock(card.asked.request.asked_at());
  match (&place.step, draft.missing()) {
    (_, Some(_)) if questions.len() == 1 => ctx.t_args("cards.choice.missingOne", [("agent", &*card.agent)]),
    (_, Some(index)) => ctx.t_args("cards.choice.missing", [("number", (index + 1).to_string().as_str())]),
    _ if place.in_panel => ctx.t_args("cards.choice.enterSends", [("agent", &*card.agent)]),
    (Some(step), None) => ctx.t_args(
      "cards.choice.waiting",
      [("time", time.as_str()), ("step", &**step), ("agent", &*card.agent)],
    ),
    (None, None) => ctx.t_args(
      "cards.choice.waitingNoStep",
      [("time", time.as_str()), ("agent", &*card.agent)],
    ),
  }
}
