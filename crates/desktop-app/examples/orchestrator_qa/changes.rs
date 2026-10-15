//! The `changes` state: a conversation in every state of the cards of `Orchestrator · publish proposal` (`b55NqO`),
//! its states (`UKgrR`) and `Orchestrator · run proposals · states` (`Ee2JE`). Earlier turns hold an instruction
//! published by the person, a publication dismissed, one the chat did itself, a profile draft that widens (proposed
//! while *Let the Orchestrator publish drafts* is on, which the example turns on for this state), a publication
//! refused as changed since, a cancel and a merge applied and dismissed, and a run started in auto mode and one the
//! queue refused; the newest turn waits for the person with an instruction's publication (its draft saved here, its
//! text changed), a cancel and a merge. The cards read their facts from the seed as it is, so a card names the
//! seed's own drafts, runs and their states.

use std::{collections::BTreeSet, error::Error};

use orchester_core::{
  id::{ChatId, ConnectionId, ResourceId},
  name::Name,
  orchestrator::{
    ActionOutcome, ChatAction, TurnState,
    change::{Change, ProposedChange, Publishing},
  },
  permission::{AgentPermissions, CommandPermission, NetworkAccess},
  resource::{
    AgentProfile, AgentTargets, Draft, InjectionEvent, Instruction, InstructionScope, Pipeline, PromptText,
    RepositoryAccess, ResourceBody, ResourceKind, Slug, Version, VersionNumber,
  },
  run::{Run, RunStatus},
  store::{MergeStore, ResourceStore, RunStore, TaskStore},
  time::Timestamp,
};
use orchester_store::ProjectDb;

use super::data::{Part, Turn, ago, keep};

/// The conversation, kept in `db` on `connection`.
pub fn conversation(db: &mut ProjectDb, connection: ConnectionId) -> Result<ChatId, Box<dyn Error>> {
  let runs = Runs::of(db)?;
  let instruction = instruction_draft(db)?;
  let widening = profile_draft(db)?;
  let published = published(db)?;
  let refused = ActionOutcome::Refused {
    code: "changed_since_proposed".to_owned(),
    reason: "The draft of Review checklist changed after this proposal. Nothing was published: ask the Orchestrator \
             again to see the changes you would publish."
      .to_owned(),
  };
  let turns = vec![
    turn(
      "Publish the delivery pipeline.",
      ago(70),
      vec![settled(&published, ActionOutcome::Applied)],
    ),
    turn(
      "Publish it again later.",
      ago(66),
      vec![settled(&published, ActionOutcome::Dismissed)],
    ),
    turn(
      "Publish it now.",
      ago(62),
      vec![change(&published, ActionOutcome::Done)],
    ),
    turn(
      "Publish the Builder profile.",
      ago(58),
      vec![change(&widening, ActionOutcome::Waits)],
    ),
    turn(
      "Publish the review checklist.",
      ago(54),
      vec![settled(&instruction, refused)],
    ),
    turn("Cancel that run.", ago(50), vec![runs.cancel(ActionOutcome::Applied)]),
    turn("Cancel it.", ago(46), vec![runs.cancel(ActionOutcome::Dismissed)]),
    turn(
      "Merge the finished run.",
      ago(42),
      vec![runs.merge(ActionOutcome::Applied, true)],
    ),
    turn("Merge it.", ago(38), vec![runs.merge(ActionOutcome::Dismissed, false)]),
    turn("Start the next task.", ago(34), vec![runs.started(None)]),
    turn(
      "Start the blocked task.",
      ago(30),
      vec![runs.started(Some(
        "The task waits for another task to be merged. A start from the chat follows the queue's rules, so nothing \
       started.",
      ))],
    ),
  ];
  let waiting = vec![
    Part::Reply("I saved the change as a draft. Publishing it changes what agents read, so it waits for your click."),
    Part::Action(Box::new(change(&instruction, ActionOutcome::Waits))),
    Part::Action(Box::new(runs.cancel(ActionOutcome::Waits))),
    Part::Action(Box::new(runs.merge(ActionOutcome::Waits, false))),
  ];
  let mut all = turns;
  all.push(Turn {
    text: "Tighten the review instruction and publish it; cancel the stuck run and merge the finished one.",
    at: ago(10),
    parts: waiting,
    end: TurnState::Answered,
  });
  keep(db, ChatId::generate(), connection, all)
}

