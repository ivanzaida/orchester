//! The sidebar's project selector (`ZcJDx` in `zo9Py`) and the menu it opens (01.06 B `n9vdhp`,
//! menu `IHBfD` in the select menu's style, `u0ByM`): RECENT projects, most recently opened first,
//! with a check at the project shown, then All projects and Add project. Choosing another project
//! opens it in the current tab as a new history entry; Ctrl/Cmd+click or a middle click opens it
//! in a new tab in the background, Ctrl/Cmd+Shift+click in a new tab that becomes active, and so
//! does the hovered row's *Open in new tab* button (`NcJQ0`), for the shown project too. A project
//! that cannot be opened any more leads to All projects, which says why. Like the rest of the
//! sidebar the selector and the menu are pointer-only; Escape or a click outside closes it.

use std::path::PathBuf;

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, Placement},
    events::MouseEvent,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Button, Column, Popup, Rect, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{CursorIcon, Element, HitTestBehavior, Style, border::Border, dimension::Dimension},
};
use orchester_store::RegisteredProject;

use crate::{
  background,
  messages::{self, Message},
  navigation::{Navigation, route::Route},
  tabs::{OpenIn, OpenedProject},
  theme::{ExtraColor, ExtraRadius, ExtraShadow, ExtraSpacing, ExtraTypography},
  ui::{
    field::SELECT_OPTION_HEIGHT,
    icon::{Icon, IconSize, icon},
    link::Link,
    pressable::{pressable, surface},
  },
  workspace::OpenProject,
};

/// Current project block, selector and avatar sizes (`RaOff`, `ZcJDx`, `I01Ke`), the menu's
/// width and its distance under the selector (`IHBfD`), how many recent projects it lists, and the
/// *Open in new tab* button (`NcJQ0`) with where its tooltip sits right of the row (`yRd7u`: 12 past
/// the row's end, 9 below its top).
const CURRENT_PROJECT_HEIGHT: f32 = 70.0;
const SELECTOR_HEIGHT: f32 = 42.0;
const AVATAR_SIZE: f32 = 24.0;
const MENU_WIDTH: f32 = 260.0;
const MENU_GAP: f32 = 6.0;
const RECENT_LIMIT: usize = 5;
const DIVIDER_HEIGHT: f32 = 1.0;
const NEW_TAB_BUTTON: f32 = 20.0;
const TOOLTIP_OFFSET: (f32, f32) = (12.0, 9.0);

type OpenAction = FutureAction<(PathBuf, OpenIn), OpenedProject, Message>;

