//! The tabs, filters, items and panels a screen keeps in its route, so each is an entry of a tab's history:
//! Secrets, the Skills library, an agent profile, Instructions, a pipeline's editor, Project settings and a pull
//! request's Agent feedback.

use std::num::NonZeroU32;

use orchester_app::routes::{
  AgentTab, HubFilter, HubView, InstructionsPanel, InstructionsTab, InstructionsView, PipelineView, PipelinesFilter,
  PipelinesView, ProjectPage, ProjectSettingsTab, Route, SecretsPanel, SecretsView, SkillsFilter, StepPage,
};
use orchester_core::{
  id::ConnectionId,
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
  let server: ConnectionId = CONNECTION.parse().expect("a connection id");
  let mut all = Vec::new();
  for panel in [
    SecretsPanel::Inspect,
    SecretsPanel::Add,
    SecretsPanel::Replace,
    SecretsPanel::Archive,
  ] {
    for environment in [None, Some(Environment::Staging)] {
      for chosen in [None, Some(secret)] {
        all.push(at(ProjectPage::Secrets(SecretsView {
          environment,
          secret: chosen,
          panel,
        })));
      }
    }
  }
  all.extend(SkillsFilter::ALL.map(|filter| at(ProjectPage::Skills(filter))));
  all.extend(HubFilter::ALL.map(|filter| at(ProjectPage::hub(HubView::new(filter, None)))));
  all.push(at(ProjectPage::hub(HubView::on_server(HubFilter::McpServers, server))));
  all.push(at(ProjectPage::hub(HubView::on_server(HubFilter::Archived, server))));
  all.push(at(ProjectPage::hub(HubView::new(HubFilter::All, Some(id)))));
  all.push(at(ProjectPage::hub(HubView::new(HubFilter::Tools, Some(id)))));
  all.extend(PipelinesFilter::ALL.map(|filter| at(ProjectPage::pipelines(PipelinesView::new(filter, None)))));
  all.push(at(ProjectPage::pipelines(PipelinesView::new(
    PipelinesFilter::Archived,
    Some(id),
  ))));
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
  let review = StepKey::new("review").expect("a step key");
  for view in [
    PipelineView::Steps,
    PipelineView::Step(review.clone()),
    PipelineView::StepPage(review.clone(), StepPage::Permissions),
    PipelineView::StepPage(review.clone(), StepPage::Sources),
    PipelineView::StepPage(review, StepPage::Instructions),
    PipelineView::Deliver,
    PipelineView::Settings,
    PipelineView::Versions,
    PipelineView::Publish,
  ] {
    if view != PipelineView::Publish {
      all.push(at(ProjectPage::PipelineVersion(id, VersionNumber::FIRST, view.clone())));
    }
    all.push(at(ProjectPage::Pipeline(id, view)));
  }
  all.push(at(ProjectPage::CopyPipeline(id)));
  all.extend(ProjectSettingsTab::ALL.map(|tab| at(ProjectPage::Settings(tab))));
  all.push(at(ProjectPage::MakeLocal));
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
      })),
      format!("{root}/secrets?environment=production&secret={id}&panel=replace"),
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
  ];
  for (route, path) in paths {
    assert_eq!(route.path(), path);
  }
}

#[test]
fn project_settings_questions_have_their_documented_paths() {
  let settings = format!("/project/{PROJECT}/settings");
  for (route, path) in [
    (
      at(ProjectPage::Settings(ProjectSettingsTab::Context)),
      format!("{settings}?tab=context"),
    ),
    (
      at(ProjectPage::MakeLocal),
      format!("{settings}?tab=sharing&ask=make-local"),
    ),
  ] {
    assert_eq!(route.path(), path);
  }
}

#[test]
fn pipeline_tabs_and_step_pages_have_their_documented_paths() {
  let pipeline = format!("/project/{PROJECT}/pipelines/{CONNECTION}");
  let id = CONNECTION.parse().expect("an id");
  let at_view = |view| at(ProjectPage::Pipeline(id, view));
  let review = || StepKey::new("review").expect("a step key");
  for (view, path) in [
    (PipelineView::Steps, pipeline.clone()),
    (PipelineView::Step(review()), format!("{pipeline}?step=review")),
    (
      PipelineView::StepPage(review(), StepPage::Permissions),
      format!("{pipeline}?step=review&page=permissions"),
    ),
    (PipelineView::Deliver, format!("{pipeline}?panel=deliver")),
    (PipelineView::Settings, format!("{pipeline}?panel=settings")),
    (PipelineView::Versions, format!("{pipeline}?panel=versions")),
    (PipelineView::Publish, format!("{pipeline}?panel=publish")),
  ] {
    assert_eq!(at_view(view).path(), path);
  }
  let viewed = |view| {
    at(ProjectPage::PipelineVersion(
      id,
      VersionNumber::new(NonZeroU32::new(2).expect("two")),
      view,
    ))
  };
  for (view, path) in [
    (PipelineView::Steps, format!("{pipeline}?version=2")),
    (
      PipelineView::Step(review()),
      format!("{pipeline}?version=2&step=review"),
    ),
    (PipelineView::Versions, format!("{pipeline}?version=2&panel=versions")),
  ] {
    assert_eq!(viewed(view).path(), path);
  }
  assert_eq!(
    at(ProjectPage::CopyPipeline(id)).path(),
    format!("/project/{PROJECT}/pipelines/new?start=copy&from={CONNECTION}")
  );
}

