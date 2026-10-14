//! A start the Orchestrator's agent proposed (`orchester_workspace::orchestrator::proposal`): nothing ran, and it
//! waits for the person, in the action card's frame (`Action card` `nQIhx`): *Start <task> on <pipeline>?*, that
//! nothing started, and *Dismiss* and *Start*. Only a person starts it: lurq's synthetic input (an MCP client driving
//! the window) is refused in the handler, visibly, as the other person-only controls refuse it; *Dismiss* starts
//! nothing and stays open to automation.

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
use orchester_core::time::Timestamp;
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
  theme::{ConversationText, ExtraSpacing, ExtraTypography, IconSize},
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
  pub(super) task: String,
  pub(super) pipeline: String,
  pub(super) at: Timestamp,
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

pub(super) struct StartProposal {
  decide: DecideAction,
  /// Synthetic input tried *Start*.
  refused: Signal<bool>,
}

impl Component for StartProposal {
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
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ProposalProps>().clone();
    let id = props.id.as_str();
    let state = self.decide.state().get();
    let args = [("task", props.task.as_str()), ("pipeline", props.pipeline.as_str())];
    let head = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(icon(Icon::Play, IconSize::Md, PaletteColor::TextSecondary))
      .child(
        Text::new(&ctx.t_args("orchestrator.proposal.title", args))
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
            Text::new(&ctx.t_args(
              "orchestrator.proposal.waits",
              [("time", display::clock(props.at).as_str())],
            ))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
            .nowrap(),
          ),
      );
    let note = |text: &str, suffix: &str, color: PaletteColor| {
      Text::new(text)
        .id(format!("{id}-{suffix}"))
        .variant(ExtraTypography::Note.role())
        .color(color)
        .width(Dimension::Pct(100.0))
    };
    let refused = self.refused.get().then(|| {
      note(
        &ctx.t("orchestrator.proposal.personOnly"),
        "person-only",
        PaletteColor::Warning,
      )
    });
    let failed = state
      .error
      .as_ref()
      .map(|error| note(&message_text(ctx, error), "error", PaletteColor::Danger));
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
      .child(
        Text::new(&ctx.t("orchestrator.proposal.effect"))
          .id(format!("{id}-effect"))
          .variant(TypographyStyle::Body)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(refused)
      .with_children(failed)
      .child(self.buttons(ctx, &props, state.is_pending()))
  }
}

impl StartProposal {
  /// *Dismiss* and *Start*, at the card's end; neither while a decision is being taken.
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
      let (decide, starting, refused) = (self.decide.clone(), deciding(Decision::Start), self.refused.clone());
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
      .child(Spacer::new().flex(1.0))
      .child(dismiss)
      .child(start)
  }
}
