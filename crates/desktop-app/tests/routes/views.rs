//! The tabs, filters, items and panels a screen keeps in its route, so each is an entry of a tab's history:
//! Secrets, the Skills library, an agent profile, Instructions and Project settings.

use orchester_app::routes::{
  AgentTab, InstructionsTab, InstructionsView, ProjectPage, ProjectSettingsTab, Route, SecretsPanel, SecretsTab,
  SecretsView, SkillsFilter,
};
use orchester_core::secret::Environment;

use super::{CONNECTION, PROJECT, at};

/// Every view of those screens, with each choice they keep.
pub(super) fn view_routes() -> Vec<Route> {
  let id = CONNECTION.parse().expect("a resource id");
  let secret = CONNECTION.parse().expect("a secret id");
  let mut all = Vec::new();
  for tab in [SecretsTab::List, SecretsTab::Usage] {
    for panel in [
      SecretsPanel::Inspect,
      SecretsPanel::Add,
      SecretsPanel::Replace,
      SecretsPanel::Archive,
    ] {
      for environment in [None, Some(Environment::Staging)] {
        for chosen in [None, Some(secret)] {
          all.push(at(ProjectPage::Secrets(SecretsView {
            tab,
            environment,
            secret: chosen,
            panel,
          })));
        }
      }
    }
  }
  all.extend(SkillsFilter::ALL.map(|filter| at(ProjectPage::Skills(filter))));
  all.extend(AgentTab::ALL.map(|tab| at(ProjectPage::Agent(id, tab))));
  all.extend(AgentTab::ALL.map(|tab| at(ProjectPage::NewAgent(tab))));
  for tab in InstructionsTab::ALL {
    all.push(at(ProjectPage::Instructions(InstructionsView::Rule(tab, None))));
    all.push(at(ProjectPage::Instructions(InstructionsView::Rule(tab, Some(id)))));
    all.push(at(ProjectPage::Instructions(InstructionsView::New(tab))));
  }
  all.push(at(ProjectPage::Instructions(InstructionsView::Import)));
  all.extend(ProjectSettingsTab::ALL.map(|tab| at(ProjectPage::Settings(tab))));
  all
}

#[test]
fn views_have_their_documented_paths() {
  let root = format!("/project/{PROJECT}");
  let id = CONNECTION;
  let paths = [
    (
      at(ProjectPage::Secrets(SecretsView {
        environment: Some(Environment::Production),
        secret: Some(CONNECTION.parse().expect("a secret id")),
        panel: SecretsPanel::Replace,
        ..SecretsView::default()
      })),
      format!("{root}/secrets?environment=production&secret={id}&panel=replace"),
    ),
    (
      at(ProjectPage::Secrets(SecretsView {
        tab: SecretsTab::Usage,
        ..SecretsView::default()
      })),
      format!("{root}/secrets?tab=usage"),
    ),
    (at(ProjectPage::Skills(SkillsFilter::All)), format!("{root}/skills")),
    (
      at(ProjectPage::Skills(SkillsFilter::Roles)),
      format!("{root}/skills?filter=roles"),
    ),
    (
      at(ProjectPage::Agent(id.parse().expect("an id"), AgentTab::UsedBy)),
      format!("{root}/agents/{id}?tab=used_by"),
    ),
    (
      at(ProjectPage::NewAgent(AgentTab::Profile)),
      format!("{root}/agents/new"),
    ),
    (
      at(ProjectPage::Instructions(InstructionsView::default())),
      format!("{root}/instructions"),
    ),
    (
      at(ProjectPage::Instructions(InstructionsView::Rule(
        InstructionsTab::Roles,
        Some(id.parse().expect("an id")),
      ))),
      format!("{root}/instructions?tab=roles&rule={id}"),
    ),
    (
      at(ProjectPage::Instructions(InstructionsView::New(InstructionsTab::Steps))),
      format!("{root}/instructions?tab=steps&rule=new"),
    ),
    (
      at(ProjectPage::Instructions(InstructionsView::Import)),
      format!("{root}/instructions?view=import"),
    ),
    (
      at(ProjectPage::Settings(ProjectSettingsTab::CodeIndex)),
      format!("{root}/settings?tab=code_index"),
    ),
  ];
  for (route, path) in paths {
    assert_eq!(route.path(), path);
  }
}

#[test]
fn views_refuse_what_they_do_not_name() {
  let root = format!("/project/{PROJECT}");
  for path in [
    format!("{root}/secrets?panel=open"),
    format!("{root}/secrets?environment=moon"),
    format!("{root}/skills?filter=none"),
    format!("{root}/agents/new?tab=history"),
    format!("{root}/instructions?view=library"),
    format!("{root}/instructions?rule=first"),
    format!("{root}/settings?tab=memory"),
  ] {
    assert!(Route::parse(&path).is_err(), "{path} names no view");
  }
}
