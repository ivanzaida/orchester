//! The `changes` state: a conversation whose agent proposed publishing a pipeline's draft (saved here, its
//! description changed), canceling a run and merging one, each waiting for the person (`docs/design/
//! orchestrator-toolset.md`), after earlier turns whose proposals were applied, refused and dismissed: the cards of
//! publish, cancel and merge in every state. *Publish* publishes the draft; *Cancel run* and *Merge* go to the
//! project's runner and merge job as for any client, so they are refused where the seeded run cannot be canceled or
//! merged.

use std::error::Error;

use orchester_core::{
  id::{ChatId, ConnectionId},
  orchestrator::{
    ActionOutcome, ChatAction, TurnState,
    change::{Change, ProposedChange, Publishing},
  },
  resource::{Draft, Pipeline, ResourceKind},
  run::Run,
  store::{ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::data::{Part, Turn, ago, keep};

/// The conversation, kept in `db` on `connection`.
pub fn conversation(db: &mut ProjectDb, connection: ConnectionId) -> Result<ChatId, Box<dyn Error>> {
  let publish = publication(db)?;
  let (run, key) = a_run(db)?;
  let number = run.number().get();
  let checkpoint = run
    .last_checkpoint()
    .map(|checkpoint| checkpoint.revision().to_string());
  let title = db
    .task(run.task())?
    .map(|task| task.details().title.as_str().to_owned())
    .unwrap_or_default();
  let action = |tool: &str, outcome: ActionOutcome| ChatAction {
    tool: tool.to_owned(),
    task: (tool != "publish_pipeline").then(|| key.clone()),
    outcome,
    held: None,
  };
  let proposed = |change: Change| {
    ActionOutcome::ProposedChange(Box::new(ProposedChange {
      subject: title.clone(),
      change,
    }))
  };
  let cancel = proposed(Change::Cancel { run: run.id(), number });
  let merge = proposed(Change::Merge {
    run: run.id(),
    number,
    checkpoint,
  });
  let refused = ActionOutcome::Refused {
    code: "needs_decision".to_owned(),
    reason: "The main checkout has local changes in the way: decide them on the task's Local integration page."
      .to_owned(),
  };
  let ask = format!("Publish the Delivery draft, cancel run #{number} and merge it.");
  keep(
    db,
    ChatId::generate(),
    connection,
    vec![
      turn(
        "Publish the review fix.",
        ago(40),
        vec![action("publish_pipeline", ActionOutcome::Done)],
      ),
      turn(&format!("Merge {key}."), ago(35), vec![action("merge_run", refused)]),
      turn(
        &format!("Cancel run #{number}."),
        ago(30),
        vec![action("cancel_run", ActionOutcome::Dismissed)],
      ),
      Turn {
        text: &ask,
        at: ago(10),
        parts: vec![
          Part::Reply("Each of these waits for you: apply or dismiss them below."),
          Part::Action(action("publish_pipeline", publish)),
          Part::Action(action("cancel_run", cancel)),
          Part::Action(action("merge_run", merge)),
        ],
        end: TurnState::Answered,
      },
    ],
  )
}

/// An answered turn of `text` at `at` with `actions`.
fn turn(text: &str, at: Timestamp, actions: Vec<ChatAction>) -> Turn<'_> {
  Turn {
    text,
    at,
    parts: actions.into_iter().map(Part::Action).collect(),
    end: TurnState::Answered,
  }
}

/// A draft of the first published pipeline, its description changed, and its publication proposed.
fn publication(db: &mut ProjectDb) -> Result<ActionOutcome, Box<dyn Error>> {
  let resource = db
    .resources(ResourceKind::Pipeline)?
    .into_iter()
    .find(|resource| matches!(db.latest_version::<Pipeline>(resource.id()), Ok(Some(_))))
    .ok_or("the tasks scenario has a published pipeline")?;
  let latest = db
    .latest_version::<Pipeline>(resource.id())?
    .ok_or("the pipeline is published")?;
  let body = latest
    .body()
    .clone()
    .with_description("Delivers a task from research to a reviewed pull request.".to_owned());
  let saved = Timestamp::now();
  db.update_draft::<Pipeline>(resource.id(), |_, _| Ok(Draft::new(body, Some(latest.number()), saved)))?;
  Ok(ActionOutcome::ProposedChange(Box::new(ProposedChange {
    subject: resource.name().as_str().to_owned(),
    change: Change::Publish {
      of: Publishing::Pipeline,
      resource: resource.id(),
      version: latest.number().get() + 1,
      saved,
    },
  })))
}

/// A run of the seed, preferably one still going, and its task's key.
fn a_run(db: &ProjectDb) -> Result<(Run, String), Box<dyn Error>> {
  let mut found = Vec::new();
  for task in db.tasks()? {
    for run in db.runs_of_task(task.id())?.into_iter().flatten() {
      found.push((run, task.key().to_string()));
    }
  }
  let going = found
    .iter()
    .position(|(run, _)| !run.status().is_terminal())
    .unwrap_or(0);
  if found.is_empty() {
    return Err("the tasks scenario has a run".into());
  }
  Ok(found.swap_remove(going))
}