/// An answered turn of `text` at `at` with `actions`.
fn turn(text: &str, at: Timestamp, actions: Vec<ChatAction>) -> Turn<'_> {
  Turn {
    text,
    at,
    parts: actions
      .into_iter()
      .map(|action| Part::Action(Box::new(action)))
      .collect(),
    end: TurnState::Answered,
  }
}

/// The action of a change: `tool`'s, about `change`, gone as `outcome`.
fn change((tool, task, change): &(String, Option<String>, ProposedChange), outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    change: Some(change.clone()),
    ..ChatAction::new(tool, task.clone(), outcome)
  }
}

/// [`change`] settled by a person a minute after it was proposed.
fn settled(about: &(String, Option<String>, ProposedChange), outcome: ActionOutcome) -> ChatAction {
  ChatAction {
    settled: Some(Timestamp::now()),
    ..change(about, outcome)
  }
}

/// The publication of `resource`'s draft of kind `of`, as version `version`, saved at `saved`.
fn publication(
  of: Publishing,
  (resource, name): (ResourceId, String),
  (version, saved): (u32, Timestamp),
) -> (String, Option<String>, ProposedChange) {
  (
    of.tool().to_owned(),
    None,
    ProposedChange {
      subject: name,
      change: Change::Publish {
        of,
        resource,
        version,
        saved,
      },
    },
  )
}

/// The seed's first instruction with a published version (else *Review checklist* v1, made here on the first step
/// of the seed's first published pipeline), a draft of it with a line changed and two added, and its publication.
fn instruction_draft(db: &mut ProjectDb) -> Result<(String, Option<String>, ProposedChange), Box<dyn Error>> {
  let (resource, name, latest) = match latest_of::<Instruction>(db, ResourceKind::Instruction) {
    Ok(found) => found,
    Err(_) => review_checklist(db)?,
  };
  let body = latest.body();
  let mut lines: Vec<String> = body.text().as_str().lines().map(str::to_owned).collect();
  lines.pop();
  lines.push("- Every changed module has a test named after it.".to_owned());
  lines.push("- Name a missing test in the review and return the work to Implement.".to_owned());
  let draft = Instruction::new(
    PromptText::new(&lines.join("\n"))?,
    body.targets().clone(),
    body.events().clone(),
    body.scope().clone(),
  )?;
  let saved = save(db, resource, draft, latest.number())?;
  Ok(publication(
    Publishing::Instruction,
    (resource, name),
    (latest.number().get() + 1, saved),
  ))
}

/// *Review checklist* v1, for the first step of the seed's first published pipeline.
fn review_checklist(db: &mut ProjectDb) -> Result<(ResourceId, String, Version<Instruction>), Box<dyn Error>> {
  let (pipeline, _, version) = latest_of::<Pipeline>(db, ResourceKind::Pipeline)?;
  let step = version
    .body()
    .steps()
    .first()
    .ok_or("the pipeline has a step")?
    .key
    .clone();
  let body = Instruction::new(
    PromptText::new(
      "## Tests\n- Run the whole test suite once, at the end.\n- Ask for tests where behaviour changes.",
    )?,
    AgentTargets::All,
    BTreeSet::from([InjectionEvent::Start]),
    InstructionScope::Step { pipeline, step },
  )?;
  let name = "Review checklist";
  let now = Timestamp::now();
  let created = db
    .create_resource(Slug::new("review-checklist")?, Name::new(name)?, &body, now)?
    .id();
  let version = db.publish_draft::<Instruction>(created, now)?;
  Ok((created, name.to_owned(), version))
}

/// The first profile of the seed with a published version, a draft of it with network access on, and its
/// publication: it widens what the profile's agents may do.
fn profile_draft(db: &mut ProjectDb) -> Result<(String, Option<String>, ProposedChange), Box<dyn Error>> {
  let (resource, name, latest) = latest_of::<AgentProfile>(db, ResourceKind::AgentProfile)?;
  let draft = latest.body().clone().with_permissions(AgentPermissions::new(
    RepositoryAccess::ReadOnly,
    CommandPermission::AskBefore,
    NetworkAccess::On,
  ));
  let saved = save(db, resource, draft, latest.number())?;
  Ok(publication(
    Publishing::Profile,
    (resource, name),
    (latest.number().get() + 1, saved),
  ))
}

