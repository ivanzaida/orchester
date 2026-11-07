//! One card (`f1J8z` in 05 `EIjDH`): `surface_raised` in a 1 `border_strong` stroke, radius 6, 12 inside and its parts
//! 10 apart, centred: a 13 `messages-square` in `warning` (2 down in its box), the title in 12.5/500 `text_primary`
//! and under it, 3 apart, since when and what taking it means in 11.5/1.5 `text_muted`; then *Open that conversation*
//! as a ghost with `external-link` and *Take it here* as the primary with `arrow-down-to-line`. Both are the person's:
//! lurq's synthetic input (an MCP client driving the window) is refused in the handler, visibly, as the other
//! person-only cards refuse it. *Take it here* moves the subscription here (`take_over_in`); *Open that conversation*
//! keeps the line the card settles into (`leave_there_in`) and opens it. A choice is carried out on the session's
//! runtime, so it is kept even when the card goes away first. Chosen, the card is a line in its place (`JZzui` in
//! `DE2n1`): a 13 icon in `text_muted`, what became of the target in 12.5 `text_secondary`, what it adds in 12
//! `text_muted`, and *Open ↗* to the other conversation.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::{ChatId, ProjectId},
  orchestrator::{Chat, watch::LedElsewhere},
};
use orchester_workspace::{
  background, display,
  messages::{self, Message},
  orchestrator::watch::{leave_there_in, take_over_in},
};

use super::{LedCard, Settled, target_name};
use crate::{
  mcp::qa_person::QaPerson,
  navigation::Navigation,
  routes::{OrchestratorChat, ProjectPage, Route},
  screens::orchestrator::line::{LineLink, link_element},
  services::Services,
  theme::{ExtraSpacing, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    pressable::Press,
    tinted_note::{person_only, tinted_note},
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct LedProps {
  pub(super) id: String,
  pub(super) project: ProjectId,
  /// The project's folder.
  pub(super) root: PathBuf,
  /// The conversation that shows the card.
  pub(super) chat: Chat,
  /// The order of the command that named the target.
  pub(super) order: String,
  pub(super) card: LedCard,
}

impl DevtoolsInspectable for LedProps {}

/// The person's choice on a card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
  Take,
  Leave,
}

/// A choice to carry out in conversation `chat` of the project at `root`.
#[derive(Clone)]
struct Choosing {
  root: PathBuf,
  chat: Chat,
  order: String,
  led: LedElsewhere,
  choice: Choice,
}

impl DevtoolsInspectable for Choosing {}

impl Choosing {
  /// Carries the choice out. Blocks on SQLite.
  fn apply(&self) -> Result<(), Message> {
    let applied = match self.choice {
      Choice::Take => take_over_in(&self.root, &self.chat, &self.led.target, &self.order).map(drop),
      Choice::Leave => leave_there_in(&self.root, &self.chat, &self.led),
    };
    applied.map_err(|error| error.message())
  }
}

type ChooseAction = FutureAction<Choosing, (), Message>;

/// Carries a choice out on the session's runtime and tells the runners the conversation changed, so it reads again.
fn choose_action(ctx: &mut Ctx) -> ChooseAction {
  let services = Services::of(ctx);
  ctx.future_action(move |choosing: Choosing| {
    let runners = services.runners.clone();
    let chosen = services.runtime.spawn(async move {
      let root = choosing.root.clone();
      background::run(move || choosing.apply()).await??;
      runners.chat_changed(&root);
      Ok::<(), Message>(())
    });
    async move { chosen.await.map_err(|_| messages::interrupted())? }
  })
}

pub(super) struct LedCardView {
  choose: ChooseAction,
  /// Synthetic input pressed one of the card's buttons.
  refused: Signal<bool>,
  /// Whether the buttons take synthetic input as the person's: in a QA copy only.
  qa_person: QaPerson,
}

impl Component for LedCardView {
  type Props = LedProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      choose: choose_action(ctx),
      refused: ctx.signal(false),
      qa_person: QaPerson::of(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<LedProps>().clone();
    match &props.card.settled {
      Some(settled) => settled_line(ctx, &props, settled),
      None => self.card(ctx, &props),
    }
  }
}

impl LedCardView {
  /// The card that waits for the person's choice, with why a press was refused or a choice failed under it.
  fn card(&self, ctx: &mut Ctx, props: &LedProps) -> Element {
    let id = props.id.as_str();
    let led = &props.card.led;
    let target = target_name(ctx, &led.target);
    let title = ctx.t_args(
      "orchestrator.watch.led.title",
      [("target", target.to_string()), ("title", led.title.clone())],
    );
    let since = (
      super::super::date(ctx, led.since).to_string(),
      display::clock(led.since),
    );
    let note = ctx.t_args("orchestrator.watch.led.note", [("date", since.0), ("time", since.1)]);
    let words = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Fine.role())
      .child(
        Text::new(&title)
          .id(format!("{id}-title"))
          .variant(OrchestratorText::Emphasis.role())
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .width(Dimension::Pct(100.0)),
      )
      .child(
        Text::new(&note)
          .id(format!("{id}-note"))
          .variant(OrchestratorText::CardNote.role())
          .width(Dimension::Pct(100.0)),
      );
    let glyph = Column::new()
      .flex_shrink(0.0)
      .padding(Padding::new().top(ExtraSpacing::Tight.role()))
      .child(icon(Icon::MessagesSquare, IconSize::Base, PaletteColor::Warning));
    let [open, take] = self.buttons(ctx, props);
    let card = Row::new()
      .id(id.to_owned())
      .describe("outcome", "led")
      .width(Dimension::Pct(100.0))
      .padding(SpacingSize::Md)
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
      .rounded(RadiusSize::Md)
      .child(glyph)
      .child(words)
      .child(open)
      .child(take);
    let refused = self
      .refused
      .get()
      .then(|| person_only(format!("{id}-person-only"), &ctx.t("orchestrator.watch.led.personOnly")));
    let failed = self.choose.state().get().error.map(|why| {
      tinted_note(
        format!("{id}-error"),
        (Icon::CircleAlert, PaletteColor::Danger),
        &message_text(ctx, &why),
      )
    });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(card)
      .with_children(refused)
      .with_children(failed)
      .into()
  }

  /// *Open that conversation* and *Take it here*; neither takes a press while a choice is carried out.
  fn buttons(&self, ctx: &mut Ctx, props: &LedProps) -> [ActionButton; 2] {
    let id = props.id.as_str();
    let open = ActionButton::new(
      format!("{id}-open"),
      ButtonKind::Ghost,
      ctx.t("orchestrator.watch.led.open"),
    )
    .icon(Icon::ExternalLink);
    let take = ActionButton::new(
      format!("{id}-take"),
      ButtonKind::Primary,
      ctx.t("orchestrator.watch.led.take"),
    )
    .icon(Icon::ArrowDownToLine);
    if self.choose.state().get().is_pending() {
      return [open.disabled(), take.loading()];
    }
    let choosing = |choice| Choosing {
      root: props.root.clone(),
      chat: props.chat.clone(),
      order: props.order.clone(),
      led: props.card.led.clone(),
      choice,
    };
    let (taking, leaving) = (choosing(Choice::Take), choosing(Choice::Leave));
    let navigation = Navigation::of(ctx);
    let there = conversation(props.project, props.card.led.chat);
    let (choose, refused) = (self.choose.clone(), (self.refused.clone(), self.qa_person.clone()));
    let open = open.on_click(move || {
      if refuses(&refused) {
        return;
      }
      choose.run(leaving.clone());
      navigation.go(there.clone());
    });
    let (choose, refused) = (self.choose.clone(), (self.refused.clone(), self.qa_person.clone()));
    let take = take.on_click(move || {
      if refuses(&refused) {
        return;
      }
      choose.run(taking.clone());
    });
    [open, take]
  }
}

