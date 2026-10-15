//! A start or a change the Orchestrator's agent proposed (`orchester_workspace::orchestrator::proposal`): nothing ran
//! or changed, and it waits for the person, in the action card's frame (`Action card` `nQIhx`): *Start <task> on
//! <pipeline>?*, *Publish <name> as v<n>?*, *Cancel* or *Merge run #<n> of <task>?*, that nothing happened yet, and
//! the person's action and *Dismiss*. A change has no frame of its own yet: it is the start's card with its own words.
//! Only a person starts or applies it: lurq's synthetic input (an MCP client driving the window) is refused in the
//! handler, visibly, as the other person-only controls refuse it; *Dismiss* changes nothing and stays open to
//! automation.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  orchestrator::{
    ActionOutcome,
    change::{Change, ProposedChange, Publishing},
    proposal::ProposedStart,
  },
  time::Timestamp,
};
use orchester_workspace::{
  display,
  messages::Message,
  orchestrator::{
    proposal::{self, Decision},
    tools::ChatScope,
  },
};

use crate::{
  mcp::domain,
  services::Services,
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct ProposalProps {
  pub(super) id: String,
  pub(super) scope: ChatScope,
  /// The proposal's entry in the conversation.
  pub(super) seq: i64,
  /// The key of the task it names; empty when it names none.
  pub(super) task: String,
  pub(super) asked: Asked,
  pub(super) at: Timestamp,
}

/// What the person is asked.
#[derive(Clone, PartialEq)]
pub(super) enum Asked {
  /// To start a task: its title and brief's opening, the pipeline.
  Start(ProposedStart),
  /// To publish a draft, cancel or merge a run.
  Change(ProposedChange),
}

impl Asked {
  /// What a proposal waiting with `outcome` asks; `None` for an action that waits for nobody.
  pub(super) fn of(outcome: &ActionOutcome) -> Option<Self> {
    match outcome {
      ActionOutcome::Proposed(start) => Some(Self::Start((**start).clone())),
      ActionOutcome::ProposedChange(change) => Some(Self::Change((**change).clone())),
      ActionOutcome::Done | ActionOutcome::Refused { .. } | ActionOutcome::Dismissed => None,
    }
  }
}

impl DevtoolsInspectable for ProposalProps {}

/// A decision to take on a proposal.
#[derive(Clone)]
struct Deciding {
  scope: ChatScope,
  seq: i64,
  decision: Decision,
}

impl DevtoolsInspectable for Deciding {}

type DecideAction = FutureAction<Deciding, (), Message>;

pub(super) struct ProposalCard {
  decide: DecideAction,
  /// Synthetic input tried the person's action.
  refused: Signal<bool>,
  /// The acceptance criteria are shown.
  criteria_open: Signal<bool>,
}

/// What a card says and offers.
struct Asking {
  glyph: Icon,
  title: Arc<str>,
  effect: Arc<str>,
  person_only: Arc<str>,
  /// The person's action: its label and icon.
  accept: (Arc<str>, Icon),
}

impl Component for ProposalCard {
  type Props = ProposalProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let i18n = ctx.i18n().clone();
    let decide = ctx.future_action(move |deciding: Deciding| {
      let (services, i18n) = (services.clone(), i18n.clone());
      async move {
        let tools = domain::on(&services);
        let translate = domain::translate(i18n);
        proposal::decide(
          (&tools, &translate, &services.runners),
          &deciding.scope,
          deciding.seq,
          deciding.decision,
        )
        .await
      }
    });
    Self {
      decide,
      refused: ctx.signal(false),
      criteria_open: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ProposalProps>().clone();
    let id = props.id.as_str();
    let state = self.decide.state().get();
    let asking = asking(ctx, &props.asked, &props.task);
    let head = head(ctx, id, (asking.glyph, &asking.title), props.at);
    let note = |text: &str, suffix: &str, color: PaletteColor| {
      Text::new(text)
        .id(format!("{id}-{suffix}"))
        .variant(ExtraTypography::Note.role())
        .color(color)
        .width(Dimension::Pct(100.0))
    };
    let refused = self
      .refused
      .get()
      .then(|| note(&asking.person_only, "person-only", PaletteColor::Warning));
    let failed = state
      .error
      .as_ref()
      .map(|error| note(&message_text(ctx, error), "error", PaletteColor::Danger));
    let (asked, criteria) = match &props.asked {
      Asked::Start(start) => (
        Some(asked(ctx, id, (&props.task, start))),
        self.criteria(ctx, id, &start.criteria),
      ),
      Asked::Change(change) => (subject(id, change), None),
    };
    Column::new()
      .id(id.to_owned())
      .describe("outcome", "proposed")
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Section.role(), SpacingSize::Md))
      .spacing(ExtraSpacing::Loose.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Lg)
      .child(head)
      .with_children(asked)
      .with_children(criteria)
      .with_children(super::action::open_task(
        ctx,
        id,
        props.scope.project,
        Some(&props.task),
      ))
      .child(
        Text::new(&asking.effect)
          .id(format!("{id}-effect"))
          .variant(TypographyStyle::Body)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(refused)
      .with_children(failed)
      .child(self.buttons(ctx, &props, asking.accept, state.is_pending()))
  }
}

