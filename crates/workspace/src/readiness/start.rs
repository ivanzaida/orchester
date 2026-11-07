//! A start that is refused because the pipeline it runs is not ready on this computer
//! (`docs/design/team-configuration.md`, rule 13): the run starts nothing, counts no attempt, and says which parts are
//! missing, instead of starting and failing later.
//!
//! This is the minimal shared answer of a local start. It reads the pipeline's [`ReadinessStanding`] and refuses on
//! what the preflight ([`orchester_core::run::preflight`]) cannot see: the program of an MCP server a profile starts,
//! the value of a tool's secret, a keychain that cannot say. What the preflight sees and refuses with its own typed
//! reason (a profile's connection that is gone, a profile or tool with no published version, a secret the project no
//! longer has) stays the preflight's: the standing is then [`Undecided`] and no refusal is made here. Where both would
//! refuse (an archived secret), this refuses first, with the part to fix.

use std::{collections::HashMap, path::Path};

use orchester_core::{
  id::{ResourceId, SecretId},
  secret::SecretVault,
  sharing::{ConnectionProblem, LocalProgramProblem, Missing, ProgramProblem},
};

use super::{
  ReadinessError,
  project::{PipelineReadiness, ProjectReadiness},
  standing::{ReadinessStanding, Undecided},
};
use crate::{messages::Message, workspace::Workspace};

/// The code a refusal of a start for this reason carries.
pub const NOT_READY_HERE: &str = "not_ready_here";

/// One part that keeps a pipeline from running here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unready {
  /// What the computer lacks, as the readiness rule names it.
  Missing(Missing),
  /// The keychain did not say whether the tool's secret has a value, so the run cannot be known to find it.
  SecretUnreadable(SecretId),
}

/// A pipeline that cannot run on this computer now, and why. Holds no secret value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotReadyHere {
  pipeline: String,
  parts: Vec<Unready>,
  /// The names of the project's resources, to say which profile or server a part is about.
  names: HashMap<ResourceId, String>,
}

impl NotReadyHere {
  /// The pipeline's name.
  pub fn pipeline(&self) -> &str {
    &self.pipeline
  }

  /// The parts to fix, in the order to fix them. Never empty.
  pub fn parts(&self) -> &[Unready] {
    &self.parts
  }

  /// What a person reads: the pipeline, then one message per part.
  pub fn messages(&self) -> Vec<Message> {
    std::iter::once(Message::new("runs.notReadyHere.lead").with("pipeline", self.pipeline.clone()))
      .chain(self.parts.iter().map(|part| self.message(part)))
      .collect()
  }

  fn named(&self, resource: ResourceId) -> String {
    self
      .names
      .get(&resource)
      .cloned()
      .unwrap_or_else(|| resource.to_string())
  }

  fn message(&self, part: &Unready) -> Message {
    let missing = match part {
      Unready::SecretUnreadable(_) => return Message::new("runs.notReadyHere.secretUnreadable"),
      Unready::Missing(missing) => missing,
    };
    match missing {
      Missing::Connection { profile, problem, .. } => {
        let key = match problem {
          ConnectionProblem::NoneOfKind => "runs.notReadyHere.connection.none",
          ConnectionProblem::PickNeeded => "runs.notReadyHere.connection.pickNeeded",
        };
        Message::new(key).with("profile", self.named(*profile))
      }
      Missing::McpOff { server } => Message::new("runs.notReadyHere.mcpOff").with("server", self.named(*server)),
      Missing::McpProgram { server, problem } => {
        let server = self.named(*server);
        match problem {
          ProgramProblem::NotSet => Message::new("runs.notReadyHere.mcpProgram.notSet").with("server", server),
          ProgramProblem::FileGone(path) => Message::new("runs.notReadyHere.mcpProgram.fileGone")
            .with("server", server)
            .with("path", path.display().to_string()),
        }
      }
      Missing::Secret { name } => Message::new("runs.notReadyHere.secret").with("name", name.to_string()),
      Missing::LocalMcpProgram { name, problem, .. } => {
        let key = match problem {
          LocalProgramProblem::NotFound => "runs.notReadyHere.localProgram.notFound",
          LocalProgramProblem::Moved => "runs.notReadyHere.localProgram.moved",
        };
        Message::new(key).with("server", name.to_string())
      }
      Missing::LocalSecret { name, .. } => Message::new("runs.notReadyHere.localSecret").with("name", name.to_string()),
    }
  }

  fn of(readiness: &ProjectReadiness, pipeline: &PipelineReadiness) -> Option<Self> {
    let parts = match pipeline.standing() {
      ReadinessStanding::Ready => return None,
      ReadinessStanding::Missing(missing) => missing.missing().iter().cloned().map(Unready::Missing).collect(),
      ReadinessStanding::Undecided(why) => vec![Unready::SecretUnreadable(unreadable_secret(why)?)],
    };
    Some(Self {
      pipeline: pipeline.name().to_owned(),
      parts,
      names: readiness
        .resources()
        .iter()
        .map(|found| (found.resource(), found.name().to_owned()))
        .collect(),
    })
  }
}

/// The secret a pipeline is undecided about because the keychain would not say; any other reason is the preflight's.
fn unreadable_secret(why: &Undecided) -> Option<SecretId> {
  match why {
    Undecided::SecretUnreadable(secret) => Some(*secret),
    Undecided::Member { why, .. } => unreadable_secret(why),
    _ => None,
  }
}

impl Workspace {
  /// Whether a run of `pipeline` in the project at `repo_root` may start here: `None`, or what keeps it from running.
  /// Reads the project's readiness once ([`Workspace::project_readiness`]: it blocks, and it reads the keychain for
  /// each local tool secret), so call it as a start is asked for, never on a timer. A pipeline the project has no
  /// published version of is left to the preflight, which refuses it with its own reason.
  pub fn not_ready_here(
    &self,
    repo_root: &Path,
    vault: &dyn SecretVault,
    pipeline: ResourceId,
  ) -> Result<Option<NotReadyHere>, ReadinessError> {
    let readiness = self.project_readiness(repo_root, vault)?;
    Ok(
      readiness
        .pipeline(pipeline)
        .and_then(|found| NotReadyHere::of(&readiness, found)),
    )
  }
}
