//! A start the Orchestrator's agent proposed (`orchester_workspace::orchestrator::proposal`): nothing ran, and it
//! waits for the person, in the action card's frame (`Action card` `nQIhx`): *Start <task> on <pipeline>?*, that
//! nothing started, and *Dismiss* and *Start*. Only a person starts it: lurq's synthetic input (an MCP client driving
//! the window) is refused in the handler, visibly, as the other person-only controls refuse it; *Dismiss* starts
//! nothing and stays open to automation.

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{orchestrator::proposal::ProposedStart, time::Timestamp};
use orchester_workspace::{
  display,
  orchestrator::{proposal::Decision, tools::ChatScope},
};

use super::card::{self, DecideAction, Deciding, State};
use crate::{
  theme::{ExtraSpacing, ExtraTypography, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct ProposalProps {
  pub(super) id: String,
  pub(super) scope: ChatScope,
  /// The proposal's entry in the conversation.
  pub(super) seq: i64,
  pub(super) task: String,
  /// What the person is asked to start: the task's title and brief's opening, the pipeline.
  pub(super) start: ProposedStart,
  /// A person started it and the start is being carried out.
  pub(super) applying: bool,
  pub(super) at: Timestamp,
}

impl DevtoolsInspectable for ProposalProps {}

pub(super) struct StartProposal {
  decide: DecideAction,
  /// Synthetic input tried *Start*.
  refused: Signal<bool>,
  /// The acceptance criteria are shown.
  criteria_open: Signal<bool>,
}

impl Component for StartProposal {
  type Props = ProposalProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      decide: card::decide_action(ctx),
      refused: ctx.signal(false),
      criteria_open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ProposalProps>().clone();
    let id = props.id.as_str();
    let state = self.decide.state().get();
    let title = ctx.t_args(
      "orchestrator.proposal.title",
      [
        ("task", props.task.as_str()),
        ("pipeline", props.start.pipeline.as_str()),
      ],
    );
    let state_now = if props.applying {
      applying(ctx, props.at)
    } else {
      waits(ctx, props.at)
    };
    let head = card::head(id, (Icon::Play, &title), None, state_now);
    let refused = self.refused.get().then(|| {
      card::note(
        id,
        "person-only",
        (Icon::Hand, PaletteColor::Warning),
        &ctx.t("orchestrator.proposal.personOnly"),
      )
    });
    let failed = state.error.as_ref().map(|error| {
      card::note(
        id,
        "error",
        (Icon::CircleAlert, PaletteColor::Danger),
        &message_text(ctx, error),
      )
    });
    card::frame(id, "proposed")
      .child(head)
      .child(asked(ctx, id, (&props.task, &props.start)))
      .with_children(self.criteria(ctx, id, &props.start.criteria))
      .with_children(super::action::open_task(
        ctx,
        id,
        props.scope.project,
        Some(&props.task),
      ))
      .child(
        Text::new(&ctx.t("orchestrator.proposal.effect"))
          .id(format!("{id}-effect"))
          .variant(TypographyStyle::Body)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(refused)
      .with_children(failed)
      .child(self.buttons(ctx, &props, state.is_pending() || props.applying))
  }
}

/// That the proposal made at `at` waits for the person.
pub(super) fn waits(ctx: &Ctx, at: Timestamp) -> State {
  State {
    glyph: Icon::Hourglass,
    color: PaletteColor::Warning,
    words: ctx.t_args("orchestrator.proposal.waits", [("time", display::clock(at).as_str())]),
  }
}

/// That a person's decision on the proposal made at `at` is being carried out.
pub(super) fn applying(ctx: &Ctx, at: Timestamp) -> State {
  State {
    glyph: Icon::LoaderCircle,
    color: PaletteColor::TextSecondary,
    words: ctx.t_args("orchestrator.change.applying", [("time", display::clock(at).as_str())]),
  }
}

impl StartProposal {
  /// *Start* and *Dismiss*, at the card's start as the action card's actions are; neither while a decision is being
  /// taken.
  fn buttons(&self, ctx: &Ctx, props: &ProposalProps, pending: bool) -> Row {
    let id = props.id.as_str();
    let deciding = |decision| Deciding {
      scope: props.scope.clone(),
      seq: props.seq,
      decision,
    };
    let dismiss = ActionButton::new(
      format!("{id}-dismiss"),
      ButtonKind::Secondary,
      ctx.t("orchestrator.proposal.dismiss"),
    );
    let start = ActionButton::new(
      format!("{id}-start"),
      ButtonKind::Primary,
      ctx.t("orchestrator.proposal.start"),
    )
    .icon(Icon::Play);
    let (dismiss, start) = if pending {
      (dismiss.disabled(), start.disabled())
    } else {
      let (decide, dismissing) = (self.decide.clone(), deciding(Decision::Dismiss));
      let dismiss = dismiss.on_click(move || decide.run(dismissing.clone()));
      let (decide, starting, refused) = (
        self.decide.clone(),
        deciding(Decision::Accept { widens: Vec::new() }),
        self.refused.clone(),
      );
      let start = start.on_click(move || {
        if is_synthetic_input() {
          tracing::warn!("synthetic input reached the start of a run the Orchestrator proposed; not taken");
          refused.set(true);
          return;
        }
        decide.run(starting.clone());
      });
      (dismiss, start)
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(start)
      .child(dismiss)
  }

  /// The task's acceptance criteria, behind *Show N acceptance criteria*; nothing when it has none.
  fn criteria(&self, ctx: &Ctx, id: &str, criteria: &[String]) -> Option<Column> {
    if criteria.is_empty() {
      return None;
    }
    let open = self.criteria_open.get();
    let count = criteria.len().to_string();
    let key = if open {
      "orchestrator.proposal.hideCriteria"
    } else {
      "orchestrator.proposal.showCriteria"
    };
    let toggle = self.criteria_open.clone();
    let button = ActionButton::new(
      format!("{id}-criteria-toggle"),
      ButtonKind::Ghost,
      ctx.t_args(key, [("count", count.as_str())]),
    )
    .icon(if open { Icon::ChevronUp } else { Icon::ChevronDown })
    .small()
    .on_click(move || toggle.update(|open| *open = !*open));
    let listed = open.then(|| {
      Column::new()
        .id(format!("{id}-criteria"))
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .with_children(criteria.iter().map(|criterion| {
          Text::new(&ctx.t_args("orchestrator.proposal.criterion", [("criterion", criterion.as_str())]))
            .variant(TypographyStyle::Description)
            .width(Dimension::Pct(100.0))
        }))
    });
    Some(
      Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Fine.role())
        .child(Row::new().child(button))
        .with_children(listed),
    )
  }
}

/// The task the person is asked to start: its title, the opening of its brief when it has one, and that it goes on
/// when it does.
fn asked(ctx: &Ctx, id: &str, (task, start): (&str, &ProposedStart)) -> Column {
  Column::new()
    .id(format!("{id}-task"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(
      Text::new(&start.title)
        .id(format!("{id}-task-title"))
        .variant(OrchestratorText::Emphasis.role())
        .width(Dimension::Pct(100.0)),
    )
    .with_children((!start.brief.is_empty()).then(|| {
      Text::new(&start.brief)
        .id(format!("{id}-brief"))
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0))
    }))
    .with_children(start.brief_cut.then(|| {
      Text::new(&ctx.t_args("orchestrator.proposal.briefCut", [("task", task)]))
        .id(format!("{id}-brief-cut"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Warning)
        .width(Dimension::Pct(100.0))
    }))
}
