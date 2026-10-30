//! The Base branch chip of New module and Edit module (`Base branch` `shP2f` in 02 `eyrnI`), after the dates: the
//! checked-out branch by default, the branch's name once one is set (`git-branch`). Its menu (`Base branch menu`
//! `mTQMc`, 360 wide) finds a branch as it is typed (`Find` `QQdTx`) and offers the checked-out branch with the one the
//! checkout has now, the repository's branches and, under *Remote only*, the remote-tracking ones whose local branch a
//! choice creates, the choice checked; its foot says how many of the module's tasks without a base branch of their own
//! run from it, and that runs under way keep theirs (`Note` `SSlIo`). The choice is saved with the form
//! (`super::save`).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureState},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  core::Signal,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  id::ModuleId,
  integration::base::{BranchPick, RemoteBranch},
  run::BranchName,
};
use orchester_workspace::{
  background,
  messages::Message,
  modules::base::{ModuleBasePick, ModuleBaseRead, module_base_in},
};

use super::form::FormProps;
use crate::{
  screens::task_form::{
    bar::HEIGHT as BAR_HEIGHT,
    chips::{ChipState, HelpPlace, Look, MenuOption, PropertyChip},
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{field::FieldInput, filter::filter_menu_panel, icon::Icon, message::message_text, popover::room_under},
};

/// The menu's width (`mTQMc`).
const MENU_WIDTH: f32 = 360.0;
/// The height of *Find a branch* (`QQdTx`).
const FIND_HEIGHT: f32 = 32.0;

/// The base branch the form holds.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BasePick(pub ModuleBasePick);

impl DevtoolsInspectable for BasePick {}

/// What the menu offers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read(ModuleBaseRead);

impl DevtoolsInspectable for Read {}

/// The chip's popover and what *Find a branch* holds.
#[derive(Clone)]
pub struct BaseChip {
  pub state: ChipState,
  pub find: Signal<String>,
}

impl BaseChip {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      state: ChipState::new(ctx),
      find: ctx.signal(String::new()),
    }
  }
}

pub fn chip(ctx: &mut Ctx, props: &FormProps, chosen: &Signal<BasePick>, base: &BaseChip) -> Element {
  let read = read(ctx, props);
  let pick = chosen.get().0;
  let (look, value) = match pick.branch() {
    Some(branch) => (Look::Set, Arc::from(branch.as_str())),
    None => (Look::Empty, ctx.t("modules.form.base.checkedOut")),
  };
  let panel = base.state.is_open().then(|| menu(ctx, &read, chosen, base));
  PropertyChip::new(
    "module-base",
    Icon::GitBranch,
    (ctx.t("modules.form.base.label"), value),
    look,
  )
  .render(&base.state, panel)
}

/// The repository's branches and the module's tasks that take its base branch, read once for the form.
fn read(ctx: &mut Ctx, props: &FormProps) -> FutureState<Read, Message> {
  let programs = Services::of(ctx).programs;
  let module = props.module.as_ref().map(|module| module.id());
  let root = props.project.open.repo_root.clone();
  ctx
    .future((root, module), move |(root, module): (PathBuf, Option<ModuleId>)| {
      let programs = programs.clone();
      async move { background::run(move || module_base_in(&root, &programs, module).map(Read)).await? }
    })
    .state()
    .get()
}

/// The menu, as tall as the room between the chip and the action bar lets it be: *Find a branch*, the checked-out
/// branch and the note stay, the branches between them scroll.
fn menu(ctx: &mut Ctx, read: &FutureState<Read, Message>, chosen: &Signal<BasePick>, base: &BaseChip) -> Element {
  let find = FieldInput::new("module-base-find", ctx.t("modules.form.base.find"), base.find.clone())
    .placeholder(ctx.t("modules.form.base.find"))
    .search()
    .text_style(ExtraTypography::Lead.role())
    .box_height(FIND_HEIGHT)
    .render(ctx);
  let mut panel = filter_menu_panel("module-base-menu", MENU_WIDTH).child(find);
  if let Some(room) = room_under(ctx, &base.state.anchor, BAR_HEIGHT) {
    panel = panel.max_height(Dimension::Px(room));
  }
  let Some(Read(read)) = &read.data else {
    let words = match &read.error {
      Some(error) => message_text(ctx, error),
      None => ctx.t("modules.form.base.reading"),
    };
    return panel.child(foot(&words, "module-base-reading")).into();
  };
  let lists = ScrollVertical::new(branches(ctx, read, chosen, base))
    .id("module-base-scroll")
    .width(Dimension::Pct(100.0))
    .flex_shrink(1.0);
  panel
    .child(checked_out(ctx, read, chosen, base))
    .child(separator(ctx))
    .child(lists)
    .child(note(ctx, read.inheriting))
    .into()
}

