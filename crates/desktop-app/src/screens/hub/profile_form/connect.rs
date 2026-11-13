//! The project's connection on an account of the person's, made when the account is first looked at in the *Runs on*
//! picker (`docs/design/agent-accounts.md`, "Agents run on accounts"): nobody adds or names a connection, and the
//! models an account serves are read from the project's connection on it. Making one checks the CLI on the account,
//! which takes a moment, so it is made off the UI thread: the account's line says its models are being read, and a
//! refusal (the CLI's program is not found) is said under the picker.

use std::path::PathBuf;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
};
use orchester_core::id::{AgentAccountId, ProjectId};
use orchester_workspace::{background, messages::Message, tasks::outside::Outside};

use crate::services::Services;

/// An account a connection was asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Asked(AgentAccountId);
impl DevtoolsInspectable for Asked {}

/// Makes the project's connection on an account. Cloning shares it.
#[derive(Clone)]
pub(in crate::screens) struct Connect {
  action: FutureAction<AgentAccountId, (), Message>,
  /// The account last asked for.
  asked: Signal<Option<Asked>>,
  /// The account the picker last opened on with nothing chosen ([`Self::look`]).
  looked: Signal<Option<Asked>>,
}

impl Connect {
  /// For the project `project` at `root`; the screens that read its connections read them again once one is made.
  pub(in crate::screens) fn new(ctx: &mut Ctx, root: PathBuf, project: ProjectId) -> Self {
    let services = Services::of(ctx);
    let action = ctx.future_action(move |account: AgentAccountId| {
      let (root, changes, workspace) = (root.clone(), services.changes.clone(), services.workspace.clone());
      async move {
        background::run(move || {
          workspace
            .connect_account(&root, account)
            .map_err(|error| error.message())
        })
        .await??;
        changes.record(project, Outside::Connections);
        Ok(())
      }
    });
    Self {
      action,
      asked: ctx.signal(None),
      looked: ctx.signal(None),
    }
  }

  /// Makes the project's connection on `account`, unless one is being made.
  pub(super) fn run(&self, account: AgentAccountId) {
    if self.action.run_if_idle(account) {
      self.asked.set(Some(Asked(account)));
    }
  }

  /// The picker opened on `account` with nothing chosen, as it does in a project that has no connection yet: its
  /// connection is made as a press on it would make it, once; a press asks again.
  pub(super) fn look(&self, account: AgentAccountId) {
    if self.looked.get_untracked() != Some(Asked(account)) {
      self.looked.set(Some(Asked(account)));
      self.run(account);
    }
  }

  /// The account whose connection is being made now.
  pub(super) fn connecting(&self) -> Option<AgentAccountId> {
    self
      .action
      .state()
      .get()
      .is_pending()
      .then(|| self.asked.get())
      .flatten()
      .map(|asked| asked.0)
  }

  /// Why the last one asked for was not made.
  pub(super) fn refused(&self) -> Option<Message> {
    let state = self.action.state().get();
    let pending = state.is_pending();
    state.error.filter(|_| !pending)
  }
}
