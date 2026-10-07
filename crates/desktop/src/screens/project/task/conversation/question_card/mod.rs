//! An agent's question with suggested answers in the chat (C·15 `lS9BF`, C·16 `juFIX`, states C·17 `UpHyY`):
//! the question card of C·02 in the asking agent's message, with the options lettered A, B, C… as radio rows
//! (single choice) or checkbox rows (several allowed), each with its description, and *Type your own
//! answer* always last; *Answer* sends once each question has an answer. Several questions of one request
//! are numbered and answered together. Answered, the card keeps only what was answered and who answered,
//! with *Show all options* ([`answered`]).
//!
//! Keyboard ([`keys`]): the card never takes the focus by itself; Tab or a click reaches it. A letter
//! picks its row (the own answer's letter focuses its field), ↑ ↓ move, Space picks or checks, Enter
//! answers, Esc in the field goes back to the options. The inset focus ring marks the row the keyboard
//! is on; a row picked with the pointer shows none.

mod answered;
mod keys;
mod rows;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  question::{Question, QuestionRequest, QuestionState},
  run::Answerer,
};
use parking_lot::Mutex;

use self::keys::{Spot, card_keys};
use super::{
  cards::frame,
  chat::{Avatar, chat_row},
};
use crate::{
  display,
  messages::Message,
  runner::Runners,
  tasks::{
    question::{Answering, Draft},
    run::control_message,
    timeline::ChoiceCard,
  },
  theme::{ExtraColor, ExtraSpacing, ExtraTypography},
  ui::{
    button::ActionButton,
    field::field_error,
    icon::{Icon, IconSize, icon},
    markdown::MarkdownText,
    message_text,
    status_label::compact_status_label,
  },
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ChoiceCardProps {
  pub(super) card: ChoiceCard,
}

impl lurq::app::component::DevtoolsInspectable for ChoiceCardProps {}

type AnswerAction = FutureAction<Answering, (), Message>;

fn answer_action(ctx: &mut Ctx) -> AnswerAction {
  let runners = ctx
    .use_context::<Runners>()
    .expect("the root provides Runners before mounting screens");
  ctx.future_action(move |answering: Answering| {
    let runners = runners.clone();
    async move {
      let command = answering.command(Answerer::Person);
      runners.control_run(command).await.map_err(|error| {
        tracing::warn!(%error, "the runner did not take the answer");
        control_message(&error)
      })
    }
  })
}

/// The own answer of each question: what is typed in its field, and the field.
#[derive(Default)]
struct Fields {
  texts: Vec<Signal<String>>,
  inputs: Vec<ElementRef>,
  /// Each question's option rows.
  rows: Vec<Vec<ElementRef>>,
}

impl Fields {
  fn shaped(questions: &[Question]) -> Self {
    Self {
      texts: questions.iter().map(|_| Signal::new(String::new())).collect(),
      inputs: questions.iter().map(|_| ElementRef::new()).collect(),
      rows: questions
        .iter()
        .map(|question| question.options().iter().map(|_| ElementRef::new()).collect())
        .collect(),
    }
  }
}

/// What the card's rows and keys share.
#[derive(Clone)]
struct CardState {
  request: Arc<QuestionRequest>,
  draft: Signal<Draft>,
  /// The row the keyboard focus is on, as its focus and blur report it.
  focused: Signal<Option<Spot>>,
  /// The last input was a key, so the focused row shows the ring.
  keyed: Signal<bool>,
  /// A row the keys move the focus to, at the next render.
  focus_to: Signal<Option<Spot>>,
  fields: Arc<Mutex<Fields>>,
  action: AnswerAction,
}

impl CardState {
  /// Sends the answers, once every question has one and none is on its way.
  fn submit(&self) {
    let draft = self.draft.get_untracked();
    let state = self.action.state().get_untracked();
    if draft.missing().is_some() || state.is_pending() || state.data.is_some() {
      return;
    }
    let questions = self.request.questions();
    self.action.run(Answering {
      run: self.request.run(),
      request: self.request.id().to_owned(),
      answers: draft.given(questions),
    });
  }

  /// Marks how the person reaches the card, as the ring shows; set only on a change.
  fn set_keyed(&self, keyed: bool) {
    if self.keyed.get_untracked() != keyed {
      self.keyed.set(keyed);
    }
  }
}

pub(super) struct ChoiceCardView {
  draft: Signal<Draft>,
  seeded: Signal<bool>,
  focused: Signal<Option<Spot>>,
  keyed: Signal<bool>,
  focus_to: Signal<Option<Spot>>,
  show_all: Signal<bool>,
  fields: Arc<Mutex<Fields>>,
}

impl Component for ChoiceCardView {
  type Props = ChoiceCardProps;

  fn create(ctx: &mut Ctx) -> Self {
    answer_action(ctx);
    Self {
      draft: ctx.signal(Draft::default()),
      seeded: ctx.signal(false),
      focused: ctx.signal(None),
      keyed: ctx.signal(false),
      focus_to: ctx.signal(None),
      show_all: ctx.signal(false),
      fields: Arc::new(Mutex::new(Fields::default())),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChoiceCardProps>().clone();
    let action = answer_action(ctx);
    let asked = &props.card.asked;
    let questions = asked.request.questions();
    if !self.seeded.get_untracked() {
      self.seeded.set(true);
      self.draft.set(Draft::new(questions));
      *self.fields.lock() = Fields::shaped(questions);
    }
    let state = CardState {
      request: Arc::new(asked.request.clone()),
      draft: self.draft.clone(),
      focused: self.focused.clone(),
      keyed: self.keyed.clone(),
      focus_to: self.focus_to.clone(),
      fields: self.fields.clone(),
      action,
    };
    self.take_focus_request(ctx);
    let card = match &asked.state {
      QuestionState::Waiting => waiting(ctx, &props.card, &state),
      QuestionState::Answered { .. } | QuestionState::Interrupted { .. } => {
        answered::card(ctx, &props.card, &self.show_all)
      }
    };
    let meta = ctx.t_args(
      "runs.conversation.agentMeta",
      [
        ("step", &*props.card.step),
        ("time", display::clock(asked.request.asked_at()).as_str()),
      ],
    );
    chat_row(
      format!("conversation-choice-{}", asked.request.id()),
      Avatar::Question,
      (&props.card.agent, &meta),
      vec![card],
    )
  }
}

impl ChoiceCardView {
  /// Moves the focus where a key asked it to go.
  fn take_focus_request(&self, ctx: &Ctx) {
    let Some(spot) = self.focus_to.get_untracked() else {
      return;
    };
    self.focus_to.set(None);
    let fields = self.fields.lock();
    let target = match spot.row {
      keys::RowAt::Option(option) => fields.rows.get(spot.question).and_then(|rows| rows.get(option)),
      keys::RowAt::Own => fields.inputs.get(spot.question),
    };
    if let Some(target) = target {
      ctx.focus(target);
    }
  }
}

/// The card while it waits: who asks and how many questions, each question with its options and own
/// answer, then the keys' hint and *Answer*.
fn waiting(ctx: &mut Ctx, card: &ChoiceCard, state: &CardState) -> Element {
  let questions = state.request.questions();
  let title = match questions.len() {
    1 => ctx.t_args("runs.conversation.question.asks", [("agent", &*card.agent)]),
    count => ctx.t_args(
      "runs.conversation.question.asksSeveral",
      [("agent", &*card.agent), ("count", count.to_string().as_str())],
    ),
  };
  let chip = match questions {
    [question] => question.header(),
    _ => None,
  };
  let waiting = ctx.t("runs.conversation.card.waitingAnswer");
  let head = head(
    (Icon::MessageCircleQuestion, &title, chip),
    (Icon::MessageCircle, &waiting),
  );
  let mut body: Vec<Element> = Vec::new();
  for (index, question) in questions.iter().enumerate() {
    if index > 0 {
      body.push(hairline());
    }
    body.push(question_block(ctx, state, (index, question)));
  }
  body.push(footer(ctx, state));
  frame(
    format!("conversation-question-card-{}", state.request.id()),
    true,
    head,
    body,
  )
  .spacing(ExtraSpacing::Stack.role())
  .on_key_down(card_keys(state.clone()))
  .into()
}

/// The card's head: glyph, title, the question's header as a chip, and the status label at the right.
fn head((glyph, title, chip): (Icon, &str, Option<&str>), (state_glyph, state): (Icon, &str)) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Event, PaletteColor::TextPrimary))
    .child(
      Text::new(title)
        .variant(ExtraTypography::NoticeHeading.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .with_children(chip.map(rows::header_chip))
    .child(Spacer::new().flex(1.0))
    .child(compact_status_label(state_glyph, state))
    .into()
}

/// A rule between two questions of one request.
fn hairline() -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .into()
}

