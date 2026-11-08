//! A team MCP server the team changed while an admin edited it (`orchester_workspace::agents::team_mcp_conflict`, rule
//! 18 of `docs/design/team-configuration.md`, A18–A19): a draft keeps the version it was edited from even after the
//! team published, what the team published since is read with the field-by-field comparison, and *Rebase my draft*
//! carries the draft onto it: the fields only the person changed are kept, the team's are taken, and a field both
//! changed has no answer until it is picked. Nothing is changed when the merge is refused.
//!
//! `cargo test -p orchester-workspace --test team_mcp_conflict team_mcp_conflict_`

use std::{collections::HashMap, fs, num::NonZeroU32, path::Path};

use orchester_core::{
  blob::ContentHash,
  id::{ProjectId, ResourceId, ServerId, UserId},
  name::Name,
  project::{Project, ProjectKey},
  resource::{Slug, TeamMcpServer, VersionNumber},
  sharing::{MemberRole, SharedPerson},
  store::ResourceStore,
  time::Timestamp,
};
use orchester_store::{CachedVersion, ProjectDb, PublishState, SharingLink};
use orchester_workspace::{
  agents::{
    team_mcp_conflict::{RebaseError, rebase},
    team_mcp_edit::{self, Saved},
    team_mcp_merge::{McpField, Pick, Verdict},
  },
  app_mcp::setup::{NewTeamMcpServer, Runs, team_mcp_server},
};
use tempfile::TempDir;

const HASH: &str = "5f1d6b0f0e2b9a8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a5b4c";

fn at(millis: i64) -> Timestamp {
  Timestamp::from_unix_millis(millis)
}

fn number(value: u32) -> VersionNumber {
  VersionNumber::new(NonZeroU32::new(value).expect("a version number"))
}

fn fields(arguments: &[&str], exempt: &[&str]) -> NewTeamMcpServer {
  NewTeamMcpServer {
    name: "figma".to_owned(),
    runs: Runs::Program {
      program: "npx".to_owned(),
      arguments: arguments.iter().map(ToString::to_string).collect(),
      folder: String::new(),
      variables: vec!["FIGMA_TOKEN".to_owned()],
    },
    exempt_tools: exempt.iter().map(ToString::to_string).collect(),
  }
}

fn body(fields: &NewTeamMcpServer) -> TeamMcpServer {
  team_mcp_server(fields, &[]).expect("the fields make a server")
}

fn cached(at_number: u32, fields: &NewTeamMcpServer) -> CachedVersion<TeamMcpServer> {
  let server = body(fields);
  CachedVersion {
    number: number(at_number),
    published_at: at(10 + i64::from(at_number)),
    publisher: SharedPerson {
      id: UserId::generate(),
      name: "olha".to_owned(),
    },
    server_body: serde_json::to_value(&server).expect("a body"),
    hash: ContentHash::new(HASH).expect("hash"),
    references: Vec::new(),
    local_body: server,
    waits_here: false,
  }
}

/// A team project whose owner is the person, with `figma` numbered v1 by the server.
fn team_with_figma(v1: &NewTeamMcpServer) -> (TempDir, ResourceId) {
  let repo = tempfile::tempdir().expect("temporary directory");
  fs::create_dir(repo.path().join(".git")).expect("create .git");
  let project = Project::new(
    ProjectId::generate(),
    ProjectKey::new("KON").expect("key"),
    Name::new("Kontur").expect("name"),
  );
  let mut db = ProjectDb::create(repo.path(), project).expect("project");
  let link = SharingLink {
    server: ServerId::generate(),
    project: ProjectId::generate(),
    key: ProjectKey::new("KON").expect("key"),
    role: Some(MemberRole::Owner),
    joined_at: at(1),
    cursor: 0,
    last_synced_at: at(1),
    removed_at: None,
    address: Some("https://team.example/orchester".to_owned()),
  };
  db.start_sharing(&link, &[], (&[], &HashMap::new())).expect("shared");
  let (figma, first) = db
    .create_published(
      Slug::new("figma").expect("slug"),
      Name::new("figma").expect("name"),
      &body(v1),
      at(2),
    )
    .expect("created");
  db.confirm_publish::<TeamMcpServer>(first.id(), &cached(1, v1))
    .expect("numbered");
  (repo, figma.id())
}

fn edit_of(root: &Path, id: ResourceId) -> team_mcp_edit::McpEdit {
  team_mcp_edit::read(root, id)
    .expect("read")
    .expect("a form can hold it")
}

fn teammate_publishes(root: &Path, id: ResourceId, version: u32, fields: &NewTeamMcpServer) {
  ProjectDb::open(root)
    .expect("open")
    .cache_shared_version::<TeamMcpServer>(id, &cached(version, fields))
    .expect("cached");
}

fn draft_fields(root: &Path, id: ResourceId) -> Option<(NewTeamMcpServer, VersionNumber)> {
  edit_of(root, id).draft.map(|draft| (draft.fields, draft.based_on))
}

