//! *RECENT* in the project switcher's menu: the five projects used last, most recent first, read from
//! the workspace each time the menu opens, the shown one checked (`Switcher project row` `NUMiQ`). A row
//! opens its project's Overview in the active tab; under the pointer it offers *Open in new tab*
//! (`Open in new tab` `o4Lzf`).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Text, TextOverflow},
  core::Signal,
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_store::RegisteredProject;
use orchester_workspace::{background, messages};

use super::{glyph_tile, initial};
use crate::{
  navigation::Navigation,
  routes::{OverviewTab, ProjectPage, Route},
  services::Services,
  theme::{ExtraColor, ExtraSpacing, ExtraTypography, IconSize, ShellText},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// How many projects *RECENT* lists.
pub const RECENT_LIMIT: usize = 5;

/// The rows of *RECENT*, read again for each `opening` of the menu; `shown` is checked. Nothing while
/// they are read, or when the list cannot be read (the log says why).
pub fn rows(ctx: &mut Ctx, opening: u64, shown: Option<ProjectId>, open: &Signal<bool>) -> Vec<Element> {
  let workspace = Services::of(ctx).workspace;
  let read = ctx.future(opening, move |_| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.projects()).await?.map_err(|error| {
        tracing::error!(%error, "cannot read the recent projects");
        messages::store_error(&error)
      })
    }
  });
  let projects = read.state().get().data.unwrap_or_default();
  projects
    .into_iter()
    .take(RECENT_LIMIT)
    .map(|project| {
      let props = RecentProps {
        current: Some(project.id) == shown,
        open: open.clone(),
        project,
      };
      ctx.mount_keyed::<RecentRow>(&format!("recent-{}", props.project.id), props)
    })
    .collect()
}

#[derive(Clone)]
struct RecentProps {
  project: RegisteredProject,
  current: bool,
  /// The menu's, which a choice closes.
  open: Signal<bool>,
}

impl PartialEq for RecentProps {
  fn eq(&self, other: &Self) -> bool {
    (&self.project, self.current) == (&other.project, other.current)
  }
}

impl DevtoolsInspectable for RecentProps {}

struct RecentRow {
  hovered: Signal<bool>,
}

impl Component for RecentRow {
  type Props = RecentProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      hovered: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let RecentProps { project, current, open } = ctx.props::<RecentProps>().clone();
    let navigation = Navigation::of(ctx);
    let route = Route::Project(project.id, ProjectPage::Overview(OverviewTab::default()));
    let id = format!("recent-{}", project.id);
    let name = project.name.as_str().to_owned();
    let tile = glyph_tile(
      Text::new(&initial(&name))
        .variant(ShellText::SwitcherInitial.role())
        .color(PaletteColor::TextPrimary)
        .into(),
      PaletteColor::SurfaceRaised,
    );
    let text = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Hair.role())
      .child(one_line(Text::new(&name).variant(ExtraTypography::Strong.role())))
      .child(one_line(
        Text::new(&project.repo_root.display().to_string()).variant(ExtraTypography::MonoNote.role()),
      ));
    let trailing = if self.hovered.get() {
      Some(new_tab(ctx, &id, (navigation.clone(), route.clone()), open.clone()))
    } else {
      current.then(|| {
        icon(Icon::Check, IconSize::Md, PaletteColor::TextPrimary)
          .id(format!("{id}-current"))
          .into()
      })
    };
    let (enter, leave) = (self.hovered.clone(), self.hovered.clone());
    pressable(move || {
      open.set(false);
      navigation.go(route.clone());
    })
    .id(id)
    .describe("current", if current { "true" } else { "false" })
    .describe("a11y_name", name)
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Sm, ExtraSpacing::Snug.role()))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .rounded(RadiusSize::Md)
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .on_mouse_enter(move || enter.set(true))
    .on_mouse_leave(move || leave.set(false))
    .child(tile)
    .child(text)
    .with_children(trailing)
  }
}

/// *Open in new tab*: the project's Overview in a new tab right of the active one.
fn new_tab(ctx: &mut Ctx, row: &str, (navigation, route): (Navigation, Route), open: Signal<bool>) -> Element {
  let id = format!("{row}-new-tab");
  let label = ctx.t("shell.switcher.openInNewTab");
  let button = pressable(move || {
    open.set(false);
    navigation.open_tab();
    navigation.replace(route.clone());
  })
  .id(id.clone())
  .describe("a11y_name", label.clone())
  .width(Dimension::Px(NEW_TAB_SIZE))
  .height(Dimension::Px(NEW_TAB_SIZE))
  .justify(Justify::Center)
  .rounded(RadiusSize::Sm)
  .background(PaletteColor::SurfaceRaised)
  .child(icon(Icon::SquarePlus, IconSize::Md, PaletteColor::TextMuted));
  Tooltip::new(id, label).wrap(ctx, button)
}

/// `Open in new tab` (`o4Lzf`) size.
const NEW_TAB_SIZE: f32 = 20.0;

fn one_line(text: Text) -> Text {
  text
    .nowrap()
    .max_width(Dimension::Pct(100.0))
    .text_overflow(TextOverflow::Elipsis)
}
