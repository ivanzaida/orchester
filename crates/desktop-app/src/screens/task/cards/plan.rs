//! A plan of a gated step (`Plan card` `Q5J0r`), one card per version: *Plan v1 · <task>* and where it stands
//! in a head over a `border` line (11 over and under it, 14 at its sides), the plan's first lines and *Show
//! the whole plan · 24 lines* (12 by 14, 6 apart), and while it waits a footer over a `border` line with
//! *Approve plan & continue* and what approving starts. The changes a plan needs are written in the composer
//! (C2), which asks for them. A decided version goes quiet and says who decided; one that changes were asked
//! for says what was asked.

use std::sync::Arc;

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::{Answerer, PlanDecision};
use orchester_ipc::Command;
use orchester_workspace::tasks::timeline::PlanCard;

use super::{
  ComposerState, Place,
  decisions::{Decisions, Given, Outcome, Refused, Stage, refusal},
  frame::{Stands, frame, head, held, note},
};
use crate::{
  theme::{CardsSpacing, CardsText, ExtraColor, ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    markdown::MarkdownView,
    pressable::pressable,
  },
};

/// Lines of a plan shown before *Show the whole plan* (`vGWCj`: a heading and four steps).
const PLAN_LINES: usize = 6;

#[derive(Clone, PartialEq)]
struct PlanProps {
  place: Place,
  card: PlanCard,
}

impl lurq::app::component::DevtoolsInspectable for PlanProps {}

pub(super) fn card(ctx: &mut Ctx, place: Place, card: &PlanCard) -> Element {
  ctx.mount_keyed::<PlanView>(
    &format!("cards-plan-{}-{}", card.plan.execution, card.plan.version),
    PlanProps {
      place,
      card: card.clone(),
    },
  )
}

struct PlanView {
  whole: Signal<bool>,
  approve: ElementRef,
}

impl Component for PlanView {
  type Props = PlanProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      whole: ctx.signal(false),
      approve: ElementRef::new(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let PlanProps { place, card } = ctx.props::<PlanProps>().clone();
    let decisions = Decisions::of(ctx);
    let id = format!("cards-plan-{}-{}", card.plan.execution, card.plan.version);
    let outcome = decisions.outcome((place.run(), &place.key));
    let lost = card.waiting && decisions.lost();
    // Approved here, or refused as no longer waiting, before the run's change is read again.
    let settled = outcome.as_ref().is_some_and(|outcome| {
      matches!(
        outcome.stage,
        Stage::Taken(_) | Stage::Refused(Refused::Ended | Refused::Superseded)
      )
    });
    let waiting = card.waiting && !lost && !settled;
    let version = card.plan.version.to_string();
    let title = ctx.t_args(
      "cards.plan.title",
      [
        ("version", version.as_str()),
        ("task", place.detail.task.details().title.as_str()),
      ],
    );
    let head = head(
      &id,
      (Icon::Map, IconSize::Md, PaletteColor::TextSecondary),
      (&title, CardsText::Title.role()),
      stands(ctx, &card, (waiting, lost, outcome.as_ref())).into_element(&id),
    )
    .padding(Padding::symmetric(
      ExtraSpacing::Inset.role(),
      CardsSpacing::PlanHead.role(),
    ))
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border));
    let focus = decisions.take_focus(&place.key);
    let footer = waiting.then(|| self.footer(ctx, (&id, &place, &card), (&decisions, focus)));
    // Why a decision given here did not count, once the plan no longer waits for one.
    let refused = match outcome.map(|outcome| outcome.stage) {
      Some(Stage::Refused(refused @ (Refused::Ended | Refused::Superseded))) if !waiting => Some(refused),
      _ => None,
    };
    frame(id.clone(), waiting)
      .child(head)
      .child(self.body(ctx, &id, (&place, &card)))
      .with_children(footer)
      .with_children(refused.map(|refused| {
        Column::new()
          .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
          .child(refusal(ctx, &id, &refused, &card.agent))
      }))
  }
}