/// One question: its number among several (with its own header), its text, how many it takes, then its
/// rows.
fn question_block(ctx: &mut Ctx, state: &CardState, (index, question): (usize, &Question)) -> Element {
  let count = state.request.questions().len();
  let numbered: Option<Element> = (count > 1).then(|| {
    Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t_args(
          "runs.conversation.question.number",
          [
            ("number", (index + 1).to_string().as_str()),
            ("count", count.to_string().as_str()),
          ],
        ))
        .variant(TypographyStyle::Mono)
        .color(PaletteColor::TextMuted)
        .nowrap(),
      )
      .with_children(question.header().map(rows::header_chip))
      .into()
  });
  let text = MarkdownText::message(
    format!("conversation-question-{}-{index}", state.request.id()),
    question.text(),
  )
  .mount(ctx);
  let takes = ctx.t(if question.multi_select() {
    "runs.conversation.question.pickSeveral"
  } else {
    "runs.conversation.question.pickOne"
  });
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Stack.role())
    .with_children(numbered)
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Hair.role())
        .child(text)
        .child(Text::new(&takes).variant(TypographyStyle::Caption)),
    )
    .child(rows::options(ctx, state, (index, question)))
    .into()
}

/// The keys' hint and *Answer*, which waits until every question has an answer; which question has none
/// yet, with several; and why the answer did not go.
fn footer(ctx: &Ctx, state: &CardState) -> Element {
  let draft = state.draft.get();
  let sending = state.action.state().get();
  let ready = draft.missing().is_none();
  let label = ctx.t(if sending.is_pending() || sending.data.is_some() {
    "runs.conversation.question.answering"
  } else {
    "runs.conversation.question.answer"
  });
  let submit = state.clone();
  let answer = ActionButton::new(label)
    .id("conversation-question-answer")
    .icon(Icon::CornerDownLeft)
    .primary()
    .compact()
    .on_click(move || submit.submit())
    .enabled(ready && !sending.is_pending() && sending.data.is_none())
    .render(ctx);
  let missing: Option<Element> = draft
    .missing()
    .filter(|_| state.request.questions().len() > 1)
    .map(|index| {
      Row::new()
        .spacing(ExtraSpacing::Tight.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Info, IconSize::Small, PaletteColor::TextMuted))
        .child(
          Text::new(&ctx.t_args(
            "runs.conversation.question.missing",
            [("number", (index + 1).to_string().as_str())],
          ))
          .variant(ExtraTypography::Hint.role())
          .nowrap(),
        )
        .into()
    });
  let line = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(Icon::Keyboard, IconSize::Small, PaletteColor::TextMuted))
    .child(
      Text::new(&keys::hint(ctx, state))
        .id("conversation-question-hint")
        .variant(ExtraTypography::Hint.role())
        .flex_shrink(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(Spacer::new().flex(1.0))
    .with_children(missing)
    .child(Row::new().flex_shrink(0.0).child(answer));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(line)
    .with_children(
      sending
        .error
        .as_ref()
        .filter(|_| !sending.is_pending())
        .map(|error| field_error(&message_text(ctx, error))),
    )
    .into()
}
