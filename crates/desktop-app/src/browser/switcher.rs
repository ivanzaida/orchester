//! The project switcher (`Project switcher` `u4BK9L`, menu `01b` `f9Z3no`): the shown project's initial and name and a
//! chevron; its menu lists every project, with its tab count and whether an agent is acting there now, the shown one
//! checked, and says that each project browses with its own profile. The button itself shows no other project's
//! activity (`docs/design/browser.md`, "Projects"). Choosing a project shows its tab shown last.

use std::{path::PathBuf, sync::Arc, time::Instant};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text, TextOverflow},
  core::{ElementRef, Signal},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_store::RegisteredProject;
use orchester_workspace::{background, browser::BrowserView};

use super::{link::BrowserLink, model::ACTING_FOR, state::Showing};
use crate::{
  services::Services,
  theme::{BrowserSpacing, BrowserText, ExtraColor, ExtraRadius, ExtraSpacing, IconSize, ShellText},
  ui::{
    icon::{Icon, icon},
    menu::{menu_panel, separator},
    pressable::{in_title_bar, pressable},
  },
};

/// The button (`Project switcher` `u4BK9L`), its tile, a row's tile in the menu, and the menu (`f9Z3no`).
const BUTTON_HEIGHT: f32 = 28.0;
const TILE: f32 = 20.0;
const ROW_TILE: f32 = 24.0;
const MENU_WIDTH: f32 = 300.0;
/// The dot before *acting* in a row (`f9Z3no`).
const ACTING_DOT: f32 = 6.0;

pub struct Switcher {
  open: Signal<bool>,
  anchor: ElementRef,
  /// Counts the menu's openings: the projects are read again at each.
  openings: Signal<u64>,
}

impl Component for Switcher {
  type Props = Showing;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ctx.element_ref(),
      openings: ctx.signal(0),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let showing = ctx.props::<Showing>().clone();
    let (open, openings) = (self.open.clone(), self.openings.clone());
    let button = in_title_bar(pressable(move || {
      openings.update(|count| *count += 1);
      open.update(|open| *open = !*open);
    }))
    .id("browser-project-switcher")
    .ref_element(self.anchor.clone())
    .describe("a11y_name", ctx.t("browser.switcher.label"))
    .describe("project", showing.name.clone())
    .describe("menu", if self.open.get() { "open" } else { "closed" })
    .height(Dimension::Px(BUTTON_HEIGHT))
    .padding(Padding::horizontal(ExtraSpacing::Snug.role()).left(SpacingSize::Xs))
    .spacing(BrowserSpacing::Switcher.role())
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(tile(&showing.name, TILE, BrowserText::SwitcherInitial))
    .child(Text::new(&showing.name).variant(TypographyStyle::Label).nowrap())
    .child(icon(Icon::ChevronDown, IconSize::Sm, PaletteColor::TextMuted));
    let button = if self.open.get() {
      button.background(ExtraColor::Hover.role())
    } else {
      button
    };
    let menu = self.open.get().then(|| self.menu(ctx, &showing));
    let popup = Popup::new(self.anchor.clone(), menu.unwrap_or_else(Element::new))
      .open(self.open.clone())
      .placement(Placement::BottomStart)
      .offset(0.0, ExtraSpacing::MenuOffset.value());
    Row::new().child(button).child(popup)
  }
}

impl Switcher {
  /// The menu: every project of the person's and every project whose browser has tabs, by name.
  fn menu(&self, ctx: &mut Ctx, showing: &Showing) -> Element {
    let services = Services::of(ctx);
    let workspace = services.workspace.clone();
    let registered = ctx
      .future(self.openings.get(), move |_| {
        let workspace = workspace.clone();
        async move {
          background::run(move || workspace.projects().map(Registered).map_err(|error| error.to_string()))
            .await
            .map_err(|_| String::from("the projects' read stopped"))?
        }
      })
      .state()
      .get();
    if let Some(error) = &registered.error {
      tracing::error!(%error, "the Browser window cannot read the projects");
    }
    let browser = services.runners.browser_window();
    let rows = listed(registered.data.map(|read| read.0).unwrap_or_default(), &browser);
    let link = BrowserLink::of(ctx);
    let now = Instant::now();
    let rows: Vec<Element> = rows
      .into_iter()
      .enumerate()
      .map(|(index, (root, name))| {
        let view = browser.view(&root);
        let current = root == *showing.project;
        let choose = (link.clone(), self.open.clone(), root.clone());
        row(ctx, index, (&name, view.as_ref(), current, now), choose)
      })
      .collect();
    menu_panel("browser-project-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::horizontal(SpacingSize::Sm).top(SpacingSize::Sm).bottom(SpacingSize::Xs))
          .child(
            Text::new(&ctx.t("browser.switcher.section"))
              .variant(ShellText::MenuSection.role())
              .nowrap(),
          ),
      )
      .with_children(rows)
      .child(separator(ctx))
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::symmetric(SpacingSize::Sm, SpacingSize::Xs))
          .child(
            Text::new(&ctx.t("browser.switcher.note"))
              .id("browser-project-menu-note")
              .variant(TypographyStyle::Caption)
              .width(Dimension::Pct(100.0)),
          ),
      )
      .into()
  }
}