/// Whether the press being delivered is lurq's synthetic input, which a card of the person's does not take outside a
/// QA copy (`mcp::qa_person`): then `refused` says so under the card.
fn refuses((refused, qa_person): &(Signal<bool>, QaPerson)) -> bool {
  if !qa_person.refuses_input() {
    return false;
  }
  tracing::warn!("synthetic input reached the card of a target another conversation leads; not taken");
  refused.set(true);
  true
}

/// Conversation `chat` of `project`.
const fn conversation(project: ProjectId, chat: ChatId) -> Route {
  Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)))
}

/// The line a chosen card settles into: the target taken here, or left where it is led; *Open ↗* opens that
/// conversation.
fn settled_line(ctx: &mut Ctx, props: &LedProps, settled: &Settled) -> Element {
  let id = props.id.as_str();
  let target = target_name(ctx, &props.card.led.target).to_string();
  let title = settled.title.clone();
  let (glyph, outcome, what, aside) = if settled.taken {
    (
      Icon::ArrowRightLeft,
      "taken",
      ctx.t_args("orchestrator.watch.led.taken", [("target", target)]),
      ctx.t_args("orchestrator.watch.led.takenFrom", [("title", title)]),
    )
  } else {
    (
      Icon::MessagesSquare,
      "left",
      ctx.t_args("orchestrator.watch.led.left", [("target", target), ("title", title)]),
      ctx.t("orchestrator.watch.led.leftAside"),
    )
  };
  let link = LineLink {
    label: ctx.t("orchestrator.line.open"),
    press: Press::Open(Navigation::of(ctx).to(conversation(props.project, settled.chat))),
  };
  Row::new()
    .id(id.to_owned())
    .describe("outcome", outcome)
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(SpacingSize::Xs))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Base, PaletteColor::TextMuted).flex_shrink(0.0))
    .child(
      Text::new(&what)
        .id(format!("{id}-words"))
        .variant(OrchestratorText::Line.role())
        .color(PaletteColor::TextSecondary)
        .nowrap()
        .flex_shrink(0.0),
    )
    .child(
      Text::new(&aside)
        .id(format!("{id}-aside"))
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(link_element(&format!("{id}-there"), link))
    .into()
}
