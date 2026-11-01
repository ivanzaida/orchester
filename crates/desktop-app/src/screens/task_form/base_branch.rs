//! The Base branch chip after Module and its menu (`Base branch · redesign` `ri5ru`, 01 `eUFSd`: `Base branch` `aaqSu`,
//! `Base branch menu` `fSsas`): where the task's runs start from, saved after its module. *Find a branch* narrows the
//! branches; *Inherit* takes the module's base branch, named (the default), or the branch checked out when a run
//! starts; *Branches* are the repository's local ones in Geist Mono; *Remote only* the remote-tracking ones with no
//! local branch of their name, whose local branch is created when the task is saved. The menu reads the branches each
//! time it opens.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  integration::base::{BaseChoice, BranchPick},
  module::Module,
};
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  messages::Message,
  tasks::base_branch::{BasePick, BranchMenu, branch_menu},
};

use super::{
  bar::HEIGHT as BAR_HEIGHT,
  chips::{ChipState, HelpPlace, Look, MenuOption, PropertyChip},
  state::View,
};
use crate::{
  theme::{ExtraSpacing, TaskFormText},
  ui::{
    field::FieldInput, filter::filter_menu_panel, icon::Icon, menu::separator, message::message_text,
    popover::room_under,
  },
};

/// The menu's width (`Base branch menu` `fSsas`).
const MENU_WIDTH: f32 = 380.0;
/// *Find a branch*'s height (`Find` `qoTLj`).
const FIND_HEIGHT: f32 = 32.0;

/// The module the task is in, as the menu names it: its name and its base branch.
#[derive(Clone, Debug, PartialEq, Eq)]
struct InModule {
  name: Arc<str>,
  base: BaseChoice,
}

/// The chip and, while it is open, its menu.
pub fn base_branch_chip(
  ctx: &mut Ctx,
  view: &View,
  (modules, state, find): (&[Module], &ChipState, &Signal<String>),
  (pick, repo_root, programs): (&Signal<BasePick>, PathBuf, Programs),
) -> Element {
  let module = view
    .module
    .and_then(|id| modules.iter().find(|module| module.id() == id))
    .map(|module| InModule {
      name: module.name().as_str().into(),
      base: module.base().clone(),
    });
  let (look, glyph, value) = chip_value(ctx, &view.base_branch, module.as_ref());
  let panel = state.is_open().then(|| {
    let props = MenuProps {
      repo_root,
      programs,
      module,
      pick: pick.clone(),
      open: state.open.clone(),
      find: find.clone(),
      anchor: state.anchor.clone(),
    };
    ctx.mount_keyed::<BranchMenuPanel>("task-form-base-branch-panel", props)
  });
  PropertyChip::new(
    "task-form-base-branch",
    glyph,
    (ctx.t("task_form.fields.baseBranch.label"), value),
    look,
  )
  .locked(view.locked())
  .render(state, panel)
}

/// What the chip says of `pick`: the module's branch *from module*, the checked-out branch, or the branch set; its name
/// and *Checked-out branch* while the task inherits a module that sets none, or is in none.
fn chip_value(ctx: &Ctx, pick: &BasePick, module: Option<&InModule>) -> (Look, Icon, Arc<str>) {
  let inherited = module.and_then(|module| module.base.branch());
  match (pick, inherited) {
    (BasePick::Inherit, Some(branch)) => (
      Look::Set,
      Icon::GitBranch,
      ctx.t_args("task_form.fields.baseBranch.fromModule", [("branch", branch.as_str())]),
    ),
    (BasePick::Inherit, None) => (
      Look::Empty,
      Icon::GitBranch,
      ctx.t("task_form.fields.baseBranch.checkedOut"),
    ),
    (BasePick::CheckedOut, _) => (
      Look::Set,
      Icon::GitCommitHorizontal,
      ctx.t("task_form.fields.baseBranch.checkedOut"),
    ),
    (BasePick::Branch(_), _) => (
      Look::Set,
      Icon::GitBranch,
      pick.branch().map(|branch| branch.as_str().into()).unwrap_or_default(),
    ),
  }
}

