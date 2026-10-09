//! An agent's question with suggested answers (`Question card` `tKYgO`, its options and own answer as the
//! Needs-you panel answers a question in place, `Z4HeE3` in `zwuZt`): *Builder asks you*, then each question
//! with the agent's options, as radios, or as checkboxes where several may be picked, and *Or write your own
//! answer…*; *Send answer* once every question has an answer, which resumes the agent at once. Once it no
//! longer waits it says what was answered, by whom and when, or that the agent stopped waiting first
//! ([`answered`]).

mod answered;

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
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
use parking_lot::Mutex;

use super::{
  Place,
  action::{Decide, decide, refusal},
  decisions::Decisions,
  frame::{Stands, frame, head, note},
};
use crate::{
  theme::{CardsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    field::FieldInput,
    icon::Icon,
    markdown::MarkdownView,
    toggle::{Toggle, ToggleKind},
  },
};

#[derive(Clone, PartialEq)]
struct ChoiceProps {
  place: Place,
  card: ChoiceCard,
}

impl lurq::app::component::DevtoolsInspectable for ChoiceProps {}

/// What the person picked on the card.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Picked(Draft);

impl lurq::app::component::DevtoolsInspectable for Picked {}

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
  picked: Signal<Picked>,
  /// Each question's own answer, as typed; made once the questions are known.
  own: Mutex<Vec<Signal<String>>>,
  /// The first option, which takes the focus when the tab opens on the card.
  first: ElementRef,
}

impl Component for ChoiceView {
  type Props = ChoiceProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      picked: ctx.signal(Picked::default()),
      own: Mutex::new(Vec::new()),
      first: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let ChoiceProps { place, card } = ctx.props::<ChoiceProps>().clone();
    let action = decide(ctx, place.project);
    let id = format!("cards-choice-{}", card.asked.request.id());
    if !matches!(card.asked.state, QuestionState::Waiting) {
      return answered::card(ctx, &id, (&place, &card));
    }
    let questions: Arc<[Question]> = card.asked.request.questions().into();
    self.seed(&questions);
    let draft = self.draft(&questions);
    let title = match questions.len() {
      1 => ctx.t_args("cards.question.asks", [("agent", &*card.agent)]),
      count => ctx.t_args(
        "cards.question.asksSeveral",
        [("agent", &*card.agent), ("count", count.to_string().as_str())],
      ),
    };
    let stands = Stands::waiting(ctx.t("cards.state.waitingAnswer")).into_element(&id);
    let mut body = frame(id.clone(), true)
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .child(head(
        &id,
        (Icon::MessageCircleQuestion, IconSize::Md, PaletteColor::Warning),
        (&title, CardsText::Title.role()),
        stands,
      ));
    for index in 0..questions.len() {
      body = body.child(self.question(ctx, (&id, &place, &card), (&questions, index), &draft));
    }
    self.focus_on_open(ctx, &card);
    body
      .child(self.footer(ctx, (&id, &place, &card), &draft, &action))
      .into()
  }
}

impl ChoiceView {
  /// One own answer per question, the first time the questions are known.
  fn seed(&self, questions: &[Question]) {
    let mut own = self.own.lock();
    if own.len() != questions.len() {
      *own = questions.iter().map(|_| Signal::new(String::new())).collect();
      self.picked.set(Picked(Draft::new(questions)));
    }
  }

  /// What is picked, with each own answer as typed.
  fn draft(&self, questions: &[Question]) -> Draft {
    let mut draft = self.picked.get().0;
    for (index, text) in self.own.lock().iter().enumerate() {
      let text = text.get();
      if !text.trim().is_empty() {
        draft.type_own(questions, index, &text);
      }
    }
    draft
  }

  /// The first option takes the focus when the tab opens on this card.
  fn focus_on_open(&self, ctx: &mut Ctx, card: &ChoiceCard) {
    if Decisions::of(ctx).is_some_and(|decisions| decisions.take_focus(card.asked.request.asked_at())) {
      ctx.focus(&self.first);
    }
  }

