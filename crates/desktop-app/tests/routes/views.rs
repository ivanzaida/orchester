//! The tabs, filters, items and panels a screen keeps in its route, so each is an entry of a tab's history:
//! Secrets, the Skills library, an agent profile, Instructions, a pipeline's editor and Project settings.

use std::num::NonZeroU32;

use orchester_app::routes::{
  AgentTab, InstructionsPanel, InstructionsTab, InstructionsView, PipelinePanel, ProjectPage, ProjectSettingsTab,
  Route, SecretsPanel, SecretsTab, SecretsView, SkillsFilter,
};
use orchester_core::{
  resource::{StepKey, VersionNumber},
  secret::Environment,
};

use super::{CONNECTION, PROJECT, at};

const PANELS: [InstructionsPanel; 4] = [
  InstructionsPanel::Hidden,
  InstructionsPanel::Variables,
  InstructionsPanel::Preview,
  InstructionsPanel::Full,
];

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
    for panel in PANELS {
      all.push(at(ProjectPage::Instructions(InstructionsView::Rule(tab, None, panel))));
      all.push(at(ProjectPage::Instructions(InstructionsView::Rule(
        tab,
        Some(id),
        panel,
      ))));
      all.push(at(ProjectPage::Instructions(InstructionsView::New(tab, panel))));
    }
  }
  all.push(at(ProjectPage::Instructions(InstructionsView::Import)));
  for panel in [
    PipelinePanel::Pipeline,
    PipelinePanel::Step(StepKey::new("review").expect("a step key")),
    PipelinePanel::Settings,
    PipelinePanel::History,
    PipelinePanel::Publish,
  ] {
    if panel != PipelinePanel::Publish {
      all.push(at(ProjectPage::PipelineVersion(
        id,
        VersionNumber::FIRST,
        panel.clone(),
      )));
    }
    all.push(at(ProjectPage::Pipeline(id, panel)));
  }
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
      at(ProjectPage::Instructions(InstructionsView::rule(
        InstructionsTab::Roles,
        Some(id.parse().expect("an id")),
      ))),
      format!("{root}/instructions?tab=roles&rule={id}"),
    ),
    (
      at(ProjectPage::Instructions(InstructionsView::Rule(
        InstructionsTab::Roles,
        None,
        InstructionsPanel::Preview,
      ))),
      format!("{root}/instructions?tab=roles&panel=preview"),
    ),
    (
      at(ProjectPage::Instructions(InstructionsView::new_rule(
        InstructionsTab::Steps,
      ))),
      format!("{root}/instructions?tab=steps&rule=new"),
    ),
    (
      at(ProjectPage::Instructions(
        InstructionsView::default().with_panel(InstructionsPanel::Full),
      )),
      format!("{root}/instructions?panel=prompt"),
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
fn pipeline_panels_have_their_documented_paths() {
  let pipeline = format!("/project/{PROJECT}/pipelines/{CONNECTION}");
  let at_panel = |panel| at(ProjectPage::Pipeline(CONNECTION.parse().expect("an id"), panel));
  let step = PipelinePanel::Step(StepKey::new("review").expect("a step key"));
  for (panel, path) in [
    (PipelinePanel::Pipeline, pipeline.clone()),
    (step, format!("{pipeline}?step=review")),
    (PipelinePanel::Settings, format!("{pipeline}?panel=settings")),
    (PipelinePanel::History, format!("{pipeline}?panel=history")),
    (PipelinePanel::Publish, format!("{pipeline}?panel=publish")),
  ] {
    assert_eq!(at_panel(panel).path(), path);
  }
  let viewed = |panel| {
    at(ProjectPage::PipelineVersion(
      CONNECTION.parse().expect("an id"),
      VersionNumber::new(NonZeroU32::new(2).expect("two")),
      panel,
    ))
  };
  for (panel, path) in [
    (PipelinePanel::Pipeline, format!("{pipeline}?version=2")),
    (
      PipelinePanel::Step(StepKey::new("review").expect("a step key")),
      format!("{pipeline}?version=2&step=review"),
    ),
    (PipelinePanel::History, format!("{pipeline}?version=2&panel=history")),
  ] {
    assert_eq!(viewed(panel).path(), path);
  }
}

/// A panel named in the query that is the tab's own reads as the rule without it.
#[test]
fn an_instructions_panel_of_the_tab_reads_as_the_rule() {
  let root = format!("/project/{PROJECT}");
  let route = Route::parse(&format!("{root}/instructions?tab=steps&panel=preview")).expect("a route");
  assert_eq!(
    route,
    at(ProjectPage::Instructions(InstructionsView::rule(
      InstructionsTab::Steps,
      None
    )))
  );
  assert_eq!(route.path(), format!("{root}/instructions?tab=steps"));
}

#[test]
fn views_refuse_what_they_do_not_name() {
  let root = format!("/project/{PROJECT}");
  let id = CONNECTION;
  for path in [
    format!("{root}/secrets?panel=open"),
    format!("{root}/secrets?environment=moon"),
    format!("{root}/skills?filter=none"),
    format!("{root}/agents/new?tab=history"),
    format!("{root}/instructions?view=library"),
    format!("{root}/instructions?rule=first"),
    format!("{root}/instructions?panel=sources"),
    format!("{root}/instructions?view=import&panel=preview"),
    format!("{root}/pipelines/{id}?panel=steps"),
    format!("{root}/pipelines/{id}?step=Not%20a%20key"),
    format!("{root}/pipelines/{id}?step=review&panel=history"),
    format!("{root}/pipelines/{id}?version=0&panel=history"),
    format!("{root}/pipelines/{id}?version=latest"),
    format!("{root}/pipelines/{id}?version=1&panel=publish"),
    format!("{root}/settings?tab=memory"),
  ] {
    assert!(Route::parse(&path).is_err(), "{path} names no view");
  }
}