#[derive(Clone)]
struct MenuProps {
  repo_root: PathBuf,
  programs: Programs,
  module: Option<InModule>,
  pick: Signal<BasePick>,
  open: Signal<bool>,
  find: Signal<String>,
  /// The chip, which the menu opens under.
  anchor: ElementRef,
}

/// Compared by what the menu shows; the signals by their ids.
impl PartialEq for MenuProps {
  fn eq(&self, other: &Self) -> bool {
    self.repo_root == other.repo_root
      && self.programs == other.programs
      && self.module == other.module
      && self.pick.id() == other.pick.id()
      && self.open.id() == other.open.id()
      && self.find.id() == other.find.id()
  }
}

impl DevtoolsInspectable for MenuProps {}

/// The menu, mounted while it is open: it reads the branches when it opens.
struct BranchMenuPanel;

impl Component for BranchMenuPanel {
  type Props = MenuProps;

  fn create(_: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<MenuProps>().clone();
    let programs = props.programs.clone();
    let read = ctx.future(props.repo_root.clone(), move |root| {
      let programs = programs.clone();
      async move { background::run(move || branch_menu(&programs, &root)).await? }
    });
    let state = read.state().get();
    let menu = state.data.as_ref();
    let find = FieldInput::new(
      "task-form-base-branch-find",
      ctx.t("task_form.fields.baseBranch.find"),
      props.find.clone(),
    )
    .search()
    .placeholder(ctx.t("task_form.fields.baseBranch.find"))
    .box_height(FIND_HEIGHT)
    .render(ctx);
    let chosen = props.pick.get();
    let mut panel = filter_menu_panel("task-form-base-branch-menu", MENU_WIDTH)
      .child(find)
      .child(head(&ctx.t("task_form.fields.baseBranch.inherit")))
      .child(from_module(ctx, &props, &chosen))
      .child(checked_out(ctx, &props, (&chosen, menu)))
      .child(separator(ctx))
      .child(branches(ctx, &props, (&chosen, menu, state.error.as_ref())));
    if let Some(room) = room_under(ctx, &props.anchor, BAR_HEIGHT) {
      panel = panel.max_height(Dimension::Px(room));
    }
    panel
  }
}

