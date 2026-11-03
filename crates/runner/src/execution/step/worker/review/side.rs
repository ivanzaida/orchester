//! The side agent of a side question about a run (`orchester_core::run::side_question`): on the Orchestrator's
//! connection and model, and on its API provider when it has one, in the reviewer's isolation (an empty folder the
//! runner removes afterwards, supervised, bounded in time and stopped when asked) with no tools at all. It reads only
//! what the desktop gave it, the run's transcript and changes, with every value of the project's secrets masked first;
//! its answer is masked the same way, and the provider's key is kept out of all of it.

use std::{
  path::{Path, PathBuf},
  sync::Arc,
  time::Duration,
};

use orchester_core::{
  command::{SecretVariable, redact_secrets},
  connection::{
    AgentConnection, ApiProvider,
    invocation::{OnProvider, account_environment, agent_environment},
    reviewer::CliSideAgent,
  },
  memory::SecretScreen,
  orchestrator::runs_orchestrator,
  run::{
    review::ReviewError,
    side_question::{SIDE_INSTRUCTIONS, SIDE_SCHEMA, SideFailure, SideInput, SideOutcome, SideReply, SideSource},
  },
  secret::{SecretValue, SecretVault},
  store::{ConnectionStore, SecretStore, SettingsStore},
};
use orchester_store::ProjectDb;
use tokio::{runtime::Handle, sync::watch};

use super::process::{ReviewPlace, SupervisedRunner};
use crate::execution::{connection_check, idle::check_daily_budget};

/// The longest an answer may take; the side agent is stopped then.
const SIDE_TIMEOUT: Duration = Duration::from_secs(180);

/// Answers `input` with the side agent of the project at `root`, its keys read from `vault`, until `stop` says so.
pub(in crate::execution) async fn answer_side(
  root: PathBuf,
  vault: Arc<dyn SecretVault>,
  input: SideInput,
  stop: watch::Receiver<()>,
) -> SideOutcome {
  let runtime = Handle::current();
  let answered =
    tokio::task::spawn_blocking(move || side_blocking(&root, vault.as_ref(), input, (stop, runtime))).await;
  match answered {
    Ok(outcome) => outcome,
    Err(error) => failed(error.to_string()),
  }
}

/// What the side agent starts from: its command's parts, the secret values to mask, and the provider's key.
struct Setup {
  agent: CliSideAgent,
  screen: SecretScreen,
  on_provider: Option<(ApiProvider, Arc<SecretValue>)>,
  model: String,
}

fn side_blocking(
  root: &Path,
  vault: &dyn SecretVault,
  input: SideInput,
  (stop, runtime): (watch::Receiver<()>, Handle),
) -> SideOutcome {
  let place = match ReviewPlace::with(SIDE_INSTRUCTIONS, SIDE_SCHEMA) {
    Ok(place) => place,
    Err(error) => return failed(format!("cannot make the folder the side agent runs in: {error}")),
  };
  let setup = match setup(root, vault, &place) {
    Ok(setup) => setup,
    Err(failure) => return SideOutcome::Failed { failure },
  };
  let input = input.masked(&|text| setup.screen.mask(text));
  let mut command = setup.agent.command(&input);
  let mut secrets: Vec<SecretVariable> = Vec::new();
  if let Some((provider, key)) = &setup.on_provider {
    let model = &setup.agent.model.0;
    let on = OnProvider {
      provider,
      model,
      key: Arc::clone(key),
    };
    command = on.apply(command, setup.agent.provider);
    secrets.push(on.key_variable(setup.agent.provider));
  }
  let probe = stop.clone();
  let runner = SupervisedRunner {
    runtime,
    timeout: SIDE_TIMEOUT,
    stop,
  };
  let asked = setup.agent.ask(&command, &runner);
  // The folder goes only after the side agent, which runs in it, has ended.
  drop(place);
  let mask = |text: &str| setup.screen.mask(&redact_secrets(text, &secrets));
  match asked.reply {
    Ok(reply) => SideOutcome::Answered {
      reply: masked_reply(&reply, &mask),
      model: setup.model,
    },
    // Stopped by the person: the side agent was ended on purpose.
    Err(_) if matches!(probe.has_changed(), Ok(true)) => SideOutcome::Failed {
      failure: SideFailure::Stopped,
    },
    Err(ReviewError::TimedOut) => SideOutcome::Failed {
      failure: SideFailure::TimedOut,
    },
    Err(error) => {
      tracing::warn!(detail = error.detail(), "the side agent gave no answer");
      failed(error.detail())
    }
  }
}

/// The side agent of the project at `root` on the Orchestrator's connection and model, running in `place`; or why it
/// cannot answer.
fn setup(root: &Path, vault: &dyn SecretVault, place: &ReviewPlace) -> Result<Setup, SideFailure> {
  let db = ProjectDb::open(root).map_err(|error| failure(error.to_string()))?;
  let settings = db
    .orchestrator_settings()
    .map_err(|error| failure(error.to_string()))?
    .ok_or(SideFailure::NoOrchestrator)?;
  check_daily_budget(&db).map_err(|_| SideFailure::DailyBudgetUsed)?;
  let stored = db
    .connection(settings.connection())
    .map_err(|error| failure(error.to_string()))?
    .ok_or(SideFailure::NoOrchestrator)?;
  let connection = AgentConnection::try_from(stored).map_err(|_| SideFailure::NoOrchestrator)?;
  let provider = connection.provider();
  if !runs_orchestrator(provider) {
    return Err(SideFailure::NoOrchestrator);
  }
  if connection.account().start_problem(Path::is_dir).is_some() {
    return Err(SideFailure::SignedOut);
  }
  let executable = connection
    .executable()
    .ok_or_else(|| failure("the connection has no CLI program".to_owned()))?
    .to_path_buf();
  let on_provider = connection
    .api_provider()
    .map(|on| connection_check::api_key(&db, vault, on.key()).map(|key| (on.clone(), key)))
    .transpose()
    .map_err(failure)?;
  let secrets = db.secrets().map_err(|error| failure(error.to_string()))?;
  let screen = SecretScreen::load(db.project().id(), &secrets, vault).map_err(|_| SideFailure::SecretsUnreadable)?;
  let mut environment = agent_environment(std::env::vars_os());
  db.project_environment()
    .map_err(|error| failure(error.to_string()))?
    .add_to(&mut environment);
  let environment =
    account_environment(environment, connection.account(), provider).map_err(|error| failure(error.to_string()))?;
  Ok(Setup {
    agent: CliSideAgent {
      provider,
      executable,
      model: (settings.model().clone(), settings.effort().map(str::to_owned)),
      folder: place.folder(),
      environment,
    },
    screen,
    on_provider,
    model: settings.model().as_str().to_owned(),
  })
}

/// `reply` with `mask` applied to its text and sources.
fn masked_reply(reply: &SideReply, mask: &dyn Fn(&str) -> String) -> SideReply {
  SideReply {
    text: mask(&reply.text),
    sources: reply
      .sources
      .iter()
      .map(|source| SideSource {
        kind: source.kind,
        name: mask(&source.name),
        detail: mask(&source.detail),
      })
      .collect(),
  }
}

const fn failure(detail: String) -> SideFailure {
  SideFailure::Failed { detail }
}

const fn failed(detail: String) -> SideOutcome {
  SideOutcome::Failed {
    failure: failure(detail),
  }
}
