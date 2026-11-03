//! The story-point estimator ([`orchester_core::task::estimate`]) on auto mode's reviewer: the same connection,
//! model and isolation (an empty folder the runner removes afterwards, supervised and bounded in time, or one request
//! of a native connection's client with its key from the keychain), with the estimate's instructions and schema. A
//! task a person sized, or one whose points were made or asked from its brief and criteria as they are, asks nothing.
//! A request that asked a model is recorded with what it spent ([`EstimateRecord`]); points are stored only where no
//! person's value is, and only while the task's brief and criteria are still those they were made from.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  connection::reviewer::{CliEstimator, Estimated, EstimatorCli, NativeSpend, native_estimate_request},
  id::TaskId,
  native::model::ModelClient,
  resource::ModelId,
  run::review::{DetailRedaction, ReviewError, ReviewFailure, ReviewSpend, ReviewerKind},
  secret::SecretVault,
  store::TaskStore,
  task::estimate::{
    ESTIMATE_INSTRUCTIONS, ESTIMATE_SCHEMA, EstimateBasis, EstimateInput, EstimateOutcome, EstimateRecord,
    StoryEstimate, check_automation, estimate_prompt,
  },
  time::Timestamp,
};
use orchester_ipc::Rejection;
use orchester_store::{AutomationPoints, ProjectDb, StoreError};
use tokio::runtime::Handle;

use super::{
  process::{ReviewPlace, SupervisedRunner, stop_review},
  setup::{ReviewerKeys, ReviewerSetup, reviewer_setup},
};

/// The longest an estimate may take; the estimator is stopped then, and the task gets no points.
const ESTIMATE_TIMEOUT: Duration = Duration::from_secs(90);

/// Estimates the story points of `task` of the project at `root`, a native reviewer's key read from `vault`.
pub(in crate::execution) async fn estimate_task(
  root: PathBuf,
  vault: Arc<dyn SecretVault>,
  task: TaskId,
) -> Result<EstimateOutcome, Rejection> {
  let runtime = Handle::current();
  tokio::task::spawn_blocking(move || estimate_blocking(&root, vault, task, runtime))
    .await
    .map_err(|error| Rejection::Failed {
      detail: error.to_string(),
    })?
}

fn estimate_blocking(
  root: &Path,
  vault: Arc<dyn SecretVault>,
  task: TaskId,
  runtime: Handle,
) -> Result<EstimateOutcome, Rejection> {
  let failed = |error: StoreError| Rejection::Failed {
    detail: error.to_string(),
  };
  let mut db = ProjectDb::open(root).map_err(failed)?;
  let keys = ReviewerKeys {
    vault,
    project: db.project().id(),
  };
  let task = db.task(task).map_err(failed)?.ok_or(Rejection::TaskNotFound)?;
  let basis = EstimateBasis::of(task.details());
  if check_automation(task.estimate()).is_err() {
    return Ok(EstimateOutcome::PersonsValue);
  }
  if !StoryEstimate::wanted(task.estimate(), &basis) {
    return Ok(EstimateOutcome::UpToDate);
  }
  if db.estimate_asked(task.id(), &basis).map_err(failed)? {
    return Ok(EstimateOutcome::TriedBefore);
  }
  let input = EstimateInput::new(task.details(), &db.calibration_records().map_err(failed)?);
  let setup = reviewer_setup(&db, &keys).map_err(failed)?;
  let provider = setup.provider().map(|(provider, ..)| provider);
  let connection = setup.connection().cloned();
  let started_at = Timestamp::now();
  let Some((model, estimated)) = setup.estimate(&input, runtime) else {
    return Ok(EstimateOutcome::Failed {
      failure: ReviewFailure::NoReviewer,
    });
  };
  let ended_at = Timestamp::now();
  let failure = estimated.answer.as_ref().err().map(ReviewError::failure);
  let usage = match estimated.spend {
    ReviewSpend::Nothing => None,
    ReviewSpend::Unknown => Some(None),
    ReviewSpend::Reported(usage) => Some(Some(usage)),
  };
  if let Err(error) = &estimated.answer {
    tracing::warn!(
      task = %task.key(),
      connection = connection.as_ref().map_or("", |used| &*used.name),
      connection_id = connection.as_ref().map(|used| used.id.to_string()).unwrap_or_default(),
      model = model.as_str(),
      failure = ?error.failure(),
      detail = error.detail(),
      "the story-point estimator gave no points"
    );
  }
  // Only a request that asked a model is kept: one that asked none is asked again at the next change.
  if let (Some(provider), Some(usage)) = (provider, usage) {
    let record = EstimateRecord {
      task: task.id(),
      basis: basis.clone(),
      provider,
      model,
      started_at,
      ended_at,
      usage,
      failure,
    };
    db.record_estimate_usage(&record).map_err(failed)?;
  }
  let answer = match estimated.answer {
    Ok(answer) => answer,
    Err(error) => {
      return Ok(EstimateOutcome::Failed {
        failure: error.failure(),
      });
    }
  };
  let estimate = StoryEstimate::automation(answer.points, Some(&answer.reason), basis, ended_at);
  Ok(match db.set_automation_points(task.id(), &estimate).map_err(failed)? {
    AutomationPoints::Stored => EstimateOutcome::Estimated { points: answer.points },
    AutomationPoints::PersonsValue => EstimateOutcome::PersonsValue,
    AutomationPoints::Outdated => EstimateOutcome::Outdated,
  })
}

