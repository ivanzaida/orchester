//! The sidebar's project selector (`ZcJDx` in `zo9Py`) and the menu it opens (01.06 B `n9vdhp`,
//! menu `IHBfD` in the select menu's style, `u0ByM`): RECENT projects, most recently opened first,
//! with a check at the project shown, then All projects and Add project. Choosing another project
//! opens it as a new history entry; the project shown before keeps its runner in the background.
//! A project that cannot be opened any more leads to All projects, which says why. Like the rest
//! of the sidebar the selector and the menu are pointer-only; Escape or a click outside closes it.

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, Placement},
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Popup, Rect, Row, Text},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension},
};
use orchester_store::RegisteredProject;

use crate::{
  background,
  messages::{self, Message},
  navigation::Navigation,
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::{
    field::SELECT_OPTION_HEIGHT,
    icon::{Icon, IconSize, icon},
    pressable::{pressable, surface},
  },
  workspace::OpenProject,
};

/// Current project block, selector and avatar sizes (`RaOff`, `ZcJDx`, `I01Ke`), the menu's
/// width and its distance under the selector (`IHBfD`), and how many recent projects it lists.
const CURRENT_PROJECT_HEIGHT: f32 = 70.0;
const SELECTOR_HEIGHT: f32 = 42.0;
const AVATAR_SIZE: f32 = 24.0;
const MENU_WIDTH: f32 = 260.0;
const MENU_GAP: f32 = 6.0;
const RECENT_LIMIT: usize = 5;
const DIVIDER_HEIGHT: f32 = 1.0;

type OpenAction = FutureAction<PathBuf, OpenProject, Message>;

pub struct ProjectSwitcher {
  open: Signal<bool>,
  /// Counts openings, so each one reads the recent projects again.
  opened: Signal<u64>,
  anchor: ElementRef,
}

impl Component for ProjectSwitcher {
  type Props = OpenProject;

  fn create(ctx: &mut Ctx) -> Self {
    let action = open_action(ctx);
    let navigation = navigation(ctx);
    let open = ctx.signal(false);
    let close = open.clone();
    ctx.watch(&action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(project) = state.data.clone() {
        close.set(false);
        navigation.show_project(project);
      } else if state.error.is_some() {
        close.set(false);
        navigation.show_all_projects();
      }
    });
    Self {
      open,
      opened: ctx.signal(0),
      anchor: ElementRef::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    // The action first, as `create` asked for it.
    let action = open_action(ctx);
    let current = ctx.props::<OpenProject>().clone();
    let menu = if self.open.get() {
      self.menu(ctx, &current, &action)
    } else {
      Column::new().into()
    };
    let popup = Popup::new(self.anchor.clone(), menu)
      .open(self.open.clone())
      .placement(Placement::BottomStart)
      .offset(0.0, MENU_GAP);
    let (open, opened) = (self.open.clone(), self.opened.clone());
    let selected = self.open.get();
    let mut selector = pressable(surface().id("project-selector"), move || {
      if !open.get_untracked() {
        opened.update(|count| *count += 1);
      }
      open.update(|open| *open = !*open);
    })
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SELECTOR_HEIGHT))
    .padding(ExtraSpacing::Field.role())
    .spacing(ExtraSpacing::Field.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
    .describe("open", if selected { "true" } else { "false" })
    .ref_element(self.anchor.clone())
    .child(avatar(current.project.name().as_str()))
    .child(
      Text::new(current.project.name().as_str())
        .variant(TypographyStyle::Label)
        .nowrap()
        .flex(1.0),
    )
    .child(icon(Icon::ChevronsUpDown, IconSize::Small, PaletteColor::TextSecondary).id("project-selector-switch"));
    if selected {
      selector = selector.background(PaletteColor::AccentMuted);
    }
    Column::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(CURRENT_PROJECT_HEIGHT))
      .padding_top(SpacingSize::Sm)
      .padding_horizontal(SpacingSize::Md)
      .child(selector)
      .child(popup)
  }
}

