//! A task the Orchestrator parked for the person, on its Summary (`W02 · KON-47 · Summary · parked for you` on the
//! board `Orchestrator autonomy · frames to approve`; `docs/plan/orchestrator-autonomy/OA-15-…`, `OA-23-…`): the
//! first row of the column, in the warning tone of a wait that cannot be met (`rlnEZ` f), drawn as the strip a team
//! task's lines are (`crate::team::strip`). It says what is needed, and under it why the Orchestrator could not go
//! on and what to do, in its own words; **Give it back** at its end ends the parking, and the conversation that
//! leads the task takes it up again. Giving back is a person's: synthetic input is refused outside a QA copy. The
//! parking also ends by itself when the task's run moves.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::TaskId,
  orchestrator::parking::{Needs, Parking},
};
use orchester_workspace::{background, messages::Message, orchestrator::parking::give_back_in};

use super::TaskContext;
use crate::{
  mcp::qa_person::QaPerson,
  team::strip::{Strip, StripTone},
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

/// **Give it back**, which only a person presses.
pub const GIVE_BACK: &str = "task-parked-give-back";

#[derive(Clone, PartialEq)]
pub struct Props {
  task: TaskContext,
  parking: Parking,
}

impl DevtoolsInspectable for Props {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GivenBack;

impl DevtoolsInspectable for GivenBack {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failed(Message);

impl DevtoolsInspectable for Failed {}

type GiveBack = FutureAction<(PathBuf, TaskId), GivenBack, Failed>;

/// The row for `task` while it is parked.
pub fn shown(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let parking = task.detail.parked.clone()?;
  Some(ctx.mount_keyed::<ParkedRow>(
    "task-parked-row",
    Props {
      task: task.clone(),
      parking,
    },
  ))
}

/// What needs the person, as the row's title and the header's status line say it.
fn needed(ctx: &Ctx, needs: Needs) -> std::sync::Arc<str> {
  ctx.t(match needs {
    Needs::Decision => "task.parked.decision",
    Needs::Approval => "task.parked.approval",
    Needs::SignIn => "task.parked.signIn",
    Needs::Tool => "task.parked.tool",
    Needs::Ceiling => "task.parked.ceiling",
    Needs::Other => "task.parked.other",
  })
}

/// The header's status line of a parked task (`Status line` in W02): *Parked for you — an approval is needed*, in
/// the warning tone after its glyph.
pub fn status_line(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let parking = task.detail.parked.as_ref()?;
  let line = Row::new()
    .id("task-status-row")
    .width(Dimension::Pct(100.0))
    .min_width(Dimension::Px(0.0))
    .child(
      Row::new()
        .id("task-status")
        .describe("status", "parked")
        .spacing(ExtraSpacing::Pair.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::CirclePause, IconSize::Base, PaletteColor::Warning).id("task-status-icon"))
        .child(
          Text::new(&needed(ctx, parking.needs))
            .id("task-status-label")
            .variant(TypographyStyle::Label)
            .color(PaletteColor::Warning)
            .nowrap(),
        ),
    );
  Some(line.into())
}

pub struct ParkedRow {
  give_back: GiveBack,
}

impl Component for ParkedRow {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let give_back: GiveBack = ctx.future_action(move |(root, task): (PathBuf, TaskId)| async move {
      background::run(move || give_back_in(&root, task))
        .await
        .map_err(Failed)?
        .map(|_| GivenBack)
        .map_err(Failed)
    });
    let feed = ctx.props::<Props>().task.project.feed.clone();
    ctx.watch(&give_back.state(), move |state| {
      if state.is_fulfilled() {
        feed.saved();
      }
    });
    Self { give_back }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { task, parking } = ctx.props::<Props>().clone();
    let state = self.give_back.state().get();
    let title = needed(ctx, parking.needs);
    let detail = ctx.t_args(
      "task.parked.words",
      [("why", parking.why.as_str()), ("toDo", parking.to_do.as_str())],
    );
    let button = ActionButton::new(GIVE_BACK, ButtonKind::Secondary, ctx.t("task.parked.giveBack"));
    let button = if state.is_pending() {
      button.loading()
    } else {
      let (give_back, person) = (self.give_back.clone(), QaPerson::of(ctx));
      let args = (task.project.open.repo_root, parking.task);
      button.on_click(move || {
        if !person.refuses("give a parked task back to the Orchestrator") {
          give_back.run(args.clone());
        }
      })
    };
    let failed = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|Failed(message)| field_error("task-parked-failed", &message_text(ctx, message)));
    Column::new()
      .id("task-parked")
      .describe("needs", parking.needs.name())
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Strip::new(
          "task-parked-strip",
          (Icon::CirclePause, PaletteColor::Warning),
          title,
          detail,
        )
        .tone(StripTone::Warning)
        .action(button),
      )
      .with_children(failed)
  }
}
