//! A start the Orchestrator's agent proposed (`orchester_workspace::orchestrator::proposal`; `Start KON-31 on Feature
//! delivery v3?` in `3 · Decisions` `bxJZp`): nothing ran, and it waits for the person as a decision card
//! ([`super::decision`]): *Start <task> on <pipeline>?* with the task, the task's title and the opening of its brief,
//! its acceptance criteria behind *Show them*, and *Start*, *Dismiss* and *Open <task>*. Only a person starts it:
//! lurq's synthetic input (an MCP client driving the window) is refused in the handler, visibly, as the other
//! person-only controls refuse it; *Dismiss* starts nothing and stays open to automation. While a person's start is
//! carried out it is an event line with the loader.

use std::sync::Arc;

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

use super::{
  card::{self, DecideAction, Deciding},
  decision,
  line::{Chip, ChipTone, Line, Tone},
};
use crate::{
  navigation::Navigation,
  theme::{ExtraSpacing, ExtraTypography, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    message::message_text,
    tinted_note::{person_only, tinted_note},
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
    let time = display::clock(props.at);
    if props.applying {
      return Line::new(id, (Icon::LoaderCircle, Tone::Plain), title)
        .outcome("applying")
        .target(Some(props.task.as_str().into()))
        .chip(Some(Chip::new(ctx.t("orchestrator.line.applying"), ChipTone::Working)))
        .time(Some(time))
        .render(ctx);
    }
    let refused = self.refused.get().then(|| {
      person_only(
        format!("{id}-person-only"),
        &ctx.t("orchestrator.proposal.personOnly"),
      )
    });
    let failed = state.error.as_ref().map(|error| {
      tinted_note(
        format!("{id}-error"),
        (Icon::CircleAlert, PaletteColor::Danger),
        &message_text(ctx, error),
      )
    });
    let actions = self.actions(ctx, &props, state.is_pending());
    decision::frame(id, "proposed", false)
      .child(decision::head(ctx, id, (Icon::Play, &title), Some(&props.task), &time))
      .child(asked(ctx, id, (&props.task, &props.start)))
      .with_children(self.criteria(ctx, id, &props.start.criteria))
      .with_children(refused)
      .with_children(failed)
      .child(actions)
      .into()
  }
}

impl StartProposal {
  /// *Start*, *Dismiss* and *Open <task>*, with *Only you can apply*; neither takes a press while a decision is
  /// being taken.
  fn actions(&self, ctx: &mut Ctx, props: &ProposalProps, pending: bool) -> Row {
    let id = props.id.as_str();
    let deciding = |decision| Deciding {
      scope: props.scope.clone(),
      seq: props.seq,
      decision,
    };
    let dismiss = decision::dismiss_button(ctx, id);
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
    let mut buttons: Vec<Element> = vec![start.into(), dismiss.into()];
    if let Some(route) = super::action::task_route(props.scope.project, &props.task) {
      let label = ctx.t_args("orchestrator.action.openTask", [("task", props.task.as_str())]);
      let navigation = Navigation::of(ctx);
      buttons.push(
        decision::open_button(format!("{id}-open"), label)
          .opens(navigation.to(route))
          .into(),
      );
    }
    decision::actions(ctx, id, buttons)
  }

  /// The task's acceptance criteria as a fact, *N · Show them*, and the list once open; nothing when it has none.
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
    let fact = Row::new()
      .id(format!("{id}-fact-criteria"))
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(
        Text::new(&ctx.t("orchestrator.proposal.criteria"))
          .variant(OrchestratorText::FactKey.role())
          .width(Dimension::Px(decision::FACT_KEY_WIDTH))
          .flex_shrink(0.0),
      )
      .child(button);
    Some(
      Column::new()
        .id(format!("{id}-facts"))
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xs)
        .child(fact)
        .with_children(listed),
    )
  }
}

/// What the person is asked to start: the task's title and the opening of its brief, and that the brief goes on when
/// it does.
fn asked(ctx: &Ctx, id: &str, (task, start): (&str, &ProposedStart)) -> Column {
  let about: Arc<str> = if start.brief.is_empty() {
    start.title.as_str().into()
  } else {
    ctx.t_args(
      "orchestrator.proposal.about",
      [("title", start.title.as_str()), ("brief", start.brief.as_str())],
    )
  };
  Column::new()
    .id(format!("{id}-task"))
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(decision::body(id, &about))
    .with_children(start.brief_cut.then(|| {
      Text::new(&ctx.t_args("orchestrator.proposal.briefCut", [("task", task)]))
        .id(format!("{id}-brief-cut"))
        .variant(ExtraTypography::Note.role())
        .color(PaletteColor::Warning)
        .width(Dimension::Pct(100.0))
    }))
}