/// The publication of the latest published version of the seed's first published pipeline, as if it had just
/// been proposed: its facts read the version it published.
fn published(db: &ProjectDb) -> Result<(String, Option<String>, ProposedChange), Box<dyn Error>> {
  let (resource, name, latest) = latest_of::<Pipeline>(db, ResourceKind::Pipeline)?;
  Ok(publication(
    Publishing::Pipeline,
    (resource, name),
    (latest.number().get(), latest.published_at()),
  ))
}

/// The first resource of `kind` with a published version: its id, name and that version.
fn latest_of<B: ResourceBody>(
  db: &ProjectDb,
  kind: ResourceKind,
) -> Result<(ResourceId, String, Version<B>), Box<dyn Error>> {
  for found in db.resources(kind)? {
    if let Some(latest) = db.latest_version::<B>(found.id())? {
      return Ok((found.id(), found.name().as_str().to_owned(), latest));
    }
  }
  Err("the tasks scenario has a published resource of the kind".into())
}

/// Saves `body` as the draft of `resource` over its version `latest`; when it was saved.
fn save<B: ResourceBody>(
  db: &mut ProjectDb,
  resource: ResourceId,
  body: B,
  latest: VersionNumber,
) -> Result<Timestamp, Box<dyn Error>> {
  let saved = Timestamp::now();
  db.update_draft::<B>(resource, |_, _| Ok(Draft::new(body, Some(latest), saved)))?;
  Ok(saved)
}

/// The seed's runs the cards act on: one still going, one that succeeded, one merged, and their tasks' keys.
struct Runs {
  going: (Run, String),
  ready: (Run, String),
  merged: (Run, String),
}

impl Runs {
  fn of(db: &ProjectDb) -> Result<Self, Box<dyn Error>> {
    let mut all = Vec::new();
    for task in db.tasks()? {
      for run in db.runs_of_task(task.id())?.into_iter().flatten() {
        let merged = db.run_merge(run.id())?.is_some();
        all.push((run, task.key().to_string(), merged));
      }
    }
    let pick = |wanted: &dyn Fn(&Run, bool) -> bool| {
      all
        .iter()
        .find(|(run, _, merged)| wanted(run, *merged))
        .or_else(|| all.first())
        .map(|(run, key, _)| (run.clone(), key.clone()))
        .ok_or("the tasks scenario has a run")
    };
    Ok(Self {
      going: pick(&|run, _| !run.status().is_terminal())?,
      ready: pick(&|run, merged| *run.status() == RunStatus::Succeeded && !merged)?,
      merged: pick(&|_, merged| merged)?,
    })
  }

  fn cancel(&self, outcome: ActionOutcome) -> ChatAction {
    let (run, key) = &self.going;
    let action = change(
      &(
        "cancel_run".to_owned(),
        Some(key.clone()),
        ProposedChange {
          subject: String::new(),
          change: Change::Cancel {
            run: run.id(),
            number: run.number().get(),
          },
        },
      ),
      outcome,
    );
    settle(action)
  }

  fn merge(&self, outcome: ActionOutcome, merged: bool) -> ChatAction {
    let (run, key) = if merged { &self.merged } else { &self.ready };
    let action = change(
      &(
        "merge_run".to_owned(),
        Some(key.clone()),
        ProposedChange {
          subject: String::new(),
          change: Change::Merge {
            run: run.id(),
            number: run.number().get(),
            checkpoint: run
              .last_checkpoint()
              .map(|checkpoint| checkpoint.revision().to_string()),
            into: run.base_branch().map(|branch| branch.as_str().to_owned()),
          },
        },
      ),
      outcome,
    );
    settle(action)
  }

  /// A start the chat took in auto mode: the going run, or refused for `refused`.
  fn started(&self, refused: Option<&str>) -> ChatAction {
    let (run, key) = &self.going;
    let outcome = match refused {
      Some(reason) => ActionOutcome::Refused {
        code: "task_blocked".to_owned(),
        reason: reason.to_owned(),
      },
      None => ActionOutcome::Done,
    };
    ChatAction {
      started: refused.is_none().then(|| run.id()),
      auto_mode: true,
      ..ChatAction::new("run_task", Some(key.clone()), outcome)
    }
  }
}

/// `action` settled by a person now, unless it waits.
fn settle(action: ChatAction) -> ChatAction {
  if action.outcome.is_proposed() {
    return action;
  }
  ChatAction {
    settled: Some(Timestamp::now()),
    ..action
  }
}