/// The projects the menu lists, by name: the person's, then those only a runner reported.
fn listed(registered: Vec<RegisteredProject>, browser: &orchester_workspace::browser::BrowserWindow) -> Vec<(PathBuf, Arc<str>)> {
  let mut listed: Vec<(PathBuf, Arc<str>)> = registered
    .into_iter()
    .map(|project| (project.repo_root, Arc::from(project.name.as_str())))
    .collect();
  for (root, view) in browser.projects() {
    if !listed.iter().any(|(known, _)| *known == root) {
      listed.push((root, Arc::from(view.name())));
    }
  }
  listed.sort_by(|left, right| left.1.cmp(&right.1));
  listed
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Registered(Vec<RegisteredProject>);

impl DevtoolsInspectable for Registered {}

/// A project's row: its tile, its name and what its browser holds, a check when it is shown.
fn row(
  ctx: &mut Ctx,
  index: usize,
  (name, view, current, now): (&str, Option<&BrowserView>, bool, Instant),
  (link, open, root): (Option<BrowserLink>, Signal<bool>, PathBuf),
) -> Element {
  let count = view.map_or(0, |view| view.tabs().len());
  let tabs = match count {
    0 => ctx.t("browser.switcher.tabs.none").to_string(),
    1 => ctx.t("browser.switcher.tabs.one").to_string(),
    count => ctx
      .t_args("browser.switcher.tabs.other", [("count", count.to_string())])
      .to_string(),
  };
  let acting = view
    .and_then(BrowserView::latest_acting)
    .is_some_and(|acting| now.saturating_duration_since(acting.at) < ACTING_FOR);
  let detail = Row::new()
    .spacing(SpacingSize::Xs)
    .align_items(Alignment::Center)
    .child(Text::new(&tabs).variant(TypographyStyle::Caption).nowrap())
    .with_children(acting.then(|| {
      Row::new()
        .id(format!("browser-project-{index}-acting"))
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(Text::new("·").variant(TypographyStyle::Caption))
        .child(
          Row::new()
            .width(Dimension::Px(ACTING_DOT))
            .height(Dimension::Px(ACTING_DOT))
            .rounded(ExtraRadius::Full.role())
            .background(PaletteColor::Info),
        )
        .child(
          Text::new(&ctx.t("browser.switcher.acting"))
            .variant(TypographyStyle::Caption)
            .color(PaletteColor::Info),
        )
    }));
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Hair.role())
    .child(
      Text::new(name)
        .variant(TypographyStyle::Label)
        .nowrap()
        .text_overflow(TextOverflow::Elipsis),
    )
    .child(detail);
  let button = pressable(move || {
    open.set(false);
    if let Some(link) = &link {
      link.show(&root, None);
    }
  })
  .id(format!("browser-project-{index}"))
  .describe("a11y_name", name.to_owned())
  .describe("chosen", if current { "true" } else { "false" })
  .width(Dimension::Pct(100.0))
  .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
  .spacing(SpacingSize::Sm)
  .rounded(RadiusSize::Sm)
  .hovered_style(Style::new().background(ExtraColor::Hover.role()))
  .child(tile(name, ROW_TILE, BrowserText::SwitcherInitial))
  .child(text)
  .with_children(current.then(|| icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)));
  button.into()
}

/// A project's square with its initial (`Tile` `AM9OP`).
fn tile(name: &str, side: f32, text: BrowserText) -> Row {
  let initial: String = name.chars().next().into_iter().flat_map(char::to_uppercase).collect();
  Row::new()
    .width(Dimension::Px(side))
    .height(Dimension::Px(side))
    .flex_shrink(0.0)
    .rounded(RadiusSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border_inside(BorderSize::Sm, PaletteColor::Border)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .child(Text::new(&initial).variant(text.role()))
}
