//! The run's permissions chip in the composer's row (ORC-319; `Permissions chip` `BAck4` and its states in 15f `CimC4`,
//! in the bar of 15 `x55hWH`): a 12 shield and what the run answers its requests with now in 11.5 `text_secondary`,
//! what the profile allows after it in `text_muted`, and a 12 `chevron-down`; 6 apart, 8 at its sides, radius 7, the
//! `pressed` fill while its menu is open and `hover` under the pointer. While the run approves everything its shield is
//! `shield-alert` in `warning`. Its tooltip says what the profile is and what the project allows. It opens the run's
//! menu ([`menu`]), 400 wide, its start at the chip's and its bottom 8 over the composer's box.
//!
//! A change from the menu or from an event line ([`Permissions`]) applies to this run only, from its next request, and
//! is recorded for its Conversation (`orchester_workspace::tasks::run_approvals::change_permissions_in`). Narrowing is
//! anyone's; widening — *Approve everything in this run* — is a person's: an MCP client's press (lurq's synthetic
//! input) is refused, and the refusal shows as an event line.

mod menu;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    events::is_synthetic_input,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, Style},
};
use orchester_core::{
  id::{ProjectId, ResourceId, RunId},
  permission::ApprovalMode,
  run::{Answerer, RunPermissionChange},
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{
    detail::TaskDetail,
    run_approvals::{PermissionChangeError, RunApprovals, change_permissions_in},
    run_permissions::{ChipMode, PermissionsMenu},
  },
};

use crate::{
  mcp::input_gate::{InputGate, persons_press},
  project::{ProjectFeed, ShownProject},
  theme::{ExtraColor, ExtraSpacing, IconSize, PermissionsText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// A change asked of a run's permissions: where, which run, what, and who asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Asked {
  root: PathBuf,
  run: RunId,
  change: RunPermissionChange,
  by: Answerer,
}

impl DevtoolsInspectable for Asked {}

/// What became of a change asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Changed {
  Made,
  /// Automation asked to widen the run: refused, and the refusal recorded.
  Refused,
}

impl DevtoolsInspectable for Changed {}

/// The run's permissions as the tab's chip and its event lines change them: whether the menu is open, and the change
/// under way. Provided once by the tab, so an event line's *Review…* opens the chip's menu.
#[derive(Clone)]
pub(super) struct Permissions {
  pub(super) open: Signal<bool>,
  change: FutureAction<Asked, Changed, Message>,
  gate: Option<InputGate>,
}

impl Permissions {
  /// Provides the tab's permissions; `feed` reads the task again once a change was made or refused.
  pub(super) fn provide(ctx: &mut Ctx, feed: ProjectFeed) {
    let change = ctx.future_action(|asked: Asked| async move {
      background::run(move || change_permissions_in(&asked.root, asked.run, asked.change, asked.by))
        .await?
        .map(|()| Changed::Made)
        .or_else(|error| match error {
          PermissionChangeError::PersonOnly(_) => Ok(Changed::Refused),
          PermissionChangeError::Store(error) => {
            tracing::warn!(%error, "the run's permissions were not changed");
            Err(messages::store_error(&error))
          }
        })
    });
    ctx.watch(&change.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        feed.saved();
      }
    });
    let permissions = Self {
      open: ctx.signal(false),
      change,
      gate: ctx.use_context::<InputGate>(),
    };
    ctx.provide(permissions);
  }

  pub(super) fn of(ctx: &mut Ctx) -> Self {
    ctx
      .use_context::<Self>()
      .expect("the Conversation tab provides the run's permissions")
  }

  /// Asks `change` of `run` of the project at `root`, as whoever gives the input now: a person, or an MCP client's
  /// synthetic input; nothing while a change is under way. A widening is a person's alone
  /// ([`RunPermissionChange::asked_by`]): automation's is refused to it here, and nothing is asked or recorded.
  pub(super) fn ask(&self, root: &std::path::Path, run: RunId, change: RunPermissionChange) {
    if self.change.state().get_untracked().is_pending() {
      return;
    }
    if change.widens() && !self.may_widen() {
      return;
    }
    let by = if is_synthetic_input() {
      Answerer::Automation
    } else {
      Answerer::Person
    };
    self.change.run(Asked {
      root: root.to_path_buf(),
      run,
      change,
      by,
    });
  }

  /// Whether the press being delivered may widen the run: a person's; automation's is refused to it.
  pub(super) fn may_widen(&self) -> bool {
    persons_press(self.gate.as_ref(), "approve everything in a run")
  }

  /// Why the last change was not saved, while it shows.
  fn failure(&self) -> Option<Message> {
    let state = self.change.state().get();
    let rejected = state.is_rejected();
    state.error.filter(|_| rejected)
  }
}

