//! A switch of *The Orchestrator may* (`Publish drafts` `dPSKs` … `Write with git` `VOnlw` in `S03`): the project's
//! own choice of what its Orchestrator chat does itself (ORC-594), read and saved off the UI thread; while either runs,
//! the switch cannot be flipped. Only a person flips it: the row refuses lurq's synthetic input and says so under
//! itself, and while it is mounted the app's MCP server refuses window input (`mcp::input_gate`). Off holds in the
//! project from the person's click, before the save runs and if it fails.

use std::path::{Path, PathBuf};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
    events::{InputSource, input_source},
  },
  core::Signal,
  node::Element,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::{OrchestratorConsent, Workspace},
};

use super::super::{Status, TabProps, kit::SettingRow};
use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
  services::Services,
  ui::{
    field::field_error,
    icon::Icon,
    message::message_text,
    setting_notice::status_line,
    skeleton,
    toggle::{Toggle, ToggleKind},
  },
};

#[derive(Clone)]
pub(super) struct Props {
  /// The tab's props, its status the row's own section of the page's save status.
  pub tab: TabProps,
  pub consent: OrchestratorConsent,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.tab == other.tab && self.consent == other.consent
  }
}

impl DevtoolsInspectable for Props {}

/// The keys of what a switch's row says: its label, its one line and its (i).
struct Words {
  label: &'static str,
  hint: &'static str,
  info: &'static str,
}

/// The switch's element id, which its row's parts extend.
pub(super) const fn id(consent: OrchestratorConsent) -> &'static str {
  match consent {
    OrchestratorConsent::PublishDrafts => "settings-orchestrator-publish-drafts",
    OrchestratorConsent::AnswerAgents => "settings-orchestrator-answer-agents",
    OrchestratorConsent::MergeRuns => "settings-orchestrator-merge-runs",
    OrchestratorConsent::MergeFinished => "settings-orchestrator-merge-finished",
    OrchestratorConsent::ContinueInterrupted => "settings-orchestrator-continue-interrupted",
    OrchestratorConsent::WriteGit => "settings-orchestrator-write-git",
  }
}

const fn words(consent: OrchestratorConsent) -> Words {
  match consent {
    OrchestratorConsent::PublishDrafts => Words {
      label: "project_settings.orchestrator.publishDrafts.label",
      hint: "project_settings.orchestrator.publishDrafts.hint",
      info: "project_settings.orchestrator.publishDrafts.info",
    },
    OrchestratorConsent::AnswerAgents => Words {
      label: "project_settings.orchestrator.answerAgents.label",
      hint: "project_settings.orchestrator.answerAgents.hint",
      info: "project_settings.orchestrator.answerAgents.info",
    },
    OrchestratorConsent::MergeRuns => Words {
      label: "project_settings.orchestrator.mergeRuns.label",
      hint: "project_settings.orchestrator.mergeRuns.hint",
      info: "project_settings.orchestrator.mergeRuns.info",
    },
    OrchestratorConsent::MergeFinished => Words {
      label: "project_settings.orchestrator.mergeFinished.label",
      hint: "project_settings.orchestrator.mergeFinished.hint",
      info: "project_settings.orchestrator.mergeFinished.info",
    },
    OrchestratorConsent::ContinueInterrupted => Words {
      label: "project_settings.orchestrator.continueInterrupted.label",
      hint: "project_settings.orchestrator.continueInterrupted.hint",
      info: "project_settings.orchestrator.continueInterrupted.info",
    },
    OrchestratorConsent::WriteGit => Words {
      label: "project_settings.orchestrator.writeGit.label",
      hint: "project_settings.orchestrator.writeGit.hint",
      info: "project_settings.orchestrator.writeGit.info",
    },
  }
}

/// Its name in the developer log.
const fn logged(consent: OrchestratorConsent) -> &'static str {
  match consent {
    OrchestratorConsent::PublishDrafts => "Let the Orchestrator publish drafts",
    OrchestratorConsent::AnswerAgents => "Let the Orchestrator answer agents",
    OrchestratorConsent::MergeRuns => "Let the Orchestrator merge runs",
    OrchestratorConsent::MergeFinished => "Let the Orchestrator merge finished runs on its own",
    OrchestratorConsent::ContinueInterrupted => "Let the Orchestrator continue interrupted runs",
    OrchestratorConsent::WriteGit => "Let the Orchestrator write with git",
  }
}

type SaveAction = FutureAction<bool, bool, Message>;

pub(super) struct Switch {
  /// The setting as the person last chose it; `None` until they do.
  saved: Signal<Option<bool>>,
  /// A change the row did not take because synthetic input brought it.
  blocked: Signal<bool>,
  save: SaveAction,
  /// While the row is mounted, the MCP server refuses window input.
  _protected: Protected,
}