/// Links made before the tabs, which named the history panel, still open Versions.
#[test]
fn a_pipelines_history_link_opens_its_versions_tab() {
  let pipeline = format!("/project/{PROJECT}/pipelines/{CONNECTION}");
  let id = CONNECTION.parse().expect("an id");
  assert_eq!(
    Route::parse(&format!("{pipeline}?panel=history")).expect("a route"),
    at(ProjectPage::Pipeline(id, PipelineView::Versions))
  );
  assert_eq!(
    Route::parse(&format!("{pipeline}?version=2&panel=history")).expect("a route"),
    at(ProjectPage::PipelineVersion(
      id,
      VersionNumber::new(NonZeroU32::new(2).expect("two")),
      PipelineView::Versions
    ))
  );
  assert!(
    Route::parse(&format!("{pipeline}?page=sources")).is_err(),
    "a page names its step"
  );
  assert!(Route::parse(&format!("{pipeline}?step=review&page=nothing")).is_err());
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
    format!("{root}/settings?ask=make-local"),
    format!("{root}/settings?tab=limits&ask=make-local"),
    format!("{root}/settings?tab=sharing&ask=leave"),
  ] {
    assert!(Route::parse(&path).is_err(), "{path} names no view");
  }
}

/// The Agents hub of a team project keeps its filter and the resource its panel shows; opened on its first filter with
/// nothing chosen it is the Agents route itself.
#[test]
fn views_agents_hub_names_its_filter_and_resource() {
  let root = format!("/project/{PROJECT}/agents");
  assert_eq!(ProjectPage::hub(HubView::default()), ProjectPage::Agents);
  assert_eq!(at(ProjectPage::Agents).path(), root);
  let id = CONNECTION.parse().expect("a resource id");
  assert_eq!(
    at(ProjectPage::hub(HubView::new(HubFilter::McpServers, None))).path(),
    format!("{root}?filter=mcp_servers")
  );
  assert_eq!(
    at(ProjectPage::hub(HubView::new(HubFilter::Tools, Some(id)))).path(),
    format!("{root}?filter=tools&item={CONNECTION}")
  );
  assert!(Route::parse(&format!("{root}?filter=pipelines")).is_err());
}

/// The Pipelines hub of a local project keeps its tab and the pipeline its panel shows; opened on its first tab with
/// nothing chosen it is the Pipelines route itself, and a tab it does not have names no view.
#[test]
fn views_pipelines_hub_names_its_tab_and_pipeline() {
  let root = format!("/project/{PROJECT}/pipelines");
  assert_eq!(ProjectPage::pipelines(PipelinesView::default()), ProjectPage::Pipelines);
  assert_eq!(at(ProjectPage::Pipelines).path(), root);
  let id = CONNECTION.parse().expect("a resource id");
  assert_eq!(
    at(ProjectPage::pipelines(PipelinesView::new(
      PipelinesFilter::Archived,
      None
    )))
    .path(),
    format!("{root}?filter=archived")
  );
  let chosen = at(ProjectPage::pipelines(PipelinesView::new(
    PipelinesFilter::Archived,
    Some(id),
  )));
  assert_eq!(chosen.path(), format!("{root}?filter=archived&item={CONNECTION}"));
  assert_eq!(Route::parse(&chosen.path()), Ok(chosen));
  assert_eq!(
    Route::parse(&format!("{root}?item={CONNECTION}")),
    Ok(at(ProjectPage::pipelines(PipelinesView::new(
      PipelinesFilter::All,
      Some(id)
    ))))
  );
  assert!(Route::parse(&format!("{root}?filter=profiles")).is_err());
  assert!(Route::parse(&format!("{root}?item=not-an-id")).is_err());
}

/// The *Drafts* tab is gone with pipeline drafts (D-032): a route kept from before (the history, a link) that names it
/// opens *All*, with the pipeline it named, and its path no longer names a tab.
#[test]
fn views_pipelines_hub_reads_the_gone_drafts_tab_as_all() {
  let root = format!("/project/{PROJECT}/pipelines");
  let id = CONNECTION.parse().expect("a resource id");
  assert_eq!(PipelinesFilter::ALL, [PipelinesFilter::All, PipelinesFilter::Archived]);
  assert_eq!(PipelinesFilter::from_name("drafts"), Some(PipelinesFilter::All));
  let all = at(ProjectPage::pipelines(PipelinesView::new(
    PipelinesFilter::All,
    Some(id),
  )));
  assert_eq!(
    Route::parse(&format!("{root}?filter=drafts&item={CONNECTION}")),
    Ok(all.clone())
  );
  assert_eq!(all.path(), format!("{root}?item={CONNECTION}"));
  assert_eq!(
    Route::parse(&format!("{root}?filter=drafts")),
    Ok(at(ProjectPage::Pipelines))
  );
}

/// A local project's hub also has *Archived*, and names a local MCP server by its connection id, which is not a
/// resource id: the two are different query parameters and neither is read as the other.
#[test]
fn views_agents_hub_of_a_local_project_names_a_server_and_the_archived_filter() {
  let root = format!("/project/{PROJECT}/agents");
  let server: ConnectionId = CONNECTION.parse().expect("a connection id");
  assert_eq!(
    at(ProjectPage::hub(HubView::new(HubFilter::Archived, None))).path(),
    format!("{root}?filter=archived")
  );
  let on_server = at(ProjectPage::hub(HubView::on_server(HubFilter::McpServers, server)));
  assert_eq!(
    on_server.path(),
    format!("{root}?filter=mcp_servers&server={CONNECTION}")
  );
  assert_eq!(Route::parse(&on_server.path()).expect("parsed"), on_server);
  // A form that creates a server names no server.
  let adding = at(ProjectPage::hub(HubView::adding(HubFilter::McpServers)));
  assert_eq!(adding.path(), format!("{root}?filter=mcp_servers&new=1"));
}