impl ProjectSwitcher {
  /// RECENT, the recent projects, then All projects and Add project (`IHBfD`).
  fn menu(&self, ctx: &mut Ctx, current: &OpenProject, action: &OpenAction) -> Element {
    let workspace = navigation(ctx).workspace().clone();
    let recent = ctx.future((workspace, self.opened.get()), |(workspace, _)| async move {
      background::run(move || workspace.entries()).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the recent projects");
        messages::store_error(&error)
      })
    });
    let recent = recent.state().get().data.unwrap_or_default();
    let navigation = navigation(ctx);
    let rows: Vec<Element> = recent
      .into_iter()
      .filter(|entry| !entry.folder_missing)
      .take(RECENT_LIMIT)
      .enumerate()
      .map(|(index, project)| self.recent_row(index, &project.project, current, action))
      .collect();
    let (all, add, close_all, close_add) = (navigation.clone(), navigation, self.open.clone(), self.open.clone());
    Column::new()
      .id("project-switcher")
      .width(Dimension::Px(MENU_WIDTH))
      .padding(ExtraSpacing::SelectMenu.role())
      .spacing(ExtraSpacing::Hair.role())
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::SelectMenu.role())
      .box_shadow(ExtraShadow::Popover.role())
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding_top(SpacingSize::Sm)
          .padding_horizontal(SpacingSize::Sm)
          .padding_bottom(SpacingSize::Xs)
          .child(Text::new(&ctx.t("projects.switcher.recent")).variant(ExtraTypography::Overline.role())),
      )
      .with_children(rows)
      .child(Rect::new(Dimension::Pct(100.0), Dimension::Px(DIVIDER_HEIGHT)).background(ExtraColor::Hairline.role()))
      .child(menu_item(
        "switcher-all-projects",
        Icon::Folders,
        &ctx.t("nav.allProjects"),
        move || {
          close_all.set(false);
          all.show_all_projects();
        },
      ))
      .child(menu_item(
        "switcher-add-project",
        Icon::Plus,
        &ctx.t("projects.add"),
        move || {
          close_add.set(false);
          add.show_setup();
        },
      ))
      .into()
  }

  /// A recent project; the one shown carries the check and only closes the menu.
  fn recent_row(
    &self,
    index: usize,
    project: &RegisteredProject,
    current: &OpenProject,
    action: &OpenAction,
  ) -> Element {
    let shown = project.id == current.project.id();
    let check = icon(Icon::Check, IconSize::Dismiss, PaletteColor::TextPrimary).opacity(if shown { 1.0 } else { 0.0 });
    let row = option(surface().id(format!("switcher-project-{index}")))
      .describe("project", project.name.to_string())
      .describe("current", if shown { "true" } else { "false" })
      .child(avatar(project.name.as_str()))
      .child(
        Text::new(project.name.as_str())
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap()
          .flex(1.0),
      )
      .child(check);
    if shown {
      let close = self.open.clone();
      return pressable(row, move || close.set(false)).into();
    }
    let (action, repo_root) = (action.clone(), project.repo_root.clone());
    pressable(row, move || action.run(repo_root.clone())).into()
  }
}

/// A row of the menu, as a select option (`L20XvS`) with a leading slot.
fn option(button: Button) -> Button {
  button
    .width(Dimension::Pct(100.0))
    .height(Dimension::Px(SELECT_OPTION_HEIGHT))
    .padding_horizontal(SpacingSize::Sm)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(PaletteColor::AccentMuted))
}

/// All projects or Add project (`FpP02`, `juYE0`): its icon in the avatar's slot.
fn menu_item(id: &'static str, glyph: Icon, label: &str, on_click: impl Fn() + Send + Sync + 'static) -> Element {
  let slot = Row::new()
    .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(icon(glyph, IconSize::Dismiss, PaletteColor::TextSecondary));
  let row = option(surface().id(id)).child(slot).child(
    Text::new(label)
      .variant(TypographyStyle::FieldLabel)
      .color(PaletteColor::TextPrimary)
      .flex(1.0),
  );
  pressable(row, on_click).into()
}

/// The project's initial on its tile (`I01Ke`).
fn avatar(name: &str) -> Element {
  let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
  Row::new()
    .size(Dimension::Px(AVATAR_SIZE), Dimension::Px(AVATAR_SIZE))
    .background(PaletteColor::AccentMuted)
    .rounded(RadiusSize::Sm)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(Text::new(&initial).variant(TypographyStyle::Button))
    .into()
}

fn navigation(ctx: &mut Ctx) -> Navigation {
  ctx
    .use_context::<Navigation>()
    .expect("the root provides Navigation before mounting screens")
}

/// Opens a recent project; its folder may have gone since it was last opened.
fn open_action(ctx: &mut Ctx) -> OpenAction {
  let workspace = navigation(ctx).workspace().clone();
  ctx.future_action(move |repo_root: PathBuf| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.open(&repo_root))
        .await?
        .map_err(|error| {
          tracing::error!(%error, "cannot open a recent project");
          messages::store_error(&error)
        })
    }
  })
}
