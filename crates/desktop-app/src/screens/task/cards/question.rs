//! An agent's question that its step waits on (`Question card` `tKYgO`): *Builder asks you*, where it stands,
//! the question as the agent wrote it, when and in which step it was asked, and while it waits *Answer*, which
//! puts the focus in the composer, where the answer is written (C2). Answered, it goes quiet: the person's
//! answer follows it in the conversation (`I6cZ5j`).

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize},
  },
  components::Row,
  core::ElementRef,
  layout::Alignment,
  node::{Element, padding::Padding},
};
use orchester_workspace::{
  display,
  tasks::conversation::{Author, Entry, EntryState},
};

use super::{
  ComposerState, Place,
  decisions::Decisions,
  frame::{Stands, frame, head, note},
};
use crate::{
  theme::{CardsText, ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    markdown::MarkdownView,
  },
};

#[derive(Clone, PartialEq)]
struct QuestionProps {
  place: Place,
  entry: Entry,
}

impl lurq::app::component::DevtoolsInspectable for QuestionProps {}

pub(super) fn card(ctx: &mut Ctx, place: Place, entry: &Entry) -> Element {
  ctx.mount_keyed::<QuestionCard>(
    &format!("cards-question-{}", entry.id),
    QuestionProps {
      place,
      entry: entry.clone(),
    },
  )
}

struct QuestionCard {
  answer: ElementRef,
}

impl Component for QuestionCard {
  type Props = QuestionProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      answer: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let QuestionProps { place, entry } = ctx.props::<QuestionProps>().clone();
    let id = format!("cards-question-{}", entry.id);
    let decisions = Decisions::of(ctx);
    let focus = decisions.take_focus(&place.key);
    let asked = entry.state == EntryState::Question { open: true };
    let lost = asked && decisions.lost();
    let open = asked && !lost;
    let agent = match &entry.author {
      Author::Agent(agent) | Author::YouTo(agent) => agent.clone(),
      Author::You => "".into(),
    };
    let title = ctx.t_args("cards.question.asks", [("agent", &*agent)]);
    let (glyph, stands) = if open {
      (
        PaletteColor::Warning,
        Stands::waiting(ctx.t("cards.state.waitingAnswer")),
      )
    } else if lost {
      (
        PaletteColor::TextSecondary,
        Stands::interrupted(ctx.t("cards.state.interrupted")),
      )
    } else {
      (
        PaletteColor::TextSecondary,
        Stands {
          glyph: Icon::CircleCheck,
          words: ctx.t("cards.state.answered"),
          color: PaletteColor::Success,
        },
      )
    };
    let text = MarkdownView::new(format!("{id}-text"), entry.body.clone(), place.links.clone()).render(ctx);
    let context = context(ctx, &place, &entry, (&agent, open, lost));
    let composer = Place::composer(ctx);
    let actions = open.then(|| self.actions(ctx, &id, composer, focus)).flatten();
    frame(id.clone(), open)
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .child(head(
        &id,
        (Icon::MessageCircleQuestion, IconSize::Md, glyph),
        (&title, CardsText::Title.role()),
        stands.into_element(&id),
      ))
      .child(text)
      .child(note(&format!("{id}-context"), &context))
      .with_children(actions)
  }
}

impl QuestionCard {
  /// *Answer*, which sends the person to the composer; unavailable, and why, while the composer takes no
  /// answer here.
  /// It takes the focus with `focus`, when it can be pressed.
  fn actions(&self, ctx: &mut Ctx, id: &str, composer: Option<ComposerState>, focus: bool) -> Option<Element> {
    let composer = composer?;
    let button = ActionButton::new(
      format!("{id}-answer"),
      ButtonKind::Primary,
      ctx.t("cards.question.answer"),
    )
    .icon(Icon::Reply)
    .in_form();
    if focus {
      ctx.focus(&self.answer);
    }
    Some(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          button
            .element_ref(self.answer.clone())
            .on_click(move || composer.take_up()),
        )
        .into(),
    )
  }
}

/// When and in which step it was asked, and what waiting does, that the run was interrupted, or that the
/// answer follows.
fn context(ctx: &Ctx, place: &Place, entry: &Entry, (agent, open, lost): (&str, bool, bool)) -> std::sync::Arc<str> {
  let time = display::clock(entry.at);
  let key = match (open || lost, place.step.is_some()) {
    _ if lost && place.step.is_some() => "cards.question.lost",
    _ if lost => "cards.question.lostNoStep",
    (true, true) => "cards.question.waiting",
    (true, false) => "cards.question.waitingNoStep",
    (false, true) => "cards.question.answered",
    (false, false) => "cards.question.answeredNoStep",
  };
  let step = place.step.as_deref().unwrap_or_default();
  ctx.t_args(key, [("time", time.as_str()), ("step", step), ("agent", agent)])
}