/// A group's heading (`Label · Inherit` `gUDDx`): 11/500 `text_muted`, 8 over it, 2 under and 10 at its sides.
fn head(text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(ExtraSpacing::Tight.role())
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .child(
      Text::new(text)
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// *From module · Webhooks v2* with the module's base branch under it (`SBaMd`), or that the task is in no module.
fn from_module(ctx: &Ctx, props: &MenuProps, chosen: &BasePick) -> Element {
  let (title, help) = match &props.module {
    Some(module) => (
      ctx.t_args(
        "task_form.fields.baseBranch.fromModuleOption",
        [("module", &*module.name)],
      ),
      match module.base.branch() {
        Some(branch) => ctx.t_args(
          "task_form.fields.baseBranch.fromModuleBranch",
          [("branch", branch.as_str())],
        ),
        None => ctx.t("task_form.fields.baseBranch.fromModuleCheckedOut"),
      },
    ),
    None => (
      ctx.t("task_form.fields.baseBranch.noModuleOption"),
      ctx.t("task_form.fields.baseBranch.noModuleHelp"),
    ),
  };
  let pick = props.pick.clone();
  MenuOption {
    id: "task-form-base-branch-inherit".to_owned(),
    icon: Icon::Target,
    title,
    mono: false,
    help: Some(help),
    help_place: HelpPlace::UnderIcon,
    chosen: *chosen == BasePick::Inherit,
  }
  .render(&props.open, move || pick.set(BasePick::Inherit))
}

/// *Checked-out branch* with the branch checked out now under it (`IQv7y`).
fn checked_out(ctx: &Ctx, props: &MenuProps, (chosen, menu): (&BasePick, Option<&BranchMenu>)) -> Element {
  let help = match menu {
    Some(BranchMenu {
      checked_out: Some(branch),
      ..
    }) => ctx.t_args(
      "task_form.fields.baseBranch.checkedOutNow",
      [("branch", branch.as_str())],
    ),
    Some(BranchMenu { checked_out: None, .. }) => ctx.t("task_form.fields.baseBranch.checkedOutDetached"),
    None => ctx.t("task_form.fields.baseBranch.checkedOutLater"),
  };
  let pick = props.pick.clone();
  MenuOption {
    id: "task-form-base-branch-checked-out".to_owned(),
    icon: Icon::GitCommitHorizontal,
    title: ctx.t("task_form.fields.baseBranch.checkedOut"),
    mono: false,
    help: Some(help),
    help_place: HelpPlace::UnderIcon,
    chosen: *chosen == BasePick::CheckedOut,
  }
  .render(&props.open, move || pick.set(BasePick::CheckedOut))
}

/// *Branches* and *Remote only* that hold what *Find a branch* holds, in a list that scrolls in the room the menu
/// has; while they are read, or why they cannot be, a line in their place.
fn branches(
  ctx: &mut Ctx,
  props: &MenuProps,
  (chosen, menu, error): (&BasePick, Option<&BranchMenu>, Option<&Message>),
) -> Element {
  let list = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role());
  let Some(menu) = menu else {
    let line = error.map_or_else(
      || ctx.t("task_form.fields.baseBranch.reading"),
      |error| message_text(ctx, error),
    );
    return list
      .child(head(&ctx.t("task_form.fields.baseBranch.branches")))
      .child(note(&line))
      .into();
  };
  let find = props.find.get();
  let wanted = find.trim().to_lowercase();
  let matches = |name: &str| wanted.is_empty() || name.to_lowercase().contains(&wanted);
  let local: Vec<Element> = menu
    .choices
    .local
    .iter()
    .filter(|branch| matches(branch.as_str()))
    .enumerate()
    .map(|(index, branch)| {
      let (pick, picked) = (props.pick.clone(), BasePick::Branch(BranchPick::Local(branch.clone())));
      MenuOption {
        id: format!("task-form-base-branch-local-{index}"),
        icon: Icon::GitBranch,
        title: branch.as_str().into(),
        mono: true,
        help: None,
        help_place: HelpPlace::UnderIcon,
        chosen: chosen.branch() == Some(branch),
      }
      .render(&props.open, move || pick.set(picked.clone()))
    })
    .collect();
  let remote: Vec<Element> = menu
    .choices
    .remote
    .iter()
    .filter(|remote| matches(remote.tracking()))
    .enumerate()
    .map(|(index, remote)| {
      let (pick, picked) = (props.pick.clone(), BasePick::Branch(BranchPick::Remote(remote.clone())));
      MenuOption {
        id: format!("task-form-base-branch-remote-{index}"),
        icon: Icon::Cloud,
        title: remote.tracking().into(),
        mono: true,
        help: Some(ctx.t_args(
          "task_form.fields.baseBranch.createsLocal",
          [("branch", remote.local().as_str())],
        )),
        help_place: HelpPlace::UnderIcon,
        chosen: *chosen == picked,
      }
      .render(&props.open, move || pick.set(picked.clone()))
    })
    .collect();
  let mut list = list.child(head(&ctx.t("task_form.fields.baseBranch.branches")));
  if local.is_empty() && remote.is_empty() {
    list = list.child(note(
      &ctx.t_args("task_form.fields.baseBranch.noMatch", [("text", find.trim())]),
    ));
  }
  list = list.with_children(local);
  if !remote.is_empty() {
    list = list
      .child(head(&ctx.t("task_form.fields.baseBranch.remoteOnly")))
      .with_children(remote);
  }
  ScrollVertical::new(list)
    .id("task-form-base-branch-list")
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0)
    .into()
}

/// A line in place of the branches: that they are being read, why they cannot be, or that none matches.
fn note(text: &str) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .child(
      Text::new(text)
        .id("task-form-base-branch-note")
        .variant(TaskFormText::OptionMeta.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}
