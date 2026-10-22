use orchester_api::{
  ClaimTake, ErrorCode, Heartbeat, MemberChanges, MemberCreate, MemberRole, ProjectCreated, SharedLimits,
  SharingErrorDetails, TagMerge, TagName, TaskChanges, TaskPatch, UploadTask,
};
use orchester_core::{
  id::{IdempotencyKey, TaskId},
  secret::SecretValue,
  sharing::ClaimKind,
  task::{
    TaskStatus,
    organization::{Priority, WorkflowStatus},
  },
};
use serde_json::json;

use crate::support::TestServer;

const REMOTE: &str = "https://github.com/ivanzaida/kontur.git";

fn upload(number: u32, title: &str, claim: bool) -> UploadTask {
  UploadTask {
    id: TaskId::generate(),
    number: Some(number),
    former_key: None,
    title: title.into(),
    brief: "Brief.".into(),
    criteria: vec!["It works.".into()],
    pipeline: Some("delivery".into()),
    limits: Some(SharedLimits {
      budget_tokens: Some(1_400_000),
      max_duration_ms: Some(1_800_000),
    }),
    workflow: WorkflowStatus::Todo,
    priority: Priority::High,
    tag_names: vec!["Desktop".into()],
    blocking: Vec::new(),
    blocked_reason: None,
    module: None,
    status: TaskStatus::Draft,
    created_at: 1,
    claim,
    has_run: claim,
  }
}

fn member(username: &str) -> MemberCreate {
  MemberCreate {
    username: username.into(),
    role: MemberRole::Member,
  }
}

/// The project `kontur`, created by the admin `admin` in the server admin with `tasks` uploaded.
async fn create(server: &TestServer, admin: &SecretValue, tasks: Vec<UploadTask>) -> ProjectCreated {
  let body = json!({"idempotency_key": IdempotencyKey::generate(), "name": "Kontur", "slug": "kontur", "key": "ORC",
    "remote_hint": REMOTE, "tasks": tasks});
  serde_json::from_value(server.post(admin, "/projects", body).await).expect("created project")
}

#[tokio::test]
async fn client_sharing_reads_a_project_with_its_address_and_hides_it_from_others() {
  let server = TestServer::start(|_| {}).await;
  let alice = server.admin().await;
  let (_, bohdan) = server.person(&alice, "bohdan").await;
  let client = server.client();
  let created = create(
    &server,
    &alice,
    vec![
      upload(38, "Repository switcher", true),
      upload(40, "Merge queue", false),
    ],
  )
  .await;
  let project = client.project(&alice, created.project.id).await.expect("project");
  assert_eq!(project.role, Some(MemberRole::Owner));
  assert_eq!(project.next_number, 41);
  assert_eq!(project.slug, "kontur");
  assert_eq!(project.address, format!("{}/kontur", server.url()));
  assert_eq!(
    project.remote_hint.expect("hint").as_str(),
    "github.com/ivanzaida/kontur"
  );
  let numbers: Vec<u32> = created.tasks.iter().map(|task| task.number).collect();
  assert_eq!(numbers, [38, 40]);
  assert_eq!(
    created.tasks[0].claim.as_ref().map(|claim| claim.kind),
    Some(ClaimKind::Run)
  );
  let listed = client.projects(&alice).await.expect("projects");
  assert_eq!(
    listed.iter().map(|project| project.id).collect::<Vec<_>>(),
    [created.project.id]
  );
  assert!(client.projects(&bohdan).await.expect("projects").is_empty());
  let refused = client
    .project(&bohdan, created.project.id)
    .await
    .expect_err("not a member");
  assert_eq!(refused.code(), Some(ErrorCode::NotFound));
  let tags = client.tags(&alice, created.project.id).await.expect("tags");
  assert_eq!(
    tags.iter().map(|tag| tag.name.as_str()).collect::<Vec<_>>(),
    ["Desktop"]
  );
  server.finish().await;
}

#[tokio::test]
async fn client_sharing_members_roles_and_the_last_owner() {
  let server = TestServer::start(|_| {}).await;
  let alice = server.admin().await;
  let (bohdan_id, bohdan) = server.person(&alice, "bohdan").await;
  let client = server.client();
  let project = create(&server, &alice, Vec::new()).await.project.id;
  let missing = client
    .add_member(&alice, project, &member("maria.l"))
    .await
    .expect_err("no such user");
  assert_eq!(missing.code(), Some(ErrorCode::NoSuchUser));
  let added = client
    .add_member(&alice, project, &member("bohdan"))
    .await
    .expect("add");
  assert_eq!((added.user.id, added.role), (bohdan_id, MemberRole::Member));
  let twice = client
    .add_member(&alice, project, &member("bohdan"))
    .await
    .expect_err("already");
  assert_eq!(twice.code(), Some(ErrorCode::AlreadyMember));
  assert_eq!(client.members(&bohdan, project).await.expect("members").len(), 2);
  let alice_id = client.me(&alice).await.expect("me").id;
  let demote = MemberChanges {
    role: MemberRole::Member,
  };
  let last = client
    .change_member(&alice, project, alice_id, &demote)
    .await
    .expect_err("the last owner");
  assert_eq!(last.code(), Some(ErrorCode::LastOwner));
  let promote = MemberChanges {
    role: MemberRole::Owner,
  };
  client
    .change_member(&alice, project, bohdan_id, &promote)
    .await
    .expect("make owner");
  client
    .change_member(&alice, project, alice_id, &demote)
    .await
    .expect("step down");
  client
    .remove_member(&bohdan, project, bohdan_id)
    .await
    .expect_err("last owner");
  client.remove_member(&alice, project, alice_id).await.expect("leave");
  assert_eq!(
    client.project(&alice, project).await.expect_err("gone").code(),
    Some(ErrorCode::NotFound)
  );
  server.finish().await;
}

