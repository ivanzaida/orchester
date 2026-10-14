//! The project switcher (`Project switcher` `i0xDK7`) and its menu (`Project switcher menu` `ouqOt` of
//! `V7ngk`): the project the screen shows, by its name, its folder and its initial, or that none is open.
//! A press opens the menu under it: *RECENT*, the five projects used last with the shown one checked
//! (`recent`), then *All projects*, *Open folder…* — a folder that already keeps an Orchester project,
//! chosen in the system's folder picker (`folder`), refused in place when it keeps none (`refusal`,
//! `VrS47`) — and *Add project*. While the project opens, or
//! when no project in the list has its id, it says only that a project is shown; its id is never shown.

mod folder;
mod recent;
mod refusal;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  messages::Message,
  workspace::{OpenProject, Workspace},
};
use parking_lot::Mutex;

pub use self::folder::{FolderRefusal, open_folder};
use self::refusal::{FolderStep, Picks};
use crate::{
  navigation::Navigation,
  project::Projects,
  routes::{OverviewTab, ProjectPage, Route},
  services::Services,
  theme::{ExtraColor, ExtraRadius, ExtraSpacing, IconSize, ShellText},
  ui::{
    icon::{Icon, icon},
    menu::{MenuItem, menu, menu_panel, separator},
    pressable::pressable,
  },
};

/// `Project switcher` height and its `Glyph` tile.
const SWITCHER_HEIGHT: f32 = 40.0;
const GLYPH_SIZE: f32 = 26.0;
/// `Project switcher menu` width.
const MENU_WIDTH: f32 = 320.0;

type PickAction = FutureAction<Arc<str>, Picked, Message>;

/// The folder chosen in the system's folder picker; `None` when it was closed without one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picked(Option<PathBuf>);

impl DevtoolsInspectable for Picked {}
type CheckAction = FutureAction<PathBuf, Checked, Message>;

/// What opening a chosen folder found.
#[derive(Clone, Debug, PartialEq)]
struct Checked {
  folder: PathBuf,
  opened: Result<OpenProject, FolderRefusal>,
}

impl DevtoolsInspectable for Checked {}

#[derive(Clone, PartialEq)]
pub struct SwitcherProps {
  /// The project the screen shows, if any.
  pub project: Option<ProjectId>,
}

impl DevtoolsInspectable for SwitcherProps {}

pub struct Switcher {
  anchor: ElementRef,
  open: Signal<bool>,
  /// Counts the menu's openings: *RECENT* is read again each time.
  openings: Signal<u64>,
  folder: Signal<FolderStep>,
  /// Whether the folder picker's and the reading's ends are followed already; once per switcher.
  followed: Mutex<bool>,
}

impl Component for Switcher {
  type Props = SwitcherProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      anchor: ctx.element_ref(),
      open: ctx.signal(false),
      openings: ctx.signal(0),
      folder: ctx.signal(FolderStep::Idle),
      followed: Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let workspace = Services::of(ctx).workspace;
    let check = check_action(ctx, workspace);
    let pick = pick_action(ctx);
    self.follow(ctx, &pick, &check);
    let picks = Picks {
      pick,
      check,
      step: self.folder.clone(),
      title: ctx.t("shell.switcher.pickTitle"),
    };
    let project = ctx.props::<SwitcherProps>().project;
    let button = self.button(ctx, project);
    let mut switcher = Column::new().width(Dimension::Pct(100.0)).child(button);
    if self.open.get() {
      let panel = self.panel(ctx, project, &picks);
      switcher = switcher.child(menu(self.anchor.clone(), self.open.clone(), panel));
    }
    switcher
  }
}

