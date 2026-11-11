//! The last known CLI each step of the panel's edit ran on ([`KnownCli`]), which the Agent field reads a model override
//! against when the person chooses another agent: a choice whose CLI is not known leaves it as it was. It is the
//! edit's, not the field's, so leaving the step and coming back keeps it. It holds while the step still runs what the
//! last choice gave it; after a save or *Cancel* (the panel's epoch), or once the step runs something else that no
//! choice gave it (a restored version), it starts again from the CLI the step runs on then.

use lurq::{app::component::DevtoolsInspectable, core::Signal};
use orchester_core::{connection::AgentProvider, resource::StepKey};
use orchester_workspace::pipelines::fields::{Executor, KnownCli};

/// What one step walked: the executor its last choice gave it, and the last known CLI.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Walked {
  key: StepKey,
  executor: Executor,
  path: KnownCli,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::screens::pipeline_hub) struct Paths {
  /// The panel's epoch the paths were walked in.
  epoch: u64,
  steps: Vec<Walked>,
}
impl DevtoolsInspectable for Paths {}

pub(in crate::screens::pipeline_hub) type PathsSignal = Signal<Paths>;

/// The path of step `key` in `epoch`, which runs `now` on CLI `cli`: the one walked when the step still runs what its
/// last choice gave it, else a new one from `cli`.
pub(super) fn path(
  paths: &PathsSignal,
  (epoch, key): (u64, &StepKey),
  (now, cli): (Executor, Option<AgentProvider>),
) -> KnownCli {
  let paths = paths.get_untracked();
  paths
    .steps
    .iter()
    .find(|walked| walked.key == *key && walked.executor == now)
    .filter(|_| paths.epoch == epoch)
    .map_or(KnownCli::new(cli), |walked| walked.path)
}

/// Keeps `path` as step `key`'s in `epoch`, now that a choice gave it `executor`; what an earlier epoch walked goes.
pub(super) fn keep(paths: &PathsSignal, (epoch, key): (u64, &StepKey), (executor, path): (Executor, KnownCli)) {
  paths.update(|paths| {
    if paths.epoch != epoch {
      *paths = Paths {
        epoch,
        steps: Vec::new(),
      };
    }
    paths.steps.retain(|walked| walked.key != *key);
    paths.steps.push(Walked {
      key: key.clone(),
      executor,
      path,
    });
  });
}