#[test]
fn team_mcp_conflict_nothing_is_newer_until_the_team_publishes_after_the_version_the_draft_is_on() {
  let v1 = fields(&["--stdio"], &["get_figma_data"]);
  let (repo, figma) = team_with_figma(&v1);
  let mine = fields(&["--stdio", "--port", "3845"], &["get_figma_data"]);
  assert!(matches!(
    team_mcp_edit::save(repo.path(), figma, &mine, number(1)),
    Ok(Saved::Draft(_))
  ));
  assert!(
    edit_of(repo.path(), figma).newer.is_none(),
    "the draft is on the team's latest"
  );

  teammate_publishes(
    repo.path(),
    figma,
    2,
    &fields(&["--stdio"], &["get_figma_data", "get_images"]),
  );

  let edit = edit_of(repo.path(), figma);
  let newer = edit.newer.expect("v2 is newer than the draft's v1");
  assert_eq!(newer.number, number(2));
  assert_eq!(newer.base_number, number(1));
  assert_eq!(newer.published.expect("who published it").by, "olha");
  assert_eq!(newer.mine, mine);
  let seen: Vec<_> = newer
    .comparison
    .rows
    .iter()
    .map(|row| (row.field, row.verdict))
    .collect();
  assert_eq!(
    seen,
    [
      (McpField::ExemptTools, Verdict::TakesTheirs),
      (McpField::Arguments, Verdict::KeepsMine)
    ]
  );
  assert_eq!(edit.draft.expect("kept").based_on, number(1), "the draft still says v1");
}

#[test]
fn team_mcp_conflict_saving_keeps_the_version_the_form_was_opened_on() {
  let v1 = fields(&["--stdio"], &["get_figma_data"]);
  let (repo, figma) = team_with_figma(&v1);
  teammate_publishes(
    repo.path(),
    figma,
    2,
    &fields(&["--stdio", "--port", "3846"], &["get_figma_data"]),
  );

  // The form was opened on v1: what it saves is edited from v1, not from the v2 that came since.
  let typed = fields(&["--stdio", "--port", "3845"], &["get_figma_data"]);
  assert!(matches!(
    team_mcp_edit::save(repo.path(), figma, &typed, number(1)),
    Ok(Saved::Draft(_))
  ));
  assert_eq!(draft_fields(repo.path(), figma), Some((typed, number(1))));

  // Typing the base back is no change of the person's: no draft is kept.
  assert_eq!(
    team_mcp_edit::save(repo.path(), figma, &v1, number(1)).expect("saved"),
    Saved::Unchanged
  );
  assert!(draft_fields(repo.path(), figma).is_none());
}

#[test]
fn team_mcp_conflict_rebase_keeps_mine_takes_theirs_and_asks_where_both_changed() {
  let v1 = fields(&["--stdio"], &["get_figma_data"]);
  let (repo, figma) = team_with_figma(&v1);
  let mine = fields(&["--stdio", "--port", "3845"], &["get_figma_data", "get_styles"]);
  team_mcp_edit::save(repo.path(), figma, &mine, number(1)).expect("saved");
  let theirs = fields(&["--stdio", "--port", "3846"], &["get_figma_data", "get_images"]);
  teammate_publishes(repo.path(), figma, 2, &theirs);

  // Arguments and the tools that run without asking were both changed: nothing is changed until they are picked.
  assert_eq!(
    rebase(repo.path(), figma, (None, number(2)), &[]),
    Err(RebaseError::Unresolved(vec![
      McpField::Arguments,
      McpField::ExemptTools
    ]))
  );
  assert_eq!(
    draft_fields(repo.path(), figma),
    Some((mine, number(1))),
    "the draft is as it was"
  );

  let based = rebase(
    repo.path(),
    figma,
    (None, number(2)),
    &[(McpField::Arguments, Pick::Theirs), (McpField::ExemptTools, Pick::Mine)],
  )
  .expect("rebased");

  assert_eq!(based.number, number(2));
  let expected = fields(&["--stdio", "--port", "3846"], &["get_figma_data", "get_styles"]);
  assert_eq!(based.fields, expected);
  assert_eq!(
    draft_fields(repo.path(), figma),
    Some((expected, number(2))),
    "the draft is now edited from the team's version"
  );
  assert!(
    edit_of(repo.path(), figma).newer.is_none(),
    "nothing is newer than the rebased draft"
  );
}

#[test]
fn team_mcp_conflict_rebase_without_both_changing_a_field_needs_no_pick() {
  let v1 = fields(&["--stdio"], &["get_figma_data"]);
  let (repo, figma) = team_with_figma(&v1);
  let mine = fields(&["--stdio", "--port", "3845"], &["get_figma_data"]);
  team_mcp_edit::save(repo.path(), figma, &mine, number(1)).expect("saved");
  teammate_publishes(
    repo.path(),
    figma,
    2,
    &fields(&["--stdio"], &["get_figma_data", "get_images"]),
  );

  let based = rebase(repo.path(), figma, (None, number(2)), &[]).expect("no field was changed by both");

  assert_eq!(
    based.fields,
    fields(&["--stdio", "--port", "3845"], &["get_figma_data", "get_images"]),
    "my arguments over their tools: neither side is lost"
  );
}

