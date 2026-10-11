//! Agents QA states: saved Builder draft, stale Reviewer draft, empty variants, and a repairable read error.
use orchester_core::{
  connection::{AgentProvider, SignIn},
  resource::{AgentProfile, Draft, Pipeline, PromptText, ResourceKind},
  store::ResourceStore,
};

use crate::kontur::{Kontur, SeedError};

pub fn no_connections(kontur: &mut Kontur) -> Result<(), SeedError> {
  // Unlike the task-list's "empty" scenario, this deliberately creates no catalogue.
  if !kontur.db.resources(ResourceKind::AgentProfile)?.is_empty() {
    return Err(SeedError::Invalid(
      "the empty Agents fixture already has profiles".into(),
    ));
  }
  Ok(())
}

pub fn empty(kontur: &mut Kontur) -> Result<(), SeedError> {
  no_connections(kontur)?;
  let bin = kontur.repo.join(".orchester/qa-bin");
  std::fs::create_dir_all(&bin).map_err(|error| SeedError::Invalid(error.to_string()))?;
  let program = bin.join("codex");
  std::fs::write(&program, "").map_err(|error| SeedError::Invalid(error.to_string()))?;
  super::connections::cli(
    kontur,
    ("Codex · Kontur", AgentProvider::CodexCli, program),
    ("0.139.0", SignIn::SignedIn, &["gpt-5.5", "gpt-5.4-mini"]),
    kontur.now,
  )?;
  Ok(())
}

pub fn seed(kontur: &mut Kontur) -> Result<(), SeedError> {
  super::connections::seed(kontur)?;
  for resource in kontur.db.resources(ResourceKind::AgentProfile)? {
    let Some(version) = kontur.db.latest_version::<AgentProfile>(resource.id())? else {
      continue;
    };
    let role = version.body().role().as_str();
    if role != "Builder" && role != "Reviewer" {
      continue;
    }
    let draft = Draft::new(
      changed(version.body(), "Implements the approved plan in the worktree.")?,
      Some(version.number()),
      kontur.ago(2),
    );
    if role == "Reviewer" {
      // Another writer publishes after this draft began.
      kontur.db.save_draft(
        resource.id(),
        &Draft::new(
          changed(version.body(), "Checks the published change against the criteria.")?,
          Some(version.number()),
          kontur.ago(3),
        ),
      )?;
      kontur.db.publish_draft::<AgentProfile>(resource.id(), kontur.ago(3))?;
    }
    kontur.db.save_draft(resource.id(), &draft)?;
  }
  // Usage includes the saved pipeline draft as well as its published version.
  for resource in kontur.db.resources(ResourceKind::Pipeline)? {
    if let Some(version) = kontur.db.latest_version::<Pipeline>(resource.id())? {
      kontur.db.save_draft(
        resource.id(),
        &Draft::new(
          version
            .body()
            .clone()
            .with_description("Draft workflow under review.".into()),
          Some(version.number()),
          kontur.ago(1),
        ),
      )?;
      break;
    }
  }
  super::agents_cases::seed(kontur)
}

fn changed(profile: &AgentProfile, responsibility: &str) -> Result<AgentProfile, SeedError> {
  Ok(
    AgentProfile::new(
      profile.role().clone(),
      responsibility.into(),
      PromptText::new("Implement the approved plan. Keep changes small and leave the worktree ready to review.")
        .map_err(|error| SeedError::Invalid(error.to_string()))?,
      profile.connection(),
      profile.model().clone(),
      profile.settings().clone(),
    )
    .with_permissions(profile.permissions().clone())
    .with_mcp_servers(profile.mcp_servers().clone()),
  )
}

pub fn unreadable(kontur: &mut Kontur) -> Result<(), SeedError> {
  seed(kontur)?;
  let db = rusqlite::Connection::open(kontur.database())?;
  // A QA-only backup makes retry success reproducible without losing the original fixture.
  db.execute(
    "CREATE TABLE qa_agent_draft_backup AS SELECT resource_id, body FROM resource_drafts",
    [],
  )?;
  db.execute("UPDATE resource_drafts SET body = 'broken' WHERE resource_id IN (SELECT id FROM resources WHERE kind = 'agent_profile')", [])?;
  Ok(())
}
