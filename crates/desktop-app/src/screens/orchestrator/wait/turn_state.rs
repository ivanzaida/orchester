//! Where the turn of an event stands, under the event's row, where its answer would be (`Parts · An event whose turn
//! failed or is held` `tkjdw` a–f on the board `Orchestrator autonomy · frames to approve`;
//! `docs/plan/orchestrator-autonomy/OA-02-retry-clock.md`, `OA-03-…`), read with the conversation
//! (`orchester_workspace::orchestrator::event_turns`): one row, 27 in at its start and 4 at its end, its parts 10
//! apart, a 13 icon, its words in 12.5 `text_secondary` and why in 12 `text_muted`.
//!
//! - *Couldn't answer — trying again at 14:05 · 2 of 4 — the answer took longer than 10 minutes*, a `rotate-cw` in
//!   `text_muted`.
//! - *Left to you after 4 tries*, a `circle-alert` in `warning`, with why and **Try again**, which starts one more turn
//!   for that event now. Only a person asks for it: lurq's synthetic input is refused to the client that sent it, as
//!   the screen's other person-only actions refuse it.
//! - *Held until 00:00 — today's budget is used*, *Held until 16:40 — the account's plan limit is reached*, or, for a
//!   limit whose account did not say when it lifts, *Held — the account's plan limit is reached · trying again at
//!   15:00*, each a `pause` in `text_muted`.
//!
//! The try that thinks now is the working line's to say (`super::super::ending`): *Orchestrator is thinking · 2 of 4*.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text, TextOverflow},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  connection::AccountStartProblem,
  id::ChatId,
  orchestrator::{Held, TurnFailure},
  time::Timestamp,
};
use orchester_workspace::{
  display,
  messages::Message,
  orchestrator::event_turns::{EventTurn, Hold},
  workspace::OpenProject,
};

use crate::{
  mcp::qa_person::QaPerson,
  project::try_again,
  screens::orchestrator::timeline::TurnRow,
  services::Services,
  theme::{ExtraSpacing, IconSize, OrchestratorSpacing, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
  },
};

/// A day is at most 25 hours long: the start of a day no later than that after today's start is tomorrow's.
const DAY_AT_MOST: i64 = 25 * 60 * 60 * 1000;

#[derive(Clone, PartialEq)]
pub(in crate::screens::orchestrator) struct TurnStateProps {
  id: String,
  open: OpenProject,
  /// The conversation and the entry of the event; `None` before the conversation is kept.
  entry: Option<(ChatId, i64)>,
  state: EventTurn,
}

impl DevtoolsInspectable for TurnStateProps {}

/// The row of where the turn of `row`'s event stands, keyed `key`; `None` for a row with nothing to say of it, and
/// for a try that thinks, which the working line says.
pub(in crate::screens::orchestrator) fn row(ctx: &mut Ctx, row: &TurnRow, key: &str) -> Option<Element> {
  let state = row.turn.turn_state.as_ref()?;
  if matches!(state, EventTurn::Trying { .. }) {
    return None;
  }
  let id = format!("orchestrator-turn-state-{key}");
  let chat = row.env.scope.as_ref().map(|scope| scope.chat);
  Some(ctx.mount_keyed::<TurnStateRow>(
    &id.clone(),
    TurnStateProps {
      id,
      open: row.env.open.clone(),
      entry: chat.zip(row.turn.seq()),
      state: state.clone(),
    },
  ))
}

/// One more turn the person asked for.
struct Again {
  open: OpenProject,
  entry: (ChatId, i64),
}

pub(in crate::screens::orchestrator) struct TurnStateRow {
  again: FutureAction<Again, (), Message>,
  /// Whether *Try again* takes synthetic input as the person's: in a QA copy only.
  qa_person: QaPerson,
}

impl Component for TurnStateRow {
  type Props = TurnStateProps;