#[test]
fn team_mcp_conflict_rebase_refuses_a_comparison_the_team_has_moved_past() {
  let v1 = fields(&["--stdio"], &[]);
  let (repo, figma) = team_with_figma(&v1);
  let mine = fields(&["--stdio", "--port", "3845"], &[]);
  team_mcp_edit::save(repo.path(), figma, &mine, number(1)).expect("saved");
  teammate_publishes(repo.path(), figma, 2, &fields(&["--stdio", "--port", "3846"], &[]));
  teammate_publishes(repo.path(), figma, 3, &fields(&["--stdio", "--port", "3847"], &[]));

  assert_eq!(
    rebase(
      repo.path(),
      figma,
      (None, number(2)),
      &[(McpField::Arguments, Pick::Mine)]
    ),
    Err(RebaseError::Changed),
    "the person looked at v2; v3 is the team's now"
  );
  assert_eq!(draft_fields(repo.path(), figma), Some((mine, number(1))));
}

#[test]
fn team_mcp_conflict_rebase_without_anything_newer_is_refused() {
  let v1 = fields(&["--stdio"], &[]);
  let (repo, figma) = team_with_figma(&v1);
  team_mcp_edit::save(repo.path(), figma, &fields(&["--stdio", "--x"], &[]), number(1)).expect("saved");

  assert_eq!(
    rebase(repo.path(), figma, (None, number(1)), &[]),
    Err(RebaseError::NothingNewer)
  );
}

#[test]
fn team_mcp_conflict_a_version_in_the_outbox_that_met_the_team_is_taken_back_by_the_rebase() {
  let v1 = fields(&["--stdio"], &["get_figma_data"]);
  let (repo, figma) = team_with_figma(&v1);
  let mine = fields(&["--stdio", "--port", "3845"], &["get_figma_data"]);
  team_mcp_edit::save(repo.path(), figma, &mine, number(1)).expect("saved");
  let version = {
    let mut db = ProjectDb::open(repo.path()).expect("open");
    let version = db
      .publish_draft::<TeamMcpServer>(figma, at(20))
      .expect("published here")
      .id();
    // The team's v2 arrived, and the server said so when it was sent.
    drop(db);
    teammate_publishes(
      repo.path(),
      figma,
      2,
      &fields(&["--stdio"], &["get_figma_data", "get_images"]),
    );
    let mut db = ProjectDb::open(repo.path()).expect("open");
    db.record_publish_conflict(version, number(2)).expect("recorded");
    version
  };
  let edit = edit_of(repo.path(), figma);
  let newer = edit.newer.expect("the version that met v2 is compared");
  assert_eq!(newer.from_outbox, Some(version));
  assert_eq!(newer.base_number, number(1));
  assert_eq!(newer.mine, mine);

  let based = rebase(repo.path(), figma, (Some(version), number(2)), &[]).expect("rebased");

  assert_eq!(
    based.fields,
    fields(&["--stdio", "--port", "3845"], &["get_figma_data", "get_images"])
  );
  let db = ProjectDb::open(repo.path()).expect("open");
  assert!(
    db.waiting_publishes().expect("outbox").is_empty(),
    "taken back as the draft"
  );
  assert_eq!(
    draft_fields(repo.path(), figma).map(|(_, based_on)| based_on),
    Some(number(2))
  );
  assert!(
    !matches!(
      db.waiting_publishes()
        .expect("outbox")
        .first()
        .map(|waiting| &waiting.state),
      Some(PublishState::Conflict { .. })
    ),
    "no conflict stays in the outbox"
  );
}

#[test]
fn team_mcp_conflict_publishing_a_draft_on_an_older_version_is_stale_and_sends_nothing() {
  let v1 = fields(&["--stdio"], &[]);
  let (repo, figma) = team_with_figma(&v1);
  team_mcp_edit::save(repo.path(), figma, &fields(&["--stdio", "--x"], &[]), number(1)).expect("saved");
  teammate_publishes(repo.path(), figma, 2, &fields(&["--stdio", "--y"], &[]));

  let mut db = ProjectDb::open(repo.path()).expect("open");
  let refused = db.publish_draft::<TeamMcpServer>(figma, at(30));

  assert!(
    matches!(
      refused,
      Err(orchester_store::StoreError::Publish(orchester_core::resource::PublishError::StaleDraft {
        latest: Some(latest), ..
      })) if latest == number(2)
    ),
    "{refused:?}"
  );
  assert!(db.waiting_publishes().expect("outbox").is_empty());
  assert!(draft_fields(repo.path(), figma).is_some(), "the draft is kept");
}