impl Component for Switch {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let Props { tab, consent } = ctx.props::<Props>().clone();
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    let save = save_action(ctx, tab.project.open.repo_root.clone(), consent);
    let status = tab.status;
    ctx.watch(&save.state(), move |state| {
      status.set(match (state.is_pending(), &state.error) {
        (true, _) => Status::Saving,
        (false, Some(_)) => Status::Failed,
        (false, None) => Status::Saved,
      });
    });
    Self {
      saved: ctx.signal(None),
      blocked: ctx.signal(false),
      save,
      _protected: gate.protect(Guarded::PermissionsSetting),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { tab, consent } = ctx.props::<Props>().clone();
    let (id, words) = (id(consent), words(consent));
    let root = tab.project.open.repo_root;
    let workspace = Services::of(ctx).workspace;
    let state = self.save.state().get();
    let read = stored(ctx, (&workspace, &root), (consent, state.data));
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice once it saved; the stored value until then, or after a save failed.
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    // The row takes no change before it lived through one pass after mounting: an input meant for what the window
    // showed before cannot land on it.
    let armed = ctx
      .future((), |()| async { Ok::<(), Message>(()) })
      .state()
      .get()
      .data
      .is_some();
    let switch = Toggle::new(id, ToggleKind::Switch, shown.unwrap_or(false), ctx.t(words.label)).hide_label();
    let switch = if shown.is_some() && !fixed && armed {
      self.listen(switch, (workspace, root), consent)
    } else {
      switch
    };
    // While the stored choice is read, the switch is a bone of its size.
    let control: Element = if shown.is_none() && read.is_pending() {
      skeleton::switch().into()
    } else {
      switch.into()
    };
    let problem = match (&read.error, &state.error) {
      (Some(reason), _) => Some(("project_settings.orchestrator.unreadable", reason)),
      (None, Some(reason)) => Some(("project_settings.orchestrator.notSaved", reason)),
      (None, None) => None,
    }
    .map(|(key, reason)| {
      let reason = message_text(ctx, reason);
      Element::from(field_error(
        &format!("{id}-error"),
        &ctx.t_args(key, [("reason", &*reason)]),
      ))
    });
    let blocked = self.blocked.get().then(|| {
      Element::from(field_error(
        &format!("{id}-blocked"),
        &ctx.t("project_settings.orchestrator.blocked"),
      ))
    });
    // What the Orchestrator does once it may publish, then why the row cannot be read, was not saved or did not take
    // a change.
    let said = (consent == OrchestratorConsent::PublishDrafts && shown == Some(true)).then(|| {
      Element::from(status_line(
        &format!("{id}-on"),
        Icon::MessagesSquare,
        &ctx.t("project_settings.orchestrator.publishDrafts.on"),
      ))
    });
    let row = SettingRow::new(id, ctx.t(words.label))
      .hint(ctx.t(words.hint))
      .info(ctx.t(words.info))
      .control(control)
      .under_all(said)
      .under_all(problem)
      .under_all(blocked);
    let row = if consent == OrchestratorConsent::MergeFinished {
      row.dependent()
    } else {
      row
    };
    row.render(ctx)
  }
}

impl Switch {
  /// `switch`, saving what a person chooses for the project at `root`. Synthetic input never changes it.
  fn listen(&self, switch: Toggle, (workspace, root): (Workspace, PathBuf), consent: OrchestratorConsent) -> Toggle {
    let (saved, blocked, save) = (self.saved.clone(), self.blocked.clone(), self.save.clone());
    switch.on_change(move |chosen| {
      if input_source() == InputSource::Synthetic {
        tracing::warn!(
          setting = logged(consent),
          "synthetic input reached a person's setting; not taken"
        );
        blocked.set(true);
        return;
      }
      blocked.set(false);
      if !chosen {
        // Off holds from the person's click, before the save runs and if it fails.
        workspace.revoke_orchestrator_consent(&root, consent);
      }
      saved.set(Some(chosen));
      save.run(chosen);
    })
  }
}

/// Saves the `consent` the person chose for the project at `root`.
fn save_action(ctx: &mut Ctx, root: PathBuf, consent: OrchestratorConsent) -> SaveAction {
  let workspace = Services::of(ctx).workspace;
  ctx.future_action(move |allowed: bool| {
    let (workspace, root) = (workspace.clone(), root.clone());
    async move {
      background::run(move || workspace.set_orchestrator_allows(&root, consent, allowed))
        .await?
        .map(|()| allowed)
        .map_err(|error| {
          tracing::error!(%error, setting = logged(consent), "a person's setting was not saved");
          messages::store_error(&error)
        })
    }
  })
}

/// The project's `consent` as stored, read when the row shows and again after each save (`saved`).
fn stored(
  ctx: &mut Ctx,
  (workspace, root): (&Workspace, &Path),
  (consent, saved): (OrchestratorConsent, Option<bool>),
) -> FutureState<bool, Message> {
  let (workspace, root, hold) = (workspace.clone(), root.to_path_buf(), Services::of(ctx).hold);
  ctx
    .future(saved, move |_| {
      let (workspace, root, hold) = (workspace.clone(), root.clone(), hold.clone());
      async move {
        hold.released().await;
        background::run(move || workspace.orchestrator_allows(&root, consent))
          .await?
          .map_err(|error| {
            tracing::error!(%error, setting = logged(consent), "a person's setting cannot be read");
            messages::store_error(&error)
          })
      }
    })
    .state()
    .get()
}