impl ProposalCard {
  /// The person's action (*Start*, *Publish*, *Cancel run*, *Merge*) and *Dismiss*, at the card's start as the action
  /// card's actions are; neither while a decision is being taken.
  fn buttons(&self, ctx: &Ctx, props: &ProposalProps, (label, glyph): (Arc<str>, Icon), pending: bool) -> Row {
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
    let accept = ActionButton::new(format!("{id}-start"), ButtonKind::Primary, label).icon(glyph);
    let (dismiss, accept) = if pending {
      (dismiss.disabled(), accept.disabled())
    } else {
      let (decide, dismissing) = (self.decide.clone(), deciding(Decision::Dismiss));
      let dismiss = dismiss.on_click(move || decide.run(dismissing.clone()));
      let (decide, accepting, refused) = (self.decide.clone(), deciding(Decision::Accept), self.refused.clone());
      let accept = accept.on_click(move || {
        if is_synthetic_input() {
          tracing::warn!("synthetic input reached a proposal of the Orchestrator; not taken");
          refused.set(true);
          return;
        }
        decide.run(accepting.clone());
      });
      (dismiss, accept)
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(accept)
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

/// The card's icon and title, and that it waits for the person since `at`.
fn head(ctx: &Ctx, id: &str, (glyph, title): (Icon, &str), at: Timestamp) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(ConversationText::ResultTitle.role())
        .flex_shrink(1.0),
    )
    .child(Spacer::new().flex(1.0))
    .child(
      Row::new()
        .id(format!("{id}-state"))
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::Hourglass, IconSize::Sm, PaletteColor::Warning))
        .child(
          Text::new(&ctx.t_args("orchestrator.proposal.waits", [("time", display::clock(at).as_str())]))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
        ),
    )
}

/// What the card of `asked`, naming task `task`, says and offers.
fn asking(ctx: &Ctx, asked: &Asked, task: &str) -> Asking {
  match asked {
    Asked::Start(start) => Asking {
      glyph: Icon::Play,
      title: ctx.t_args(
        "orchestrator.proposal.title",
        [("task", task), ("pipeline", start.pipeline.as_str())],
      ),
      effect: ctx.t("orchestrator.proposal.effect"),
      person_only: ctx.t("orchestrator.proposal.personOnly"),
      accept: (ctx.t("orchestrator.proposal.start"), Icon::Play),
    },
    Asked::Change(change) => change_asking(ctx, change, task),
  }
}

fn change_asking(ctx: &Ctx, change: &ProposedChange, task: &str) -> Asking {
  let (effect, accept) = match change.change {
    Change::Publish { .. } => (
      "orchestrator.proposal.publish.effect",
      ("orchestrator.proposal.publish.accept", Icon::Upload),
    ),
    Change::Cancel { .. } => (
      "orchestrator.proposal.cancel.effect",
      ("orchestrator.proposal.cancel.accept", Icon::CircleStop),
    ),
    Change::Merge { .. } => (
      "orchestrator.proposal.merge.effect",
      ("orchestrator.proposal.merge.accept", Icon::GitMerge),
    ),
  };
  Asking {
    glyph: change_glyph(change),
    title: change_title(ctx, change, task),
    effect: ctx.t(effect),
    person_only: ctx.t("orchestrator.proposal.changePersonOnly"),
    accept: (ctx.t(accept.0), accept.1),
  }
}

/// The icon of a proposed change.
pub(super) const fn change_glyph(change: &ProposedChange) -> Icon {
  match change.change {
    Change::Publish { .. } => Icon::Upload,
    Change::Cancel { .. } => Icon::CircleStop,
    Change::Merge { .. } => Icon::GitMerge,
  }
}

/// What a proposed change asks, naming task `task`: *Publish pipeline <name> as v<n>?*, *Cancel run #<n> of
/// <task>?*, *Merge run #<n> of <task>?*.
pub(super) fn change_title(ctx: &Ctx, change: &ProposedChange, task: &str) -> Arc<str> {
  match &change.change {
    Change::Publish { of, version, .. } => {
      let key = match of {
        Publishing::Pipeline => "orchestrator.proposal.publish.pipeline",
        Publishing::Profile => "orchestrator.proposal.publish.profile",
        Publishing::Instruction => "orchestrator.proposal.publish.instruction",
        Publishing::Skill => "orchestrator.proposal.publish.skill",
        Publishing::Tool => "orchestrator.proposal.publish.tool",
      };
      let version = version.to_string();
      ctx.t_args(key, [("name", change.subject.as_str()), ("version", version.as_str())])
    }
    Change::Cancel { number, .. } => ctx.t_args(
      "orchestrator.proposal.cancel.title",
      [("run", number.to_string().as_str()), ("task", task)],
    ),
    Change::Merge { number, .. } => ctx.t_args(
      "orchestrator.proposal.merge.title",
      [("run", number.to_string().as_str()), ("task", task)],
    ),
  }
}

/// The title of the task whose run a change cancels or merges; nothing for a publication, whose name is its title.
fn subject(id: &str, change: &ProposedChange) -> Option<Column> {
  let names_task = matches!(change.change, Change::Cancel { .. } | Change::Merge { .. });
  (names_task && !change.subject.is_empty()).then(|| {
    Column::new()
      .id(format!("{id}-task"))
      .width(Dimension::Pct(100.0))
      .child(
        Text::new(&change.subject)
          .id(format!("{id}-task-title"))
          .variant(OrchestratorText::Emphasis.role())
          .width(Dimension::Pct(100.0)),
      )
  })
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
