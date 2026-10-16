//! View state of the Skills page, carried by its route.
use lurq::core::Signal;
use orchester_core::id::{ProjectId, ResourceId};

use crate::{
  navigation::Navigation,
  project::own::OwnChanges,
  routes::{ProjectPage, Route, SkillsFilter},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Panel {
  /// The library, with its filter.
  Library(SkillsFilter),
  New,
  Edit(ResourceId),
  Import,
}
impl Panel {
  fn page(self) -> ProjectPage {
    match self {
      Self::Library(filter) => ProjectPage::Skills(filter),
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
      ProjectPage::Skills(filter) => Self::Library(*filter),
      _ => Self::Library(SkillsFilter::default()),
    }
  }
  /// The library as it opens, on every skill.
  pub const LIBRARY: Self = Self::Library(SkillsFilter::All);
}
#[derive(Clone)]
pub struct PanelNav {
  pub project: ProjectId,
  pub navigation: Navigation,
}
impl PanelNav {
  pub fn set(&self, panel: Panel) {
    self.navigation.go(Route::Project(self.project, panel.page()));
  }
  /// *Cancel*: shows `panel`, leaving no entry for the page it closes.
  pub fn close(&self, panel: Panel) {
    self.navigation.close_panel(Route::Project(self.project, panel.page()));
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
