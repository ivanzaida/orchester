//! What one row of the not-ready card says: what it is about, what is missing, and its glyph (`Missing · …` rows of
//! `B6ee9`). A team project's rows say *here*; a local project's name no computer (L9 `f5CEX`).

use std::sync::Arc;

use lurq::app::ctx::Ctx;
use orchester_core::sharing::{ConnectionProblem, LocalProgramProblem, McpUnsupported, Missing, ProgramProblem};
use orchester_workspace::readiness::NotReadyPart;

use super::fix::cli_key;
use crate::ui::{agent_mark::AgentMark, icon::Icon, message::message_text};

/// The glyph at a row's start: a maker's mark for a connection a CLI runs on, else a lucide glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
  Icon(Icon),
  Mark(AgentMark),
}

pub struct Words {
  pub glyph: Glyph,
  pub what: Arc<str>,
  pub need: Arc<str>,
}

/// The row of `part`; `team` is a team project's.
pub fn of(ctx: &Ctx, part: &NotReadyPart, team: bool) -> Words {
  match part {
    NotReadyPart::Missing(part) => missing(ctx, &part.missing, &part.name, team),
    NotReadyPart::Undecided(part) => Words {
      glyph: Glyph::Icon(Icon::CircleAlert),
      what: Arc::from(part.name.as_str()),
      need: message_text(ctx, &part.message()),
    },
  }
}

fn missing(ctx: &Ctx, missing: &Missing, name: &str, team: bool) -> Words {
  let named = |glyph, key: &str| Words {
    glyph,
    what: Arc::from(name),
    need: ctx.t(key),
  };
  let server = Glyph::Icon(Icon::Plug);
  match missing {
    Missing::Connection { cli, problem, .. } => {
      let key = match problem {
        ConnectionProblem::NoneOfKind => "summary.notReady.need.connectionNone",
        ConnectionProblem::PickNeeded => "summary.notReady.need.connectionPick",
      };
      Words {
        glyph: Glyph::Mark(AgentMark::of(*cli)),
        what: Arc::from(name),
        need: ctx.t_args(key, [("cli", ctx.t(cli_key(*cli)).as_ref())]),
      }
    }
    Missing::ConnectionSignIn { cli, .. } => Words {
      glyph: Glyph::Mark(AgentMark::of(*cli)),
      what: Arc::from(name),
      need: ctx.t_args(
        "summary.notReady.need.connectionSignIn",
        [("cli", ctx.t(cli_key(*cli)).as_ref())],
      ),
    },
    Missing::DecisionConnection { problem, .. } => {
      let key = match problem {
        ConnectionProblem::NoneOfKind => "summary.notReady.need.decisionNone",
        ConnectionProblem::PickNeeded => "summary.notReady.need.decisionPick",
      };
      named(Glyph::Icon(Icon::Bot), key)
    }
    Missing::McpOff { .. } => named(server, "summary.notReady.need.mcpOff"),
    Missing::McpUnsupported {
      why: McpUnsupported::MoreThanOneSecretHeader,
      ..
    } => named(server, "summary.notReady.need.mcpMoreThanOneSecretHeader"),
    Missing::McpProgram {
      problem: ProgramProblem::NotSet,
      ..
    }
    | Missing::LocalMcpProgram {
      problem: LocalProgramProblem::NotFound,
      ..
    } => named(server, "summary.notReady.need.mcpProgramNotSet"),
    Missing::McpProgram {
      problem: ProgramProblem::FileGone(path),
      ..
    } => Words {
      glyph: server,
      what: Arc::from(name),
      need: ctx.t_args(
        "summary.notReady.need.mcpProgramFileGone",
        [("path", path.display().to_string())],
      ),
    },
    Missing::LocalMcpProgram {
      problem: LocalProgramProblem::Moved,
      ..
    } => named(server, "summary.notReady.need.mcpProgramMoved"),
    Missing::Secret { .. } | Missing::LocalSecret { .. } | Missing::SecretUnreadable { .. } => {
      secret(ctx, missing, name, team)
    }
  }
}

/// A secret's row: *Secret NAME*, and that this computer has no value for it, or could not say.
fn secret(ctx: &Ctx, missing: &Missing, name: &str, team: bool) -> Words {
  let need = match (missing, team) {
    (Missing::SecretUnreadable { .. }, _) => "summary.notReady.need.secretUnreadable",
    (_, true) => "summary.notReady.need.secretTeam",
    (_, false) => "summary.notReady.need.secret",
  };
  Words {
    glyph: Glyph::Icon(Icon::KeyRound),
    what: ctx.t_args("summary.notReady.secretName", [("name", name)]),
    need: ctx.t(need),
  }
}