  /// Question `index`: its number among several and its header, what it asks, the agent's options and the
  /// person's own answer.
  fn question(
    &self,
    ctx: &mut Ctx,
    (id, place, card): (&str, &Place, &ChoiceCard),
    (questions, index): (&Arc<[Question]>, usize),
    draft: &Draft,
  ) -> Element {
    let question = &questions[index];
    let count = questions.len();
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
    let text = MarkdownView::new(format!("{id}-text-{index}"), question.text(), place.links.clone()).render(ctx);
    let label = ctx.t_args(
      if question.multi_select() {
        "cards.choice.optionsSeveral"
      } else {
        "cards.choice.options"
      },
      [("agent", &*card.agent)],
    );
    // One per question since `seed`.
    let own = self.own.lock()[index].clone();
    let field = FieldInput::new(format!("{id}-own-{index}"), ctx.t("cards.choice.ownName"), own)
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
          .child(self.options(id, (questions, index), draft)),
      )
      .child(field)
      .into()
  }

  /// The agent's options for question `index`: radios, or checkboxes where several may be picked; each
  /// option's description under it.
  fn options(&self, id: &str, (questions, index): (&Arc<[Question]>, usize), draft: &Draft) -> Column {
    let question = &questions[index];
    let kind = if question.multi_select() {
      ToggleKind::Checkbox
    } else {
      ToggleKind::Radio
    };
    let picks = draft.of(index).cloned().unwrap_or_default();
    let rows = question.options().iter().enumerate().map(|(option, shown)| {
      let (picked, own) = (self.picked.clone(), self.own.lock().get(index).cloned());
      let (all, multi) = (questions.clone(), question.multi_select());
      let mut toggle = Toggle::new(
        format!("{id}-option-{index}-{option}"),
        kind,
        picks.picked(option),
        shown.label(),
      )
      .on_change(move |_| {
        // A picked option of a single-choice question replaces the own answer: the agent takes one.
        if let (false, Some(own)) = (multi, &own) {
          own.set(String::new());
        }
        let all = all.clone();
        picked.update(move |picked| picked.0.pick(&all, (index, option)));
      });
      if (index, option) == (0, 0) {
        toggle = toggle.element_ref(self.first.clone());
      }
      Column::new()
        .spacing(ExtraSpacing::Tight.role())
        .child(toggle)
        // Under the option's words: past the 16 control and the 8 after it.
        .with_children(shown.description().map(|description| {
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

  /// When it was asked and what an answer does, why it cannot be sent yet, *Send answer*, and why an answer
  /// did not go through.
  fn footer(
    &self,
    ctx: &mut Ctx,
    (id, place, card): (&str, &Place, &ChoiceCard),
    draft: &Draft,
    action: &Decide,
  ) -> Element {
    let state = action.state().get();
    let questions = card.asked.request.questions();
    let missing = draft.missing();
    let time = display::clock(card.asked.request.asked_at());
    let context = match (&place.step, missing.filter(|_| questions.len() > 1)) {
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
    let button = ActionButton::new(format!("{id}-send"), ButtonKind::Primary, ctx.t("cards.choice.send"))
      .icon(Icon::Reply)
      .in_form();
    let button = if state.is_pending() {
      button.loading()
    } else if state.data.is_some() || missing.is_some() {
      button.disabled()
    } else {
      let (action, answering) = (
        action.clone(),
        Answering {
          run: card.asked.request.run(),
          request: card.asked.request.id().to_owned(),
          answers: draft.given(questions),
        },
      );
      button.on_click(move || action.run(answering.clone().command(Answerer::Person)))
    };
    let sent = state.data.is_some().then(|| {
      note(
        &format!("{id}-sent"),
        &ctx.t_args("cards.choice.sent", [("agent", &*card.agent)]),
      )
    });
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
      .with_children(sent)
      .with_children(
        state
          .error
          .as_ref()
          .filter(|_| !state.is_pending())
          .map(|refused| refusal(ctx, id, refused, &card.agent)),
      )
      .into()
  }
}
