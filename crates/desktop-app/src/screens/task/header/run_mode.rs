//! The run's approval mode on the status line (`docs/design/approval-modes.md`; `Mode chip · Autopilot` `CLsZ2` and
//! `Switch to Manual for this run` `YxJCZ` in 04 `H6kPd`, the chips of 04c `vZcoN`): a tag chip that names the mode
//! the run answers its agents' requests in now and why it is Manual (its profile, its project, or the run itself),
//! shown while the run has not ended. While Autopilot answers them, the words after it switch the run to Manual for
//! itself, for good: the chip then reads *Manual · this run* and the Conversation records the switch. Switching to
//! Manual narrows what agents do without a person, so an MCP client's press is taken too, as automation's.

use std::path::PathBuf;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  id::RunId,
  run::{Answerer, EffectiveMode, ManualBecause},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::run_approvals::{RunApprovalChange, apply_in},
};

use crate::{
  project::ProjectFeed,
  screens::task::TaskContext,
  ui::{chip::TagChip, field::field_error, message::message_text, pressable::pressable},
};

/// The mode chip of `task`'s latest run, and its switch while it may be switched; nothing once the run ended.
pub fn mode(ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
  let run = task.detail.run.as_ref()?;
  if run.run.status().is_terminal() {
    return None;
  }
  Some(ctx.mount_keyed::<RunMode>(
    &format!("task-run-mode-{}", run.id()),
    ModeProps {
      root: task.project.open.repo_root.clone(),
      feed: task.project.feed.clone(),
      run: run.id(),
      mode: run.approvals.mode,
      may_switch: run.approvals.may_switch_to_manual(&run.run),
    },
  ))
}

#[derive(Clone, PartialEq)]
pub struct ModeProps {
  root: PathBuf,
  feed: ProjectFeed,
  run: RunId,
  mode: EffectiveMode,
  may_switch: bool,
}

impl DevtoolsInspectable for ModeProps {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switched;

impl DevtoolsInspectable for Switched {}

pub struct RunMode {
  switch: FutureAction<Answerer, Switched, Message>,
}

impl Component for RunMode {
  type Props = ModeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<ModeProps>().clone();
    let (root, run) = (props.root, props.run);
    let switch = ctx.future_action(move |by: Answerer| {
      let root = root.clone();
      async move {
        background::run(move || apply_in(&root, run, RunApprovalChange::SwitchToManual(by)))
          .await?
          .map(|()| Switched)
          .map_err(|error| {
            tracing::warn!(%error, "the run was not switched to Manual");
            messages::store_error(&error)
          })
      }
    });
    let feed = props.feed;
    ctx.watch(&switch.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        feed.saved();
      }
    });
    Self { switch }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ModeProps>().clone();
    let state = self.switch.state().get();
    let chip = TagChip::new("task-run-mode", ctx.t(chip_key(props.mode)));
    let switch = props.may_switch.then(|| {
      let words = Text::new(&ctx.t("task.run.mode.switchToManual"))
        .variant(TypographyStyle::Button)
        .color(PaletteColor::TextSecondary)
        .nowrap();
      let action = self.switch.clone();
      // A press while the switch is under way asks nothing more: the run is switched once.
      let pending = state.is_pending();
      pressable(move || {
        if !pending {
          action.run(answerer());
        }
      })
      .id("task-run-mode-switch")
      .child(words)
    });
    let refused = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|message| field_error("task-run-mode-error", &message_text(ctx, message)));
    Row::new()
      .id("task-run-mode-row")
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(chip)
      .with_children(switch)
      .with_children(refused)
  }
}

/// Who presses the switch: a person, or automation through an MCP client's window input.
fn answerer() -> Answerer {
  if is_synthetic_input() {
    Answerer::Automation
  } else {
    Answerer::Person
  }
}

/// The chip's words for `mode` (04c): *Autopilot*, *Manual · profile*, *Manual · project*, *Manual · this run*.
const fn chip_key(mode: EffectiveMode) -> &'static str {
  match mode {
    EffectiveMode::Autopilot => "task.run.mode.autopilot",
    EffectiveMode::Manual(ManualBecause::Profile) => "task.run.mode.manualProfile",
    EffectiveMode::Manual(ManualBecause::Project) => "task.run.mode.manualProject",
    EffectiveMode::Manual(ManualBecause::Run) => "task.run.mode.manualRun",
  }
}