impl Switcher {
  /// Follows the ends of the folder picker and of a folder's reading, once: a chosen folder is read with
  /// the menu open again under the switcher; a project folder opens; anything else, and a picker or a
  /// reading that failed, is said in the menu. The reason stays until the menu closes or the next folder
  /// is chosen.
  fn follow(&self, ctx: &mut Ctx, pick: &PickAction, check: &CheckAction) {
    let mut followed = self.followed.lock();
    if *followed {
      return;
    }
    *followed = true;
    let navigation = Navigation::of(ctx);
    let (opening, checking, step) = (self.open.clone(), check.clone(), self.folder.clone());
    ctx.watch(&pick.state(), move |state| {
      if state.is_pending() {
        return;
      }
      // A failed run keeps the data of the run before it: only a fulfilled one chose a folder.
      if let (true, Some(reason)) = (state.is_rejected(), &state.error) {
        tracing::error!(?reason, "the folder picker failed");
        step.set(FolderStep::PickFailed(reason.clone()));
        opening.set(true);
      } else if let (true, Some(Picked(Some(chosen)))) = (state.is_fulfilled(), &state.data) {
        opening.set(true);
        refusal::check(&step, &checking, chosen.clone());
      }
    });
    let (opened, step) = (self.open.clone(), self.folder.clone());
    ctx.watch(&check.state(), move |state| {
      if state.is_pending() {
        return;
      }
      // Opening the folder itself went wrong: it is refused as unreadable, never left checking. A failed
      // run keeps the data of the run before it, which must not open that folder again.
      if state.is_rejected() {
        if let (Some(reason), FolderStep::Checking(folder)) = (&state.error, step.get_untracked()) {
          step.set(FolderStep::Refused(folder, FolderRefusal::Unreadable(reason.clone())));
        }
        return;
      }
      let (true, Some(checked)) = (state.is_fulfilled(), &state.data) else {
        return;
      };
      match &checked.opened {
        Ok(project) => {
          step.set(FolderStep::Idle);
          opened.set(false);
          navigation.go(Route::Project(
            project.project.id(),
            ProjectPage::Overview(OverviewTab::default()),
          ));
        }
        Err(refusal) => step.set(FolderStep::Refused(checked.folder.clone(), refusal.clone())),
      }
    });
    let step = self.folder.clone();
    ctx.watch(&self.open, move |open| {
      if !open
        && matches!(
          step.get_untracked(),
          FolderStep::Refused(..) | FolderStep::PickFailed(_)
        )
      {
        step.set(FolderStep::Idle);
      }
    });
  }