/// The chip of the latest run of the task `detail` of `project` while the run has not ended, in the composer whose box
/// is `box_anchor`.
pub(super) fn chip(
  ctx: &mut Ctx,
  (project, detail): (&ShownProject, &TaskDetail),
  box_anchor: &ElementRef,
) -> Option<Element> {
  let run = detail.run.as_ref()?;
  if run.run.status().is_terminal() {
    return None;
  }
  let step = run.current_step();
  let open = &project.open;
  let props = ChipProps {
    shown: project.clone(),
    root: open.repo_root.clone(),
    project: (open.project.id(), open.project.name().as_str().into()),
    run: (run.id(), run.run.number().get()),
    agent: step.and_then(|step| step.agent.clone()).unwrap_or_else(|| "".into()),
    profile: step
      .and_then(|step| step.permissions.as_ref())
      .map(|permissions| permissions.profile_id),
    approvals: run.approvals.clone(),
    box_anchor: box_anchor.clone(),
  };
  Some(ctx.mount_keyed::<PermissionsChip>(&format!("conversation-permissions-{}", run.id()), props))
}

/// What the chip and its menu show of one run.
#[derive(Clone)]
struct ChipProps {
  shown: ShownProject,
  root: PathBuf,
  project: (ProjectId, Arc<str>),
  /// The run's id and number.
  run: (RunId, u32),
  /// The agent at work, or the last one that ran.
  agent: Arc<str>,
  profile: Option<ResourceId>,
  approvals: RunApprovals,
  /// The composer's box, which the menu opens over.
  box_anchor: ElementRef,
}

/// Compared by what it shows; the box is the composer's for as long as it shows.
impl PartialEq for ChipProps {
  fn eq(&self, other: &Self) -> bool {
    self.shown == other.shown
      && self.root == other.root
      && self.project == other.project
      && self.run == other.run
      && self.agent == other.agent
      && self.profile == other.profile
      && self.approvals == other.approvals
  }
}

impl DevtoolsInspectable for ChipProps {}

struct PermissionsChip {
  anchor: ElementRef,
  /// *Approve everything in this run* asks in place first.
  confirming: Signal<bool>,
}