pub struct ProjectSwitcher {
  open: Signal<bool>,
  /// Counts openings, so each one reads the recent projects again.
  opened: Signal<u64>,
  anchor: ElementRef,
  /// The recent project row under the pointer, which shows *Open in new tab*.
  hovered: Signal<Option<usize>>,
  /// Whether the pointer is over *Open in new tab*, which shows its tooltip beside the row.
  button_hovered: Signal<bool>,
  row_anchor: ElementRef,
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
      if let Some(OpenedProject { project, open }) = state.data.clone() {
        close.set(false);
        navigation.open_project(project, open);
      } else if state.error.is_some() {
        close.set(false);
        navigation.show_all_projects();
      }
    });
    Self {
      open,
      opened: ctx.signal(0),
      anchor: ElementRef::default(),
      hovered: ctx.signal(None),
      button_hovered: ctx.signal(false),
      row_anchor: ElementRef::default(),
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
        .id("project-selector-name")
        .variant(TypographyStyle::Label)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis)
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
    let navigation = navigation(ctx);
    let workspace = navigation.workspace().clone();
    let recent = ctx.future((workspace, self.opened.get()), |(workspace, _)| async move {
      background::run(move || workspace.entries()).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the recent projects");
        messages::store_error(&error)
      })
    });
    let recent = recent.state().get().data.unwrap_or_default();
    let rows: Vec<Element> = recent
      .into_iter()
      .filter(|entry| !entry.folder_missing)
      .take(RECENT_LIMIT)
      .enumerate()
      .map(|(index, project)| self.recent_row(ctx, (index, &project.project), (current, &navigation), action))
      .collect();
    let tabs = navigation.tabs().clone();
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
        Link::new(&tabs, move |open| {
          close_all.set(false);
          all.open(Route::AllProjects, open);
        }),
      ))
      .child(menu_item(
        "switcher-add-project",
        Icon::Plus,
        &ctx.t("projects.add"),
        Link::new(&tabs, move |open| {
          close_add.set(false);
          add.open(Route::Setup, open);
        }),
      ))
      .with_children(self.button_tooltip(ctx))
      .into()
  }

  /// A recent project; the one shown carries the check, and a plain click on it only closes the
  /// menu.
  fn recent_row(
    &self,
    ctx: &Ctx,
    (index, project): (usize, &RegisteredProject),
    (current, navigation): (&OpenProject, &Navigation),
    action: &OpenAction,
  ) -> Element {
    let shown = project.id == current.project.id();
    let check = icon(Icon::Check, IconSize::Dismiss, PaletteColor::TextPrimary)
      .id(format!("switcher-project-check-{index}"))
      .opacity(if shown { 1.0 } else { 0.0 });
    let hovered = self.hovered.get() == Some(index);
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    let mut row = option(surface().id(format!("switcher-project-{index}")))
      .describe("project", project.name.to_string())
      .describe("current", if shown { "true" } else { "false" })
      .on_mouse_enter(move || enter.set(Some(index)))
      .on_mouse_leave(move || {
        if leave.get_untracked() == Some(index) {
          leave.set(None);
        }
      })
      .child(avatar(project.name.as_str()))
      .child(
        Text::new(project.name.as_str())
          .id(format!("switcher-project-name-{index}"))
          .variant(TypographyStyle::FieldLabel)
          .color(PaletteColor::TextPrimary)
          .nowrap()
          .text_overflow(TextOverflow::Elipsis)
          .flex(1.0),
      )
      .with_children(hovered.then(|| self.new_tab_button(ctx, index, action, &project.repo_root)))
      .child(check);
    if hovered {
      row = row.ref_element(self.row_anchor.clone());
    }
    let (action, repo_root, close) = (action.clone(), project.repo_root.clone(), self.open.clone());
    Link::new(navigation.tabs(), move |open| {
      if shown && open == OpenIn::Current {
        close.set(false);
      } else {
        action.run((repo_root.clone(), open));
      }
    })
    .on(row)
    .into()
  }

  /// *Open in new tab* on the hovered row (`NcJQ0`), as a Ctrl/Cmd+click on it.
  fn new_tab_button(&self, ctx: &Ctx, index: usize, action: &OpenAction, repo_root: &std::path::Path) -> Element {
    let (action, repo_root) = (action.clone(), repo_root.to_path_buf());
    let (enter, leave) = (self.button_hovered.clone(), self.button_hovered.clone());
    Row::new()
      .id(format!("switcher-new-tab-{index}"))
      .describe("tooltip", tooltip(ctx))
      .size(Dimension::Px(NEW_TAB_BUTTON), Dimension::Px(NEW_TAB_BUTTON))
      .align_items(Alignment::Center)
      .justify(Justify::Center)
      .rounded(ExtraRadius::RowAction.role())
      .cursor(CursorIcon::Pointer)
      .on_mouse_enter(move || enter.set(true))
      .on_mouse_leave(move || leave.set(false))
      .on_click(move |event: MouseEvent| {
        event.stop_propagation();
        action.run((repo_root.clone(), OpenIn::Background));
      })
      .child(icon(Icon::SquarePlus, IconSize::Dismiss, PaletteColor::TextSecondary))
      .into()
  }

  /// The tooltip of *Open in new tab* (`yRd7u`) right of its row, while the pointer is over it.
  fn button_tooltip(&self, ctx: &Ctx) -> Option<Element> {
    if !self.button_hovered.get() || self.hovered.get().is_none() {
      return None;
    }
    let panel = Column::new()
      .id("switcher-new-tab-tooltip")
      .padding_vertical(ExtraSpacing::Tight.role())
      .padding_horizontal(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(RadiusSize::Md)
      .box_shadow(ExtraShadow::Popover.role())
      .child(
        Text::new(&tooltip(ctx))
          .variant(TypographyStyle::Caption)
          .color(PaletteColor::TextPrimary)
          .nowrap(),
      );
    Some(
      Popup::new(self.row_anchor.clone(), panel)
        .open_when(true)
        .placement(Placement::RightStart)
        .offset(TOOLTIP_OFFSET.0, TOOLTIP_OFFSET.1)
        .hit_test(HitTestBehavior::None)
        .dismiss_on_outside_click(false)
        .dismiss_on_escape(false)
        .into(),
    )
  }
}

/// "Open in new tab" with the platform's modifier.
fn tooltip(ctx: &Ctx) -> std::sync::Arc<str> {
  if cfg!(target_os = "macos") {
    ctx.t("tabs.openInNewTab.cmd")
  } else {
    ctx.t("tabs.openInNewTab.ctrl")
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
fn menu_item(id: &'static str, glyph: Icon, label: &str, link: Link) -> Element {
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
  link.on(row).into()
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

/// Opens a recent project where the click asked; its folder may have gone since it was last
/// opened.
fn open_action(ctx: &mut Ctx) -> OpenAction {
  let workspace = navigation(ctx).workspace().clone();
  ctx.future_action(move |(repo_root, open): (PathBuf, OpenIn)| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.open(&repo_root))
        .await?
        .map(|project| OpenedProject { project, open })
        .map_err(|error| {
          tracing::error!(%error, "cannot open a recent project");
          messages::store_error(&error)
        })
    }
  })
}
