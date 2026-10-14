//! What *Open folder…* found, right under it in the switcher's menu (`Result · …` in `uK2Hw`, `NLDG7`
//! in `VrS47`): on `surface_base`, radius 6, 8 over it, 10 under and at its sides; a 14 icon, then the
//! title in 12.5/500, the folder in Geist Mono 11 and what it means in 11.5, 3 apart; the actions under
//! them, 8 apart and indented past the icon. While the folder's data is read it says so; a refused
//! folder says why and offers another folder, the repository's top folder, the command that makes it
//! a repository, or setting a project up in it (Add project with the folder chosen).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_workspace::messages::Message;

use super::{CheckAction, FolderRefusal, PickAction};
use crate::{
  navigation::Navigation,
  routes::{Route, RouteFolder},
  theme::{ExtraSpacing, ExtraTypography, IconSize, ShellSpacing, ShellText},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::CodeBox,
    icon::{Icon, icon},
    message::message_text,
    unavailable::with_reason,
  },
};

/// Where *Open folder…* is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum FolderStep {
  #[default]
  Idle,
  /// The chosen folder's project data is read.
  Checking(PathBuf),
  /// The chosen folder does not open as a project, and why.
  Refused(PathBuf, FolderRefusal),
  /// The system's folder picker could not be shown, and why.
  PickFailed(Message),
}

impl lurq::app::component::DevtoolsInspectable for FolderStep {}

/// Choosing a folder and opening one.
#[derive(Clone)]
pub struct Picks {
  pub pick: PickAction,
  pub check: CheckAction,
  /// Where *Open folder…* is.
  pub step: Signal<FolderStep>,
  /// The folder picker's title.
  pub title: Arc<str>,
}

impl Picks {
  /// Opens the system's folder picker, unless it or a folder's reading is under way.
  pub fn pick(&self) {
    if !self.pick.state().get_untracked().is_pending() && !self.check.state().get_untracked().is_pending() {
      self.pick.run(self.title.clone());
    }
  }
}

/// Reads the project data of `folder`, saying so meanwhile, unless a reading is under way.
pub fn check(step: &Signal<FolderStep>, check: &CheckAction, folder: PathBuf) {
  if check.state().get_untracked().is_pending() {
    return;
  }
  step.set(FolderStep::Checking(folder.clone()));
  check.run(folder);
}

/// The block for `step`; nothing while no folder was chosen.
pub fn block(ctx: &mut Ctx, step: &FolderStep, picks: &Picks) -> Option<Element> {
  let (folder, glyph, title, body, actions) = match step {
    FolderStep::Idle => return None,
    FolderStep::Checking(folder) => (
      Some(folder),
      Icon::LoaderCircle,
      ctx.t("shell.switcher.folder.checking"),
      Some(ctx.t("shell.switcher.folder.checkingBody").to_string()),
      Vec::new(),
    ),
    FolderStep::Refused(folder, refusal) => {
      let (title, body) = words(ctx, refusal);
      (
        Some(folder),
        Icon::Info,
        title,
        body,
        actions(ctx, Some((folder, refusal)), picks),
      )
    }
    FolderStep::PickFailed(reason) => (
      None,
      Icon::Info,
      ctx.t("shell.switcher.folder.pickFailed"),
      Some(message_text(ctx, reason).to_string()),
      actions(ctx, None, picks),
    ),
  };
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(Text::new(&title).variant(ShellText::FolderTitle.role()))
    .with_children(folder.map(|folder| {
      Text::new(&folder.display().to_string())
        .id("switcher-folder-path")
        .variant(ExtraTypography::MonoNote.role())
    }))
    .with_children(body.map(|body| Text::new(&body).variant(ShellText::FolderBody.role())));
  let head = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
    .child(text);
  let state = match step {
    FolderStep::Checking(_) => "checking",
    FolderStep::PickFailed(_) => "pick-failed",
    _ => "refused",
  };
  let mut block = Column::new()
    .id("switcher-folder")
    .describe("state", state)
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(ExtraSpacing::Loose.role()).top(SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .rounded(RadiusSize::Md)
    .background(PaletteColor::SurfaceBase)
    .child(head);
  if !actions.is_empty() {
    block = block.child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(ShellSpacing::FolderActions.role()))
        .spacing(SpacingSize::Sm)
        .with_children(actions),
    );
  }
  Some(block.into())
}