  /// The switcher itself: the shown project's initial, name and folder, or that none is open.
  fn button(&self, ctx: &mut Ctx, project: Option<ProjectId>) -> Element {
    let known = project.and_then(|project| Projects::of(ctx).state(project));
    let listed = known.as_ref().and_then(|state| {
      Some((
        state.name()?.as_str().to_owned(),
        state.repo_root()?.display().to_string(),
      ))
    });
    let (name, detail, tile) = match (project, listed) {
      (Some(_), Some((name, folder))) => {
        let tile = Text::new(&initial(&name))
          .variant(ShellText::SwitcherInitial.role())
          .into();
        (
          name,
          Some(Text::new(&folder).variant(ShellText::SwitcherPath.role())),
          tile,
        )
      }
      (Some(_), None) => (ctx.t("shell.sidebar.project").to_string(), None, folders_glyph()),
      (None, _) => (
        ctx.t("shell.sidebar.noProject").to_string(),
        Some(Text::new(&ctx.t("shell.sidebar.chooseProject")).variant(TypographyStyle::Caption)),
        folders_glyph(),
      ),
    };
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Hair.role())
      .child(truncated(Text::new(&name).variant(TypographyStyle::Label)))
      .with_children(detail.map(truncated));
    let (open, openings) = (self.open.clone(), self.openings.clone());
    let button = pressable(move || {
      openings.update(|count| *count += 1);
      open.update(|open| *open = !*open);
    })
    .id("project-switcher")
    .ref_element(self.anchor.clone())
    .describe("state", if project.is_some() { "project" } else { "no-project" })
    .describe("menu", if self.open.get() { "open" } else { "closed" })
    .describe("a11y_name", name)
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SWITCHER_HEIGHT))
    .padding(Padding::horizontal(SpacingSize::Sm).left(ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Inline.role())
    .rounded(ExtraRadius::Tile.role())
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(glyph_tile(tile, PaletteColor::SurfaceRaised))
    .child(text)
    .child(icon(Icon::ChevronsUpDown, IconSize::Base, PaletteColor::TextMuted));
    let button = if self.open.get() {
      button.background(ExtraColor::Hover.role())
    } else {
      button
    };
    button.into()
  }

  /// The menu: *RECENT*, then All projects, Open folder… with what it found, and Add project.
  fn panel(&self, ctx: &mut Ctx, project: Option<ProjectId>, picks: &Picks) -> Element {
    let recent = recent::rows(ctx, self.openings.get(), project, &self.open);
    let navigation = Navigation::of(ctx);
    let all = MenuItem::new("switcher-all-projects", ctx.t("shell.screens.allProjects"), move || {
      navigation.go(Route::AllProjects);
    })
    .icon(Icon::Folders)
    .detail(ctx.t("shell.switcher.allProjectsDetail"))
    .render(ctx, &self.open);
    let pick = picks.clone();
    let open_folder = MenuItem::new("switcher-open-folder", ctx.t("shell.switcher.openFolder"), move || {
      pick.pick();
    })
    .icon(Icon::FolderOpen)
    .detail(ctx.t("shell.switcher.openFolderDetail"))
    .render(ctx, &self.open);
    let found = refusal::block(ctx, &self.folder.get(), picks);
    let navigation = Navigation::of(ctx);
    let add = MenuItem::new("switcher-add-project", ctx.t("shell.switcher.addProject"), move || {
      navigation.go(Route::AddProject(None));
    })
    .icon(Icon::Plus)
    .render(ctx, &self.open);
    menu_panel("project-switcher-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .child(section(&ctx.t("shell.switcher.recent")))
      .with_children(recent)
      .child(separator(ctx))
      .child(all)
      .child(open_folder)
      .with_children(found)
      .child(add)
      .into()
  }
}

/// Opens the system's folder picker titled as asked, off the UI thread; `None` when it was closed
/// without a folder.
fn pick_action(ctx: &mut Ctx) -> PickAction {
  ctx.future_action(|title: Arc<str>| async move {
    background::run(move || Picked(rfd::FileDialog::new().set_title(&*title).pick_folder())).await
  })
}

/// Opens the project in a chosen folder, off the UI thread, or finds why it cannot.
fn check_action(ctx: &mut Ctx, workspace: Workspace) -> CheckAction {
  ctx.future_action(move |folder: PathBuf| {
    let workspace = workspace.clone();
    async move {
      let chosen = folder.clone();
      let opened = background::run(move || open_folder(&workspace, &chosen)).await?;
      Ok(Checked { folder, opened })
    }
  })
}

/// A section's heading in the menu (`Section` `cRNXf`: *RECENT*).
fn section(label: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::horizontal(SpacingSize::Sm)
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Xs),
    )
    .child(Text::new(label).variant(ShellText::MenuSection.role()).nowrap())
}

/// The first letter of `name`, as the tile shows it.
pub(super) fn initial(name: &str) -> String {
  name.chars().next().into_iter().flat_map(char::to_uppercase).collect()
}

/// One line that ends in an ellipsis when it does not fit.
fn truncated(text: Text) -> Text {
  text
    .nowrap()
    .max_width(Dimension::Pct(100.0))
    .text_overflow(TextOverflow::Elipsis)
}

/// The tile's content while no project's name is known.
fn folders_glyph() -> Element {
  icon(Icon::Folders, IconSize::Base, PaletteColor::TextSecondary).into()
}

/// The square before a project's name (`Glyph` `yJEuT`, `oXpyV`).
pub(super) fn glyph_tile(content: Element, fill: PaletteColor) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_SIZE))
    .height(Dimension::Px(GLYPH_SIZE))
    .rounded(RadiusSize::Md)
    .background(fill)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(content)
}
