//! Side questions about a run on its task page (ORC-337, `orchester_core::run::side_question`): the question is kept
//! with the task as it is asked, the run's transcript and changes are read for the side agent ([`read`]), the
//! project's runner answers it on the Orchestrator's connection and model, and how it ended is kept. Nothing of it is
//! part of the run's conversation; only *Send to {agent}* passes it on ([`send`]).

mod read;
mod send;

use std::{path::Path, sync::LazyLock};

use orchester_core::{
  id::{RunId, SideQuestionId, TaskId},
  run::side_question::{QuestionError, SideFailure, SideInput, SideOutcome, SideQuestion, SideQuestionText, SideState},
  time::Timestamp,
};
use orchester_exec::programs::Programs;
use orchester_ipc::Rejection;
use orchester_store::{ProjectDb, StoreError};

pub use self::{
  read::{has_transcript_in, run_stage_in, side_input_in, transcript_lines, unified_text},
  send::{SentOn, send_decision, send_on, send_on_as, send_on_in, send_way},
};
use crate::{
  background,
  messages::{self, Message},
  runner::{RequestError, RunRequestError, Runners},
  workspace::OpenProject,
};

/// When this app started asking side questions: one still being answered that was asked before then was cut short
/// by the app's end, as no runner answers it any more.
static STARTED: LazyLock<Timestamp> = LazyLock::new(Timestamp::now);

/// One question to ask: its id, given once for the draft, the task and run it is about, and its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asking {
  pub id: SideQuestionId,
  pub task: TaskId,
  pub run: RunId,
  pub text: String,
}

/// The side questions of `task` in the project at `root`, in the order they were asked; one this app did not ask
/// and that is still being answered reads as cut short. Blocks on SQLite.
pub fn side_questions_in(root: &Path, task: TaskId) -> Result<Vec<SideQuestion>, StoreError> {
  let started = *STARTED;
  let asked = ProjectDb::open(root)?.side_questions(task)?;
  Ok(
    asked
      .into_iter()
      .map(|mut question| {
        if question.state == SideState::Answering && question.asked_at < started {
          question.state = SideState::Failed {
            failure: SideFailure::Interrupted,
            at: question.asked_at,
          };
        }
        question
      })
      .collect(),
  )
}

/// Keeps the question `asking` says, being answered. Blocks on SQLite.
pub fn keep_in(root: &Path, asking: &Asking) -> Result<SideQuestion, Message> {
  LazyLock::force(&STARTED);
  let text = SideQuestionText::new(&asking.text).map_err(|error| match error {
    QuestionError::Blank => Message::new("sideQuestion.errors.blank"),
    QuestionError::TooLong => Message::new("sideQuestion.errors.tooLong"),
  })?;
  let question = SideQuestion::asked(asking.id, (asking.task, asking.run), text, Timestamp::now());
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  db.keep_side_question(&question)
    .map_err(|error| messages::store_error(&error))?;
  Ok(question)
}

/// Keeps how side question `id` ended, as `outcome` says. Blocks on SQLite.
pub fn end_in(root: &Path, id: SideQuestionId, outcome: &SideOutcome) -> Result<(), Message> {
  let mut db = ProjectDb::open(root).map_err(|error| messages::store_error(&error))?;
  let ended = match outcome {
    SideOutcome::Answered { reply, model } => Ok((reply, model.as_str())),
    SideOutcome::Failed { failure } => Err(failure),
  };
  db.end_side_question(id, ended, Timestamp::now())
    .map(drop)
    .map_err(|error| messages::store_error(&error))
}

/// Asks the question `asking` says about a run of `project`: keeps it, reads what the side agent answers from (git
/// runs through `programs`, the run's worktree is read by the project's runner) and has the project's runner answer
/// it, then keeps how that ended. Nothing reaches the
/// run's agent. A question that cannot be kept is not asked; once kept, it always ends, answered or with why not.
pub async fn ask(
  (runners, programs): (&Runners, &Programs),
  project: &OpenProject,
  asking: Asking,
) -> Result<(), Message> {
  let root = project.repo_root.clone();
  let kept = asking.clone();
  let question = background::run(move || keep_in(&root, &kept)).await??;
  let (task, run, text) = (asking.task, asking.run, question.question.clone());
  let outcome = match side_input(runners, (project, programs), (task, run), text).await {
    Ok(input) => runners
      .answer_side_question_in(project, asking.id, input)
      .await
      .unwrap_or_else(|error| SideOutcome::Failed {
        failure: not_answered(&error),
      }),
    Err(why) => {
      tracing::warn!(?why, "the side question's run could not be read");
      SideOutcome::Failed {
        failure: SideFailure::Failed {
          detail: "the run could not be read".to_owned(),
        },
      }
    }
  };
  let (root, id) = (project.repo_root.clone(), asking.id);
  background::run(move || end_in(&root, id, &outcome)).await?
}

/// What the side agent reads about `run` of `task` in `project` to answer `question`: the run's worktree read by the
/// project's runner, the rest off the async runtime ([`side_input_in`]).
async fn side_input(
  runners: &Runners,
  (project, programs): (&OpenProject, &Programs),
  (task, run): (TaskId, RunId),
  question: SideQuestionText,
) -> Result<SideInput, Message> {
  let root = project.repo_root.clone();
  let stage = background::run(move || run_stage_in(&root, run)).await??;
  let worktree = crate::tasks::changes::not_checkpointed_in(runners, project, (run, stage)).await;
  let (root, programs) = (project.repo_root.clone(), programs.clone());
  background::run(move || side_input_in(&root, &programs, (task, run), (&question, &worktree))).await?
}

/// Asks `project`'s runner to stop the side agent answering `id`.
pub async fn stop(runners: &Runners, project: &OpenProject, id: SideQuestionId) -> Result<(), Message> {
  runners
    .stop_side_question_in(project, id)
    .await
    .map_err(|_| Message::new("sideQuestion.stop.failed"))
}

/// What the person reads of why side question got no answer.
pub fn failure_message(failure: &SideFailure) -> Message {
  match failure {
    SideFailure::NoOrchestrator => Message::new("sideQuestion.failure.noOrchestrator"),
    SideFailure::SignedOut => Message::new("sideQuestion.failure.signedOut"),
    SideFailure::DailyBudgetUsed => Message::new("sideQuestion.failure.dailyBudgetUsed"),
    SideFailure::SecretsUnreadable => Message::new("sideQuestion.failure.secretsUnreadable"),
    SideFailure::TimedOut => Message::new("sideQuestion.failure.timedOut"),
    SideFailure::Failed { detail } => Message::new("sideQuestion.failure.failed").with("detail", detail.clone()),
    SideFailure::Stopped => Message::new("sideQuestion.failure.stopped"),
    SideFailure::Interrupted => Message::new("sideQuestion.failure.interrupted"),
  }
}

/// Why the runner gave no answer, as the question keeps it.
fn not_answered(error: &RunRequestError) -> SideFailure {
  match error {
    RunRequestError::Request(RequestError::NotConnected | RequestError::Lost) => SideFailure::Interrupted,
    RunRequestError::Rejected(Rejection::ShuttingDown) => SideFailure::Interrupted,
    RunRequestError::Rejected(rejection) => SideFailure::Failed {
      detail: format!("{rejection:?}"),
    },
  }
}