/// The refusal's title and what it means.
fn words(ctx: &Ctx, refusal: &FolderRefusal) -> (Arc<str>, Option<String>) {
  match refusal {
    FolderRefusal::NotRepository => (
      ctx.t("shell.switcher.folder.notRepository"),
      Some(ctx.t("shell.switcher.folder.notRepositoryBody").to_string()),
    ),
    FolderRefusal::NotTopFolder { top } => (
      ctx.t("shell.switcher.folder.notTop"),
      Some(
        ctx
          .t_args("shell.switcher.folder.notTopBody", [("top", top.display().to_string())])
          .to_string(),
      ),
    ),
    FolderRefusal::NoProject => (
      ctx.t("shell.switcher.folder.noProject"),
      Some(ctx.t("shell.switcher.folder.noProjectBody").to_string()),
    ),
    FolderRefusal::Newer { found, supported } => (
      ctx.t("shell.switcher.folder.newer"),
      Some(
        ctx
          .t_args(
            "shell.switcher.folder.newerBody",
            [("found", found.to_string()), ("supported", supported.to_string())],
          )
          .to_string(),
      ),
    ),
    FolderRefusal::Unreadable(reason) => (
      ctx.t("shell.switcher.folder.unreadable"),
      Some(message_text(ctx, reason).to_string()),
    ),
  }
}

/// What the person can do about the `refusal` of a folder (none when the picker failed); another folder
/// always.
fn actions(ctx: &mut Ctx, refusal: Option<(&PathBuf, &FolderRefusal)>, picks: &Picks) -> Vec<Element> {
  let mut buttons: Vec<Element> = Vec::new();
  let mut extra: Option<Element> = None;
  match refusal.map(|(_, refusal)| refusal) {
    Some(FolderRefusal::NotRepository) => {
      extra = Some(
        CodeBox::new(
          "switcher-folder-command",
          ctx.t("shell.switcher.folder.command"),
          "git init",
        )
        .render(ctx),
      );
    }
    Some(FolderRefusal::NotTopFolder { top }) => {
      let (picks, top) = (picks.clone(), top.clone());
      let label = ctx.t_args("shell.switcher.folder.openTop", [("top", top.display().to_string())]);
      buttons.push(
        ActionButton::new("switcher-folder-open-top", ButtonKind::Primary, label)
          .icon(Icon::FolderOpen)
          .in_form()
          .on_click(move || check(&picks.step, &picks.check, top.clone()))
          .into(),
      );
    }
    Some(FolderRefusal::NoProject) => {
      let navigation = Navigation::of(ctx);
      let set_up = ActionButton::new(
        "switcher-folder-set-up",
        ButtonKind::Primary,
        ctx.t("shell.switcher.folder.setUp"),
      )
      .icon(Icon::Plus)
      .in_form();
      buttons.push(match refusal.and_then(|(folder, _)| RouteFolder::new(folder)) {
        Some(folder) => set_up
          .on_click(move || navigation.go(Route::AddProject(Some(folder.clone()))))
          .into(),
        None => {
          let reason = ctx.t("shell.switcher.folder.setUpUnavailable");
          with_reason(
            set_up.unavailable(reason.clone()),
            "switcher-folder-set-up-reason",
            &reason,
          )
          .into()
        }
      });
    }
    Some(FolderRefusal::Newer { .. } | FolderRefusal::Unreadable(_)) | None => {}
  }
  let another = picks.clone();
  buttons.push(
    ActionButton::new(
      "switcher-folder-another",
      ButtonKind::Secondary,
      ctx.t("shell.switcher.folder.another"),
    )
    .in_form()
    .on_click(move || another.pick())
    .into(),
  );
  let row = Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .with_children(buttons);
  extra.into_iter().chain(std::iter::once(row.into())).collect()
}