impl PlanView {
  /// The plan's first lines, or all of them, *Show the whole plan* while it is cut, and what changes were
  /// asked for.
  fn body(&self, ctx: &mut Ctx, id: &str, (place, card): (&Place, &PlanCard)) -> Element {
    let text = card.plan.text.trim();
    let lines: Vec<&str> = text.lines().collect();
    let whole = self.whole.get() || lines.len() <= PLAN_LINES;
    let shown: Arc<str> = match (text.is_empty(), whole) {
      (true, _) => ctx.t("cards.plan.empty"),
      (false, true) => text.into(),
      (false, false) => lines[..PLAN_LINES].join("\n").into(),
    };
    let plan = MarkdownView::new(format!("{id}-lines"), shown, place.links.clone())
      .style(CardsText::PlanLines.role(), PaletteColor::TextSecondary)
      .render(ctx);
    let more = (!whole).then(|| {
      let open = self.whole.clone();
      Element::from(
        pressable(move || open.set(true)).id(format!("{id}-whole")).child(
          lurq::components::Text::new(&ctx.t_args("cards.plan.whole", [("count", lines.len().to_string().as_str())]))
            .variant(TypographyStyle::Button)
            .color(ExtraColor::AccentText.role())
            .nowrap(),
        ),
      )
    });
    let asked = match (&card.plan.review, &card.plan.decision()) {
      (Some(review), Some(PlanDecision::ChangesRequested { note: asked })) => {
        let key = match review.by {
          Answerer::Person => "cards.plan.youAsked",
          Answerer::Automation => "cards.plan.automationAsked",
        };
        Some(note(
          &format!("{id}-asked"),
          &ctx.t_args(key, [("note", asked.as_str())]),
        ))
      }
      _ => None,
    };
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Snug.role())
      .child(plan)
      .with_children(more)
      .with_children(asked)
      .into()
  }

  /// *Approve plan & continue* and what approving starts; why it did not go through. It takes the focus with
  /// `focus`.
  fn footer(
    &self,
    ctx: &mut Ctx,
    (id, place, card): (&str, &Place, &PlanCard),
    (decisions, focus): (&Decisions, bool),
  ) -> Element {
    let outcome = decisions.outcome((place.run(), &place.key));
    let button = ActionButton::new(
      format!("{id}-approve"),
      ButtonKind::Primary,
      ctx.t("cards.plan.approve"),
    )
    .icon(Icon::Check)
    .in_form()
    .element_ref(self.approve.clone());
    let sending = outcome.as_ref().is_some_and(|outcome| outcome.stage == Stage::Sending);
    let button = match place.open(ctx) {
      _ if sending => button.loading(),
      _ if decisions.busy() => button.disabled(),
      None => button.disabled(),
      Some(open) => {
        let (decisions, run, key) = (decisions.clone(), place.run(), place.key.clone());
        let command = Command::DecidePlan {
          run: card.plan.run,
          execution: card.plan.execution,
          version: card.plan.version,
          decision: PlanDecision::Approved,
          by: Answerer::Person,
        };
        if focus {
          ctx.focus(&self.approve);
        }
        button.on_click(move || decisions.answer((run, &key), Given::ApprovePlan, (&open, command.clone())))
      }
    };
    let composer = Place::composer(ctx);
    let next = card
      .next
      .as_ref()
      .map(|next| (place.label_of(ctx, &next.step), next.agent.clone()));
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(Row::new().flex_shrink(0.0).child(button))
      .child(hint_line(ctx, id, (next, composer)));
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(
        ExtraSpacing::Inset.role(),
        ExtraSpacing::Loose.role(),
      ))
      .spacing(SpacingSize::Sm)
      .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(row)
      .with_children((decisions.busy() && !sending).then(|| held(ctx, id)))
      .with_children(outcome.and_then(|outcome| match outcome.stage {
        Stage::Refused(refused) => Some(refusal(ctx, id, &refused, &card.agent)),
        Stage::Sending | Stage::Taken(_) => None,
      }))
      .into()
  }
}

/// Where a version stands: it waits, its run was interrupted, it was decided (by automation, when it was;
/// here, before the run is read again), or a later version came.
fn stands(ctx: &Ctx, card: &PlanCard, (waiting, lost, outcome): (bool, bool, Option<&Outcome>)) -> Stands {
  let decided = |glyph, key: &str, color| Stands {
    glyph,
    words: ctx.t(key),
    color,
  };
  match (&card.plan.review, waiting) {
    (_, true) => Stands::waiting(ctx.t("cards.state.waitingApproval")),
    (None, false) if lost => Stands::interrupted(ctx.t("cards.state.interrupted")),
    (None, false) if outcome.is_some_and(|outcome| matches!(outcome.stage, Stage::Taken(_))) => {
      decided(Icon::CircleCheck, "cards.state.approved", PaletteColor::Success)
    }
    (None, false) => decided(Icon::History, "cards.state.earlier", PaletteColor::TextMuted),
    (Some(review), false) => match (&review.decision, review.by) {
      (PlanDecision::Approved, Answerer::Person) => {
        decided(Icon::CircleCheck, "cards.state.approved", PaletteColor::Success)
      }
      (PlanDecision::Approved, Answerer::Automation) => decided(
        Icon::CircleCheck,
        "cards.state.approvedByAutomation",
        PaletteColor::Success,
      ),
      (PlanDecision::ChangesRequested { .. }, Answerer::Person) => {
        decided(Icon::Undo2, "cards.state.changesRequested", PaletteColor::TextSecondary)
      }
      (PlanDecision::ChangesRequested { .. }, Answerer::Automation) => decided(
        Icon::Undo2,
        "cards.state.changesRequestedByAutomation",
        PaletteColor::TextSecondary,
      ),
    },
  }
}

/// The step that approving starts, as the conversation names it ("03 Implement"), and its agent unless a tool
/// runs it.
type Next = (Arc<str>, Option<Arc<str>>);

/// What approving starts, and that the changes a plan needs are written in the composer: a press on it takes
/// the person there, in the plan's mode. Without a composer, what approving starts only.
fn hint_line(ctx: &Ctx, id: &str, (next, composer): (Option<Next>, Option<ComposerState>)) -> Element {
  let words = note(&format!("{id}-hint"), &hint(ctx, next, composer.is_some()))
    .flex(1.0)
    .min_width(Dimension::Px(0.0));
  match composer {
    Some(composer) => pressable(move || composer.take_up())
      .id(format!("{id}-changes"))
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .child(words)
      .into(),
    None => words.into(),
  }
}

/// `next` is the step that starts, as the conversation names it ("03 Implement"), and its agent.
fn hint(ctx: &Ctx, next: Option<Next>, composer: bool) -> Arc<str> {
  let key = |with: &'static str, without: &'static str| if composer { with } else { without };
  match next {
    Some((step, Some(agent))) => ctx.t_args(
      key("cards.plan.hint.agent", "cards.plan.hint.agentOnly"),
      [("step", &*step), ("agent", &*agent)],
    ),
    Some((step, None)) => ctx.t_args(
      key("cards.plan.hint.tool", "cards.plan.hint.toolOnly"),
      [("step", &*step)],
    ),
    None => ctx.t(key("cards.plan.hint.last", "cards.plan.hint.lastOnly")),
  }
}
