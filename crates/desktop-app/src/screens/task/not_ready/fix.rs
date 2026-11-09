//! What each row of the not-ready card offers: the fix's words and the page it opens (rule 8 of `Agents · v2` `acFkz`).
//! A fix is where the person sets the one thing on this computer: a team MCP server's program path or its switch, the
//! secret's value, the connection a profile runs on, all on the Agents hub's resource of a team project; a local MCP
//! server's program on its own page and a local secret's value on its Replace value; a connection to add on the
//! connections page.

use orchester_core::{
  connection::AgentProvider,
  sharing::{ConnectionProblem, Missing},
};
use orchester_workspace::readiness::NotReadyPart;

use crate::routes::{ConnectionsView, HubFilter, HubView, ProjectPage, SecretsPanel, SecretsView};

/// What a row's fix says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fix {
  SetPath,
  ChooseFile,
  AddValue,
  Choose,
  AddConnection,
  SignIn,
  SwitchOn,
}

impl Fix {
  pub const fn key(self) -> &'static str {
    match self {
      Self::SetPath => "summary.notReady.fix.setPath",
      Self::ChooseFile => "summary.notReady.fix.chooseFile",
      Self::AddValue => "summary.notReady.fix.addValue",
      Self::Choose => "summary.notReady.fix.choose",
      Self::AddConnection => "summary.notReady.fix.addConnection",
      Self::SignIn => "summary.notReady.fix.signIn",
      Self::SwitchOn => "summary.notReady.fix.switchOn",
    }
  }
}

/// The fix of `part` and the page it opens; nothing for a part only a team Admin or the keychain can settle, which the
/// card's Agents button leads to.
pub fn of(part: &NotReadyPart) -> Option<(Fix, ProjectPage)> {
  let NotReadyPart::Missing(part) = part else {
    return None;
  };
  let hub = |filter, item| ProjectPage::hub(HubView::new(filter, item));
  match &part.missing {
    Missing::Connection {
      problem: ConnectionProblem::NoneOfKind,
      ..
    }
    | Missing::DecisionConnection {
      problem: ConnectionProblem::NoneOfKind,
      ..
    } => Some((Fix::AddConnection, ProjectPage::Connections(ConnectionsView::default()))),
    Missing::ConnectionSignIn { .. } => Some((Fix::SignIn, ProjectPage::Connections(ConnectionsView::default()))),
    Missing::Connection { profile, .. } => Some((Fix::Choose, hub(HubFilter::Profiles, Some(*profile)))),
    Missing::DecisionConnection { reviewer, .. } => Some((Fix::Choose, hub(HubFilter::All, Some(*reviewer)))),
    Missing::McpOff { server } => Some((Fix::SwitchOn, hub(HubFilter::McpServers, Some(*server)))),
    Missing::McpProgram { server, problem } => {
      let fix = match problem {
        orchester_core::sharing::ProgramProblem::NotSet => Fix::SetPath,
        orchester_core::sharing::ProgramProblem::FileGone(_) => Fix::ChooseFile,
      };
      Some((fix, hub(HubFilter::McpServers, Some(*server))))
    }
    Missing::Secret { .. } => Some((Fix::AddValue, hub(HubFilter::All, part.owner))),
    Missing::LocalMcpProgram { server, problem, .. } => {
      let fix = match problem {
        orchester_core::sharing::LocalProgramProblem::NotFound => Fix::SetPath,
        orchester_core::sharing::LocalProgramProblem::Moved => Fix::ChooseFile,
      };
      Some((fix, ProjectPage::EditConnection(*server)))
    }
    Missing::LocalSecret { secret, .. } => Some((
      Fix::AddValue,
      ProjectPage::Secrets(SecretsView {
        secret: Some(*secret),
        panel: SecretsPanel::Replace,
        ..SecretsView::default()
      }),
    )),
    Missing::McpUnsupported { .. } | Missing::SecretUnreadable { .. } => None,
  }
}

/// A CLI's name as the card says it.
pub const fn cli_key(cli: AgentProvider) -> &'static str {
  match cli {
    AgentProvider::ClaudeCodeCli => "summary.notReady.cli.claude",
    AgentProvider::CodexCli => "summary.notReady.cli.codex",
    AgentProvider::AnthropicApi => "summary.notReady.cli.anthropicApi",
    AgentProvider::OpenAiApi => "summary.notReady.cli.openAiApi",
  }
}