  fn create(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let i18n = ctx.i18n().clone();
    let again = ctx.future_action(move |Again { open, entry }: Again| {
      let (services, i18n) = (services.clone(), i18n.clone());
      async move { try_again(&services, i18n, &open, entry).await }
    });
    Self {
      again,
      qa_person: QaPerson::of(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TurnStateProps>().clone();
    let asked = self.again.state().get();
    let (name, glyph, words, why) = match &props.state {
      EventTurn::TriedAgain { at, next, of, why } => {
        let args = [
          ("time", when(ctx, *at).to_string()),
          ("next", next.to_string()),
          ("of", of.to_string()),
        ];
        let words = ctx.t_args("orchestrator.turnState.triedAgain", args);
        let muted = (Icon::RotateCw, PaletteColor::TextMuted);
        ("tried_again", muted, words, Some(reason(ctx, why)))
      }
      EventTurn::LeftToPerson { tries, why } => {
        let words = if *tries == 1 {
          ctx.t("orchestrator.turnState.left.one")
        } else {
          ctx.t_args("orchestrator.turnState.left.other", [("count", tries.to_string())])
        };
        // A *Try again* that started nothing says why in the reason's place.
        let why = match &asked.error {
          Some(refused) => {
            let refused = message_text(ctx, refused);
            ctx.t_args("orchestrator.turnState.refused", [("reason", &*refused)])
          }
          None => reason(ctx, why),
        };
        let warning = (Icon::CircleAlert, PaletteColor::Warning);
        ("left_to_person", warning, words, Some(why))
      }
      EventTurn::Held(hold) => ("held", (Icon::Pause, PaletteColor::TextMuted), held(ctx, *hold), None),
      // The working line says which try thinks.
      EventTurn::Trying { .. } => return Row::new(),
    };
    let button =
      matches!(props.state, EventTurn::LeftToPerson { .. }).then(|| self.try_again(ctx, &props, asked.is_pending()));
    let why = Text::new(&why.unwrap_or_default())
      .id(format!("{}-why", props.id))
      .variant(OrchestratorText::LineAside.role())
      .nowrap()
      .text_overflow(TextOverflow::Elipsis)
      .flex(1.0)
      .min_width(Dimension::Px(0.0));
    Row::new()
      .id(props.id.clone())
      .describe("turn_state", name)
      .width(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .left(OrchestratorSpacing::LeftOpen.role())
          .right(SpacingSize::Xs),
      )
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .child(icon(glyph.0, IconSize::Base, glyph.1).flex_shrink(0.0))
      .child(
        Text::new(&words)
          .id(format!("{}-words", props.id))
          .variant(OrchestratorText::Line.role())
          .color(PaletteColor::TextSecondary)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex_shrink(1.0)
          .min_width(Dimension::Px(0.0)),
      )
      .child(why)
      .with_children(button)
  }
}

impl TurnStateRow {
  /// **Try again**: one more turn for the event now, a person's press only; it waits while one is being asked for.
  fn try_again(&self, ctx: &Ctx, props: &TurnStateProps, asking: bool) -> ActionButton {
    let button = ActionButton::new(
      format!("{}-try-again", props.id),
      ButtonKind::Secondary,
      ctx.t("orchestrator.turnState.tryAgain"),
    );
    let Some(entry) = props.entry else {
      return button.disabled();
    };
    if asking {
      return button.loading();
    }
    let (again, qa_person, open) = (self.again.clone(), self.qa_person.clone(), props.open.clone());
    button.on_click(move || {
      if qa_person.refuses("ask the Orchestrator to try an event's turn again") {
        return;
      }
      again.run_if_idle(Again {
        open: open.clone(),
        entry,
      });
    })
  }
}

/// *Held until 00:00 — today's budget is used*, or the plan limit's words.
fn held(ctx: &Ctx, hold: Hold) -> Arc<str> {
  let key = match (hold.held, hold.reset_named) {
    (Held::Budget { .. }, _) => "orchestrator.turnState.heldBudget",
    (Held::PlanLimit { .. }, true) => "orchestrator.turnState.heldPlan",
    (Held::PlanLimit { .. }, false) => "orchestrator.turnState.heldPlanUnnamed",
  };
  ctx.t_args(key, [("time", &*when(ctx, hold.held.until()))])
}

/// *Orchestrator is held until 00:00*, the header's quiet line while the turns of the conversation's events are held.
pub(in crate::screens::orchestrator) fn held_line(ctx: &Ctx, hold: Hold) -> Arc<str> {
  let key = if hold.reset_named {
    "orchestrator.turnState.header"
  } else {
    "orchestrator.turnState.headerUnnamed"
  };
  ctx.t_args(key, [("time", &*when(ctx, hold.held.until()))])
}

/// `at` as a row says a time to come: its clock time today and for the start of tomorrow, the day with it later on.
fn when(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let clock = display::clock(at);
  let today = Timestamp::now().local_day_start();
  let soon = at.local_day_start() == today
    || (at == at.local_day_start() && at.unix_millis() - today.unix_millis() <= DAY_AT_MOST);
  if soon {
    return Arc::from(clock);
  }
  let date = super::super::date(ctx, at);
  ctx.t_args(
    "orchestrator.turnState.day",
    [("date", &*date), ("time", clock.as_str())],
  )
}

/// Why a turn failed, as its row adds it: *— the answer took longer than 10 minutes*.
fn reason(ctx: &Ctx, failure: &TurnFailure) -> Arc<str> {
  match failure {
    TurnFailure::TimeLimit { minutes } => ctx.t_args(
      "orchestrator.turnState.why.timeLimit",
      [("minutes", minutes.to_string())],
    ),
    TurnFailure::Failed { reason } => ctx.t_args("orchestrator.turnState.why.failed", [("reason", reason.as_str())]),
    TurnFailure::Interrupted => ctx.t("orchestrator.turnState.why.interrupted"),
    TurnFailure::CannotStart { .. } => ctx.t("orchestrator.turnState.why.cannotStart"),
    TurnFailure::NotSignedIn => ctx.t("orchestrator.turnState.why.notSignedIn"),
    TurnFailure::ConnectionGone => ctx.t("orchestrator.turnState.why.connectionGone"),
    TurnFailure::Account { problem } => match problem {
      AccountStartProblem::NoAccount => ctx.t("orchestrator.turnState.why.accountNone"),
      AccountStartProblem::FolderMissing { folder } => ctx.t_args(
        "orchestrator.turnState.why.accountFolderMissing",
        [("folder", folder.display().to_string().as_str())],
      ),
    },
  }
}