#[tokio::test]
async fn client_sharing_tasks_change_under_their_revision_and_claims_name_the_holder() {
  let server = TestServer::start(|_| {}).await;
  let alice = server.admin().await;
  let (_, bohdan) = server.person(&alice, "bohdan").await;
  let client = server.client();
  let created = create(&server, &alice, vec![upload(1, "Fix reconnect", false)]).await;
  let project = created.project.id;
  client
    .add_member(&alice, project, &member("bohdan"))
    .await
    .expect("add");
  let task = created.tasks[0].clone();
  let changed = client
    .update_task(
      &alice,
      project,
      task.id,
      &TaskPatch {
        revision: task.revision,
        changes: TaskChanges {
          title: Some("Fix reconnect after sleep".into()),
          ..TaskChanges::default()
        },
      },
    )
    .await
    .expect("edit");
  assert_eq!(changed.revision, task.revision.next().expect("next"));
  let stale = client
    .update_task(
      &bohdan,
      project,
      task.id,
      &TaskPatch {
        revision: task.revision,
        changes: TaskChanges {
          brief: Some("Another brief.".into()),
          ..TaskChanges::default()
        },
      },
    )
    .await
    .expect_err("conflict");
  assert_eq!(stale.code(), Some(ErrorCode::RevisionConflict));
  assert!(matches!(
    stale.details(),
    Some(SharingErrorDetails::Revision { current }) if **current == changed
  ));

  let run = ClaimTake { kind: ClaimKind::Run };
  let claimed = client.claim(&alice, project, task.id, &run).await.expect("claim");
  let holder = claimed.claim.expect("a claim").holder;
  let held = client.claim(&bohdan, project, task.id, &run).await.expect_err("held");
  assert_eq!(held.code(), Some(ErrorCode::ClaimHeld));
  assert!(matches!(
    held.details(),
    Some(SharingErrorDetails::Claim { claim: Some(claim) }) if claim.holder == holder
  ));
  let beat = client
    .heartbeat(&alice, &Heartbeat { tasks: vec![task.id] })
    .await
    .expect("heartbeat");
  assert!(beat.lost.is_empty());
  let lost = client
    .heartbeat(&bohdan, &Heartbeat { tasks: vec![task.id] })
    .await
    .expect("heartbeat");
  assert_eq!(lost.lost.iter().map(|lost| lost.task).collect::<Vec<_>>(), [task.id]);
  let released = client.release_claim(&alice, project, task.id).await.expect("release");
  assert!(released.claim.is_none());
  let listed = client.tasks(&bohdan, project).await.expect("tasks");
  assert_eq!(listed.tasks.len(), 1);
  server.finish().await;
}

#[tokio::test]
async fn client_sharing_tags_are_the_project_s() {
  let server = TestServer::start(|_| {}).await;
  let alice = server.admin().await;
  let client = server.client();
  let created = create(&server, &alice, vec![upload(1, "Fix reconnect", false)]).await;
  let project = created.project.id;
  let desktop = client.tags(&alice, project).await.expect("tags").remove(0);
  let runner = client
    .create_tag(&alice, project, &TagName { name: "Runner".into() })
    .await
    .expect("create");
  let taken = client
    .create_tag(&alice, project, &TagName { name: "runner".into() })
    .await
    .expect_err("taken");
  assert_eq!(taken.code(), Some(ErrorCode::TagExists));
  client
    .rename_tag(&alice, project, runner.id, &TagName { name: "Runtime".into() })
    .await
    .expect("rename");
  let merged = client
    .merge_tag(&alice, project, desktop.id, &TagMerge { into: runner.id })
    .await
    .expect("merge");
  assert_eq!(merged.tags.len(), 1);
  assert_eq!(merged.tasks[0].content.tags, [runner.id]);
  let deleted = client.delete_tag(&alice, project, runner.id).await.expect("delete");
  assert!(deleted.tags.is_empty() && deleted.tasks[0].content.tags.is_empty());
  server.finish().await;
}
