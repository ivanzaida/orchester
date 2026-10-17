//! View state of the Skills page, carried by its route.
use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::id::{ProjectId, ResourceId};

use crate::{
  navigation::{Navigation, link::Open},
  project::own::OwnChanges,
  routes::{ProjectPage, Route},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
  Library,
  New,
  Edit(ResourceId),
  Import,
}
impl Panel {
  fn page(self) -> ProjectPage {
    match self {
      Self::Library => ProjectPage::Skills,
      Self::New => ProjectPage::NewSkill,
      Self::Edit(id) => ProjectPage::Skill(id),
      Self::Import => ProjectPage::ImportSkills,
    }
  }
  pub fn of(page: &ProjectPage) -> Self {
    match page {
      ProjectPage::NewSkill => Self::New,
      ProjectPage::Skill(id) => Self::Edit(*id),
      ProjectPage::ImportSkills => Self::Import,
      _ => Self::Library,
    }
  }
}
#[derive(Clone)]
pub struct PanelNav {
  pub project: ProjectId,
  pub navigation: Navigation,
}
impl PanelNav {
  pub fn get(&self) -> Panel {
    match self.navigation.route_now() {
      Route::Project(_, page) => Panel::of(&page),
      _ => Panel::Library,
    }
  }
  pub fn set(&self, panel: Panel) {
    self.navigation.go(Route::Project(self.project, panel.page()));
  }

  /// What opens `panel`, a place of its own, where a press asks (`navigation::link`).
  pub fn to(&self, panel: Panel) -> Open {
    self.navigation.to(Route::Project(self.project, panel.page()))
  }
}
#[derive(Clone)]
pub struct View {
  pub panel: PanelNav,
  pub revision: Signal<u64>,
  /// The skill changes, the screen's own too.
  pub skills: OwnChanges,
  pub reset: Signal<u64>,
  pub reload_requested: Signal<bool>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
  All,
  Project,
  Roles,
  Pipelines,
}
impl DevtoolsInspectable for Filter {}