impl Component for PermissionsChip {
  type Props = ChipProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ElementRef::new(),
      confirming: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChipProps>().clone();
    let permissions = Permissions::of(ctx);
    let shown = PermissionsMenu::of(&props.approvals);
    let open = permissions.open.get();
    if !open && self.confirming.get_untracked() {
      self.confirming.set(false);
    }
    let face = face(ctx, &shown);
    let toggle = permissions.open.clone();
    let chip = pressable(move || toggle.update(|open| *open = !*open))
      .id("conversation-permissions")
      .ref_element(self.anchor.clone())
      .describe("state", chip_name(shown.chip))
      .describe("open", if open { "true" } else { "false" })
      .describe(
        "a11y_name",
        ctx.t_args(
          "conversation.permissions.chip.name",
          [("run", props.run.1.to_string().as_str())],
        ),
      )
      .padding_horizontal(SpacingSize::Sm)
      .rounded(RadiusSize::Md)
      .hovered_style(Style::new().background(ExtraColor::Hover.role()));
    let chip = if open {
      chip.background(ExtraColor::Pressed.role())
    } else {
      chip
    }
    .child(face);
    let tip = tip(ctx, &shown, &props);
    let chip = Tooltip::new("conversation-permissions", tip).wrap(ctx, chip);
    let panel = menu::panel(ctx, (&props, &shown), (&permissions, &self.confirming));
    // The menu's start at the chip's, its bottom 8 over the composer's box: the box is its anchor, moved along by
    // where the chip is in it.
    let start = self.anchor.x() - props.box_anchor.x();
    let gap = ctx.theme().spacing_value(SpacingSize::Sm).to_px();
    let popup = Popup::new(props.box_anchor.clone(), panel)
      .open(permissions.open.clone())
      .switch_on_anchor_press(false)
      .placement(Placement::TopStart)
      .offset(start, gap);
    Row::new().child(chip).child(popup)
  }
}

/// The chip's shield, what the run answers with and what the profile allows, and its chevron.
fn face(ctx: &Ctx, shown: &PermissionsMenu) -> Row {
  let (glyph, color) = match shown.chip {
    ChipMode::Everything => (Icon::ShieldAlert, PaletteColor::Warning),
    ChipMode::Autopilot => (Icon::ShieldCheck, PaletteColor::TextSecondary),
    ChipMode::Manual | ChipMode::ManualRun | ChipMode::ManualProject => (Icon::Shield, PaletteColor::TextSecondary),
  };
  let profile = match shown.profile {
    ApprovalMode::Manual => "conversation.permissions.chip.profileManual",
    ApprovalMode::Autopilot => "conversation.permissions.chip.profileAutopilot",
  };
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(glyph, IconSize::Sm, color).id("conversation-permissions-icon"))
    .child(
      Text::new(&ctx.t(chip_key(shown.chip)))
        .id("conversation-permissions-label")
        .variant(PermissionsText::Chip.role())
        .nowrap(),
    )
    .child(
      Text::new(&ctx.t(profile))
        .id("conversation-permissions-profile")
        .variant(PermissionsText::Chip.role())
        .color(PaletteColor::TextMuted)
        .nowrap(),
    )
    .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted))
}

/// The chip's tooltip: what the profile is and what the project allows, in a sentence.
fn tip(ctx: &Ctx, shown: &PermissionsMenu, props: &ChipProps) -> Arc<str> {
  let key = match (shown.profile, props.approvals.allow_autopilot.is_on()) {
    (ApprovalMode::Manual, _) => "conversation.permissions.chip.tip.manual",
    (ApprovalMode::Autopilot, true) => "conversation.permissions.chip.tip.autopilot",
    (ApprovalMode::Autopilot, false) => "conversation.permissions.chip.tip.projectOff",
  };
  ctx.t_args(key, [("agent", &*props.agent), ("project", &*props.project.1)])
}

const fn chip_key(mode: ChipMode) -> &'static str {
  match mode {
    ChipMode::Manual => "conversation.permissions.chip.manual",
    ChipMode::Autopilot => "conversation.permissions.chip.autopilot",
    ChipMode::Everything => "conversation.permissions.chip.everything",
    ChipMode::ManualRun => "conversation.permissions.chip.manualRun",
    ChipMode::ManualProject => "conversation.permissions.chip.manualProject",
  }
}

/// The name `describe` reports the chip by, for tests and QA.
const fn chip_name(mode: ChipMode) -> &'static str {
  match mode {
    ChipMode::Manual => "manual",
    ChipMode::Autopilot => "autopilot",
    ChipMode::Everything => "everything",
    ChipMode::ManualRun => "manual-run",
    ChipMode::ManualProject => "manual-project",
  }
}
