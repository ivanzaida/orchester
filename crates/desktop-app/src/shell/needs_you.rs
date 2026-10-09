//! What waits for the person in the projects the tabs show, and the title bar's *N need you* pill
//! (`Needs you` `O3eKK`, its states in `n6Nbi` of `eBhFg`). One thing waiting is what the project's
//! *Needs you* counts: a request for a decision, a question, a plan, a run that waits after its retries
//! (`ProjectTasks::needs_you`). The pill counts every project the tabs show and its tooltip splits the
//! count by project; with nothing waiting it stays in place, quiet, at 0. The panel it opens in the design
//! (layer `Needs you` `eBhFg`) comes later: until then it opens the Overview's *Needs you* list of the
//! active tab's project, or of the first project that has something waiting when the active tab shows
//! none.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  layout::Alignment,
  node::{CursorIcon, Element, padding::Padding},
};
use orchester_core::{id::ProjectId, name::Name};
use orchester_workspace::tasks::data::ProjectTasks;

use crate::{
  navigation::{Navigation, tabs::TabSet},
  project::Projects,
  routes::{OverviewTab, ProjectPage, Route},
  theme::{ExtraSpacing, IconSize},
  ui::{
    icon::{Icon, icon},
    pressable::pressable,
    tooltip::Tooltip,
  },
};

/// How many things wait for the person in one project the tabs show.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waiting {
  pub project: ProjectId,
  pub name: Name,
  pub count: usize,
}

/// How many things wait for the person in `project`, once it is open and its tasks are read. Reading it
/// in a render re-renders it when that changes.
pub fn needs_you(projects: &Projects, project: ProjectId) -> Option<usize> {
  projects.state(project)?.open()?;
  projects
    .feed(project)
    .tasks
    .with(|tasks| tasks.ready().map(ProjectTasks::needs_you))
}

/// Each project the tabs show whose tasks are read, in the order of the tabs, and what waits in it.
pub fn waiting(projects: &Projects, tabs: &TabSet) -> Vec<Waiting> {
  let mut all: Vec<Waiting> = Vec::new();
  for project in tabs.tabs().iter().filter_map(|tab| tab.route().project()) {
    if all.iter().any(|waiting| waiting.project == project) {
      continue;
    }
    let (Some(count), Some(name)) = (needs_you(projects, project), projects.name(project)) else {
      continue;
    };
    all.push(Waiting { project, name, count });
  }
  all
}

/// The words of `count` things waiting: *1 needs you*, *3 need you*.
pub fn count_words(ctx: &Ctx, count: usize) -> String {
  let key = if count == 1 {
    "shell.needsYou.one"
  } else {
    "shell.needsYou.other"
  };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}

/// The title bar's pill. Nothing while no project the tabs show has its tasks read.
pub struct NeedsYouPill;

impl Component for NeedsYouPill {
  type Props = ();

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let navigation = Navigation::of(ctx);
    let tabs = navigation.tabs();
    let all = waiting(&Projects::of(ctx), &tabs);
    let Some(target) = target(&all, tabs.active().route().project()) else {
      return Element::new();
    };
    let total: usize = all.iter().map(|waiting| waiting.count).sum();
    let split = per_project(ctx, &all);
    let color = if total > 0 {
      PaletteColor::Warning
    } else {
      PaletteColor::TextMuted
    };
    let pill = pressable(move || navigation.go(target.clone()))
      .id("needs-you-pill")
      .describe("count", total.to_string())
      .describe("a11y_name", count_words(ctx, total))
      .cursor(CursorIcon::Pointer)
      .padding(Padding::horizontal(SpacingSize::Sm))
      .spacing(ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .rounded(RadiusSize::Md)
      .child(icon(Icon::Hand, IconSize::Md, color.clone()))
      .child(
        Text::new(&count_words(ctx, total))
          .variant(TypographyStyle::Label)
          .color(color)
          .nowrap(),
      );
    // Nothing waiting: no amber, a quiet outline (`UThn1`).
    let pill = if total > 0 {
      pill.background(PaletteColor::WarningMuted)
    } else {
      pill.border_inside(BorderSize::Sm, PaletteColor::Border)
    };
    let pill = Tooltip::new("needs-you-pill", split).wrap(ctx, pill);
    // `Trailing` (`AqLkT`): 8 between the pill and the window controls.
    Row::new()
      .padding(Padding::new().right(SpacingSize::Sm))
      .align_items(Alignment::Center)
      .child(pill)
      .into()
  }
}

/// The tooltip's split by project: *Kontur 5 · Atlas 2*.
fn per_project(ctx: &Ctx, all: &[Waiting]) -> String {
  let separator = ctx.t("shell.needsYou.separator");
  all
    .iter()
    .map(|waiting| {
      ctx
        .t_args(
          "shell.needsYou.inProject",
          [
            ("project", waiting.name.as_str().to_owned()),
            ("count", waiting.count.to_string()),
          ],
        )
        .to_string()
    })
    .collect::<Vec<_>>()
    .join(&separator)
}

/// Where the pill leads: the Overview's *Needs you* of the active tab's project when it is among `all`,
/// else of the first project with something waiting, else of the first project.
fn target(all: &[Waiting], active: Option<ProjectId>) -> Option<Route> {
  let chosen = all
    .iter()
    .find(|waiting| Some(waiting.project) == active)
    .or_else(|| all.iter().find(|waiting| waiting.count > 0))
    .or_else(|| all.first())?;
  Some(Route::Project(
    chosen.project,
    ProjectPage::Overview(OverviewTab::NeedsYou),
  ))
}