/// The branches *Find a branch* matches, under *Branches* and *Remote only*, or that none does.
fn branches(ctx: &mut Ctx, read: &ModuleBaseRead, chosen: &Signal<BasePick>, base: &BaseChip) -> Column {
  let pick = chosen.get().0;
  let mut panel = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Tight.role());
  let typed = base.find.get().trim().to_lowercase();
  let matches = |name: &str| name.to_lowercase().contains(&typed);
  let mut local: Vec<&BranchName> = read
    .choices
    .local
    .iter()
    .filter(|branch| matches(branch.as_str()))
    .collect();
  // A branch set that the repository no longer has stays offered by its name, so the menu shows what is chosen.
  if let ModuleBasePick::Branch(BranchPick::Local(set)) = &pick
    && !read.choices.has_local(set)
    && matches(set.as_str())
  {
    local.insert(0, set);
  }
  let remote: Vec<&RemoteBranch> = read
    .choices
    .remote
    .iter()
    .filter(|remote| matches(remote.tracking()))
    .collect();
  if local.is_empty() && remote.is_empty() {
    panel = panel.child(foot(&ctx.t("modules.form.base.noMatch"), "module-base-no-match"));
  }
  if !local.is_empty() {
    panel = panel.child(heading(ctx, "modules.form.base.branches"));
  }
  for (index, branch) in local.into_iter().enumerate() {
    let choice = ModuleBasePick::Branch(BranchPick::Local(branch.clone()));
    panel = panel.child(option(
      (&format!("module-base-branch-{index}"), Icon::GitBranch),
      (branch.as_str().into(), None),
      (choice, chosen, base),
    ));
  }
  if !remote.is_empty() {
    panel = panel.child(heading(ctx, "modules.form.base.remote"));
  }
  for (index, remote) in remote.into_iter().enumerate() {
    let creates = ctx.t_args("modules.form.base.creates", [("branch", remote.local().as_str())]);
    let choice = ModuleBasePick::Branch(BranchPick::Remote(remote.clone()));
    panel = panel.child(option(
      (&format!("module-base-remote-{index}"), Icon::Cloud),
      (remote.tracking().into(), Some(creates)),
      (choice, chosen, base),
    ));
  }
  panel
}

/// *Checked-out branch*, with the branch the checkout has now (`Option · Checked-out branch` `NPwra`).
fn checked_out(ctx: &mut Ctx, read: &ModuleBaseRead, chosen: &Signal<BasePick>, base: &BaseChip) -> Element {
  let help = match &read.checked_out {
    Some(branch) => ctx.t_args("modules.form.base.checkedOutNow", [("branch", branch.as_str())]),
    None => ctx.t("modules.form.base.checkedOutDetached"),
  };
  option(
    ("module-base-checked-out", Icon::GitCommitHorizontal),
    (ctx.t("modules.form.base.checkedOut"), Some(help)),
    (ModuleBasePick::CheckedOut, chosen, base),
  )
}

/// One branch, its name in Geist Mono but for the checked-out branch's, chosen as the form's base branch.
fn option(
  (id, glyph): (&str, Icon),
  (title, help): (Arc<str>, Option<Arc<str>>),
  (choice, chosen, base): (ModuleBasePick, &Signal<BasePick>, &BaseChip),
) -> Element {
  let picked = chosen.get_untracked().0 == choice;
  let mono = choice != ModuleBasePick::CheckedOut;
  let (chosen, find) = (chosen.clone(), base.find.clone());
  MenuOption {
    id: id.to_owned(),
    icon: glyph,
    title,
    help,
    help_place: HelpPlace::UnderIcon,
    chosen: picked,
    mono,
  }
  .render(&base.state.open, move || {
    chosen.set(BasePick(choice.clone()));
    find.set(String::new());
  })
}

/// The 1 `border` line under the checked-out branch (`Separator` `h9QUfX`).
fn separator(ctx: &Ctx) -> Element {
  Row::new()
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(ctx.theme().border_size_value(BorderSize::Sm)))
    .background(PaletteColor::Border)
    .into()
}

/// A group's heading, *Branches* or *Remote only*, 11/500 `text_muted` (`Label · Branches` `n2cRi`).
fn heading(ctx: &Ctx, key: &str) -> Element {
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
      Text::new(&ctx.t(key))
        .variant(TypographyStyle::FieldLabel)
        .color(PaletteColor::TextMuted),
    )
    .into()
}

/// How many of the module's tasks run from its base branch, and that runs under way keep theirs, over a `border` line
/// (`Note` `SSlIo`).
fn note(ctx: &Ctx, inheriting: usize) -> Element {
  let words = match inheriting {
    0 => ctx.t("modules.form.base.note.none"),
    1 => ctx.t("modules.form.base.note.one"),
    count => ctx.t_args("modules.form.base.note.other", [("count", count.to_string())]),
  };
  Column::new()
    .width(Dimension::Pct(100.0))
    .border_top(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(foot(&words, "module-base-note"))
    .into()
}

/// A line of 11.5 `text_muted` words in the menu, 8 over, 4 under and 10 at its sides.
fn foot(words: &str, id: &str) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Xs)
        .left(ExtraSpacing::Loose.role())
        .right(ExtraSpacing::Loose.role()),
    )
    .child(
      Text::new(words)
        .id(id.to_owned())
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}
