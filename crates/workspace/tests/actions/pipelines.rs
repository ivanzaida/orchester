//! A team task's pipeline through the dispatcher (`docs/architecture/server.md` row 80, the person's decision of
//! 2026-11-05): a new task and an edit name one of the team's pipelines by its resource id; a person's own local
//! pipeline is refused before anything is sent, and limits still wait for D6.

use orchester_api::v2::DecisionId;
use orchester_core::{
  id::ResourceId,
  resource::{Pipeline, Slug},
  store::ResourceStore,
  task::{TaskLimits, organization::TaskOrganization},
  time::Timestamp,
  tokens::TokenBudget,
};
use orchester_store::ProjectDb;
use orchester_workspace::actions::{Action, ActionError, Intent, PipelineRefusal, Refusal, TaskAction, TeamOutcome};

use crate::{
  Checkout, completed_task, details, name, revision,
  server::{ADMIN, TeamServer},
  team,
};

/// A pipeline `slug` published here, this computer's own.
fn pipeline(db: &mut ProjectDb, slug: &str) -> ResourceId {
  let now = Timestamp::now();
  let body = crate::fixture::one_step(ResourceId::generate(), "Sign in.");
  let id = db
    .create_resource(Slug::new(slug).expect("slug"), name(slug), body.body(), now)
    .expect("a pipeline")
    .id();
  db.publish_draft::<Pipeline>(id, now).expect("published");
  id
}

/// A pipeline of the team's, `slug`, with its id and its body as the server keeps it.
fn team_pipeline(slug: &str) -> (ResourceId, &str, serde_json::Value) {
  let body = crate::fixture::one_step(ResourceId::generate(), "Sign in.");
  let value = serde_json::to_value(body.body()).expect("a pipeline serializes");
  (ResourceId::generate(), slug, value)
}

fn create_with(pipeline: Option<ResourceId>, limits: TaskLimits) -> Action {
  let mut details = details("Sign in");
  details.pipeline = pipeline;
  details.limits = limits;
  Action::Task(TaskAction::Create {
    details,
    organization: TaskOrganization::default(),
  })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_team_task_names_a_team_pipeline_by_its_id() {
  let server = TeamServer::start();
  let checkout = Checkout::linked(&server);
  let (delivery, review) = (team_pipeline("delivery"), team_pipeline("review"));
  checkout.read_team(&[delivery.clone(), review.clone()]);
  let (delivery, review) = (delivery.0, review.0);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);

  let intent = Intent::new(create_with(Some(delivery), TaskLimits::NONE));
  let task = completed_task(team(dispatcher.dispatch(&intent).await.expect("dispatched")));
  assert_eq!(server.task(task).pipeline, Some(delivery.to_string()), "sent by its id");

  let (mut base, mut edited) = (details("Sign in"), details("Sign in"));
  base.pipeline = Some(delivery);
  edited.pipeline = Some(review);
  let edit = Action::Task(TaskAction::Edit {
    task,
    read: revision(1),
    base,
    edited,
  });
  team(dispatcher.dispatch(&Intent::new(edit)).await.expect("dispatched"));
  assert_eq!(server.task(task).pipeline, Some(review.to_string()));
  assert_eq!(server.writes(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_a_local_pipeline_or_limits_are_refused_before_anything_is_sent() {
  let server = TeamServer::start();
  let checkout = Checkout::local();
  let own = pipeline(&mut checkout.db(), "own-delivery");
  let checkout = checkout.link(&server);
  checkout.read_team(&[team_pipeline("delivery")]);
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);

  let refused = dispatcher
    .dispatch(&Intent::new(create_with(Some(own), TaskLimits::NONE)))
    .await;
  assert!(
    matches!(refused, Err(ActionError::NotTeamPipeline(pipeline)) if pipeline == own),
    "{refused:?}"
  );
  let unknown = ResourceId::generate();
  let refused = dispatcher
    .dispatch(&Intent::new(create_with(Some(unknown), TaskLimits::NONE)))
    .await;
  assert!(
    matches!(refused, Err(ActionError::NotTeamPipeline(pipeline)) if pipeline == unknown),
    "{refused:?}"
  );
  let budget = TokenBudget::new(1000).expect("a budget");
  let limits = TaskLimits::new(Some(budget), None).expect("limits");
  let refused = dispatcher.dispatch(&Intent::new(create_with(None, limits))).await;
  assert!(
    matches!(refused, Err(ActionError::DecisionRequired(DecisionId::D6))),
    "{refused:?}"
  );

  let task = server.add_task("Sign in");
  let mut edited = details("Sign in");
  edited.pipeline = Some(own);
  let edit = Action::Task(TaskAction::Edit {
    task: task.id,
    read: revision(1),
    base: details("Sign in"),
    edited,
  });
  let refused = dispatcher.dispatch(&Intent::new(edit)).await;
  assert!(matches!(refused, Err(ActionError::NotTeamPipeline(_))), "{refused:?}");
  assert_eq!(server.counts(), (0, 0), "nothing reached the server");
  assert_eq!(server.writes(), 0);
}

/// The server refuses a move to an archived team pipeline `422 invalid` with its reason, which the refusal names.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn actions_the_servers_pipeline_refusal_is_named() {
  let server = TeamServer::start();
  let checkout = Checkout::linked(&server);
  let archived = team_pipeline("delivery");
  checkout.read_team(std::slice::from_ref(&archived));
  let archived = archived.0;
  let (dispatcher, _generation) = checkout.team(&server, ADMIN);
  server.archive_pipeline(&archived.to_string());
  let task = server.add_task("Sign in");
  let mut edited = details("Sign in");
  edited.pipeline = Some(archived);
  let edit = Action::Task(TaskAction::Edit {
    task: task.id,
    read: revision(1),
    base: details("Sign in"),
    edited,
  });
  let refused = team(dispatcher.dispatch(&Intent::new(edit)).await.expect("dispatched"));
  assert!(
    matches!(
      refused,
      TeamOutcome::Rejected {
        refusal: Refusal::TaskPipeline(PipelineRefusal::Archived),
        ..
      }
    ),
    "{refused:?}"
  );
  assert_eq!(server.writes(), 0);
}