impl ReviewerSetup {
  /// The estimate of `input` with this reviewer's connection and model, from the blocking pool on `runtime`; `None`
  /// when the project has no reviewer.
  fn estimate(self, input: &EstimateInput, runtime: Handle) -> Option<(ModelId, Estimated)> {
    let (_stop, stopped) = stop_review();
    let not_run = |detail: String| Estimated {
      answer: Err(ReviewError::CouldNotRun(detail)),
      spend: ReviewSpend::Nothing,
    };
    match self {
      Self::Missing => None,
      Self::Unreachable { model, .. } => Some((model, not_run("the reviewer cannot be reached".to_owned()))),
      Self::Native {
        client, model, effort, ..
      } => {
        let request = native_estimate_request(input, &model, effort);
        let mut estimated = runtime.block_on(async {
          let mut spend = NativeSpend::default();
          let mut streamed = |event| spend.observe(&event);
          match tokio::time::timeout(ESTIMATE_TIMEOUT, client.respond(&request, &mut streamed)).await {
            Ok(Ok(response)) => spend.estimated(&response),
            Ok(Err(error)) => spend.not_estimated(&error),
            Err(_) => Estimated {
              answer: Err(ReviewError::TimedOut),
              spend: ReviewSpend::Unknown,
            },
          }
        });
        // The client masks its key in its errors itself; the prompt is kept out here.
        let prompt = format!("{ESTIMATE_INSTRUCTIONS}\n{}", estimate_prompt(input));
        estimated.answer = estimated
          .answer
          .map_err(|error| error.redacted(&DetailRedaction::new(Vec::new(), &prompt)));
        Some((model, estimated))
      }
      Self::Cli {
        kind,
        executable,
        model,
        effort,
        environment,
        ..
      } => {
        let place = match ReviewPlace::with(ESTIMATE_INSTRUCTIONS, ESTIMATE_SCHEMA) {
          Ok(place) => place,
          Err(error) => {
            tracing::warn!(%error, "cannot make the folder an estimate runs in");
            return Some((model, not_run(error.to_string())));
          }
        };
        let estimator = CliEstimator {
          cli: match kind {
            ReviewerKind::Codex => EstimatorCli::Codex,
            ReviewerKind::ClaudeCode | ReviewerKind::AnthropicApi => EstimatorCli::ClaudeCode,
          },
          executable,
          model: (model.clone(), effort),
          folder: place.folder(),
          environment,
        };
        let runner = SupervisedRunner {
          runtime,
          timeout: ESTIMATE_TIMEOUT,
          stop: stopped,
        };
        let estimated = estimator.estimate(input, &runner);
        // The folder goes only after the estimator, which runs in it, has ended.
        drop(place);
        Some((model, estimated))
      }
    }
  }
}
