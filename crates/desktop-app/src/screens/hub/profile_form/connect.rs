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
use orchester_core::id::{AgentAccountId, ApiAccountId, ProjectId};
use orchester_workspace::{background, messages::Message, tasks::outside::Outside};

use crate::services::Services;

/// What the project gets a connection on: a sign-in of the person's, or one of their API accounts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::screens::hub::profile_form) enum On {
  Account(AgentAccountId),
  Api(ApiAccountId),
}
impl DevtoolsInspectable for On {}

/// Makes the project's connection on an account. Cloning shares it.
#[derive(Clone)]
pub(in crate::screens) struct Connect {
  action: FutureAction<On, (), Message>,
  /// The account last asked for.
  asked: Signal<Option<On>>,
  /// The account the picker last opened on with nothing chosen ([`Self::look`]).
  looked: Signal<Option<On>>,
  /// The account asked for while another's connection was being made: it is made next.
  waiting: Signal<Option<On>>,
}

impl Connect {
  /// For the project `project` at `root`; the screens that read its connections read them again once one is made.
  pub(in crate::screens) fn new(ctx: &mut Ctx, root: PathBuf, project: ProjectId) -> Self {
    let services = Services::of(ctx);
    let action = ctx.future_action(move |on: On| {
      let (root, changes, workspace) = (root.clone(), services.changes.clone(), services.workspace.clone());
      async move {
        background::run(move || {
          match on {
            On::Account(account) => workspace.connect_account(&root, account),
            On::Api(account) => workspace.connect_api_account(&root, account),
          }
          .map_err(|error| error.message())
        })
        .await??;
        changes.record(project, Outside::Connections);
        Ok(())
      }
    });
    let connect = Self {
      action,
      asked: ctx.signal(None),
      looked: ctx.signal(None),
      waiting: ctx.signal(None),
    };
    // One connection is made at a time; the account asked for meanwhile is next.
    let next = connect.clone();
    ctx.watch(&connect.action.state(), move |state| {
      if state.is_pending() {
        return;
      }
      if let Some(on) = next.waiting.get_untracked() {
        next.waiting.set(None);
        next.run(on);
      }
    });
    connect
  }

  /// Makes the project's connection on `account`; while another's is being made, right after it.
  pub(super) fn run(&self, account: On) {
    if self.action.run_if_idle(account) {
      self.asked.set(Some(account));
    } else if self.asked.get_untracked() != Some(account) {
      self.waiting.set(Some(account));
    }
  }

  /// The picker opened on `account` with nothing chosen, as it does in a project that has no connection yet: its
  /// connection is made as a press on it would make it, once; a press asks again.
  pub(super) fn look(&self, account: On) {
    if self.looked.get_untracked() != Some(account) {
      self.looked.set(Some(account));
      self.run(account);
    }
  }

  /// The account whose connection is being made now.
  pub(super) fn connecting(&self) -> Option<On> {
    self
      .action
      .state()
      .get()
      .is_pending()
      .then(|| self.asked.get())
      .flatten()
  }

  /// Why the last one asked for was not made.
  pub(super) fn refused(&self) -> Option<Message> {
    let state = self.action.state().get();
    let pending = state.is_pending();
    state.error.filter(|_| !pending)
  }
}
