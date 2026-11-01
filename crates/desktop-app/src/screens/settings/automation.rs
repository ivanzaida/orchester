//! Settings › Automation (`04 · Automation` `aMkOw` in the new `orchester.pen`): *Orchester MCP server*
//! (`super::server`), then the switches in three groups. *The window* opens with *Look at the window*, which every
//! client with the token may always do (`super::window_rows`), then *Agents can control this window* (read when the
//! app starts, so a change applies from the next start, as the line under it says). *Approvals and permissions* holds
//! *Let automation answer approvals* and *Let automation change agent permissions*; *The Orchestrator may* holds
//! *publish drafts*, *answer agents*, *merge runs*, *merge finished runs on its own* (ORC-69; it needs *merge runs* and
//! *answer agents* too, so it starts further in), *continue interrupted runs* and *write with git*
//! (`docs/design/orchestrator-toolset.md`, "Decisions" 3, 5 and 9). Both groups are the person's alone, as the mark
//! at their heading says. Each row says in one line what it allows; the rest, with what it needs and what it also
//! governs, is in its (i) tooltip. *Let Orchester estimate story points* is a project's own setting now, in Project
//! settings › Runs › Estimates.
//!
//! Each consent is an application setting that is off until a person turns it on here (window control: the build's
//! default until then). While *answer approvals* is off, the MCP server's approval tools refuse
//! (`automation_not_allowed`); while *change agent permissions* is off, its setup tools refuse whatever would widen
//! what agents may do (`person_only`), including matching window confirmations; while one of the Orchestrator's is
//! off, the Orchestrator chat proposes what it names to the person, or, for an interrupted or finished run, leaves it
//! to the person. A setting is read and saved off the UI thread; while either runs, its switch cannot be flipped. While
//! a row is mounted, the app's MCP server refuses window input (`mcp::input_gate`), and the row refuses lurq's
//! synthetic input, so only a person flips it, and says so under itself.

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    events::{InputSource, input_source},
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_store::StoreError;
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::{OrchestratorConsent, Workspace},
};

use super::{
  frame,
  save_state::{Outcome, SaveState},
  window_rows,
};
use crate::{
  mcp::{
    McpSettings,
    input_gate::{Guarded, InputGate, Protected},
  },
  services::Services,
  ui::{
    field::field_error,
    icon::Icon,
    message::message_text,
    setting_notice::status_line,
    setting_rows::{SettingGroup, SettingRow, marked_words},
    skeleton::{self, Skeleton},
    toggle::{Toggle, ToggleKind},
  },
};

/// A consent of what automation may do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
  /// *Agents can control this window*: what the app takes when it starts next.
  WindowControl,
  /// *Let automation answer approvals*.
  AnswerApprovals,
  /// *Let automation change agent permissions*.
  ChangePermissions,
  /// *Let the Orchestrator publish drafts*, *answer agents*, *merge runs*, *merge finished runs on its own*,
  /// *continue interrupted runs* or *write with git*.
  Orchestrator(OrchestratorConsent),
}

impl lurq::app::component::DevtoolsInspectable for Consent {}

/// The keys of what a consent's row says.
struct Words {
  title: &'static str,
  description: &'static str,
  unreadable: &'static str,
  not_saved: &'static str,
  blocked: &'static str,
}

impl Consent {
  /// The switch's element id, which its row's parts extend.
  const fn id(self) -> &'static str {
    match self {
      Self::WindowControl => "settings-window-control",
      Self::AnswerApprovals => "settings-answer-approvals",
      Self::ChangePermissions => "settings-change-permissions",
      Self::Orchestrator(OrchestratorConsent::PublishDrafts) => "settings-publish-drafts",
      Self::Orchestrator(OrchestratorConsent::AnswerAgents) => "settings-answer-agents",
      Self::Orchestrator(OrchestratorConsent::MergeRuns) => "settings-merge-runs",
      Self::Orchestrator(OrchestratorConsent::MergeFinished) => "settings-merge-finished",
      Self::Orchestrator(OrchestratorConsent::ContinueInterrupted) => "settings-continue-interrupted",
      Self::Orchestrator(OrchestratorConsent::WriteGit) => "settings-write-git",
    }
  }

  const fn words(self) -> Words {
    match self {
      Self::WindowControl => Words {
        title: "settings.automation.windowControl.title",
        description: "settings.automation.windowControl.description",
        unreadable: "settings.automation.windowControl.unreadable",
        not_saved: "settings.automation.windowControl.notSaved",
        blocked: "settings.automation.windowControl.blocked",
      },
      Self::AnswerApprovals => Words {
        title: "settings.automation.answerApprovals.title",
        description: "settings.automation.answerApprovals.description",
        unreadable: "settings.automation.answerApprovals.unreadable",
        not_saved: "settings.automation.answerApprovals.notSaved",
        blocked: "settings.automation.answerApprovals.blocked",
      },
      Self::ChangePermissions => Words {
        title: "settings.automation.changePermissions.title",
        description: "settings.automation.changePermissions.description",
        unreadable: "settings.automation.changePermissions.unreadable",
        not_saved: "settings.automation.changePermissions.notSaved",
        blocked: "settings.automation.changePermissions.blocked",
      },
      Self::Orchestrator(OrchestratorConsent::PublishDrafts) => Words {
        title: "settings.automation.publishDrafts.title",
        description: "settings.automation.publishDrafts.description",
        unreadable: "settings.automation.publishDrafts.unreadable",
        not_saved: "settings.automation.publishDrafts.notSaved",
        blocked: "settings.automation.publishDrafts.blocked",
      },
      Self::Orchestrator(OrchestratorConsent::AnswerAgents) => Words {
        title: "settings.automation.answerAgents.title",
        description: "settings.automation.answerAgents.description",
        unreadable: "settings.automation.answerAgents.unreadable",
        not_saved: "settings.automation.answerAgents.notSaved",
        blocked: "settings.automation.answerAgents.blocked",
      },
      Self::Orchestrator(OrchestratorConsent::MergeRuns) => Words {
        title: "settings.automation.mergeRuns.title",
        description: "settings.automation.mergeRuns.description",
        unreadable: "settings.automation.mergeRuns.unreadable",
        not_saved: "settings.automation.mergeRuns.notSaved",
        blocked: "settings.automation.mergeRuns.blocked",
      },
      Self::Orchestrator(OrchestratorConsent::MergeFinished) => Words {
        title: "settings.automation.mergeFinished.title",
        description: "settings.automation.mergeFinished.description",
        unreadable: "settings.automation.mergeFinished.unreadable",
        not_saved: "settings.automation.mergeFinished.notSaved",
        blocked: "settings.automation.mergeFinished.blocked",
      },
      Self::Orchestrator(OrchestratorConsent::ContinueInterrupted) => Words {
        title: "settings.automation.continueInterrupted.title",
        description: "settings.automation.continueInterrupted.description",
        unreadable: "settings.automation.continueInterrupted.unreadable",
        not_saved: "settings.automation.continueInterrupted.notSaved",
        blocked: "settings.automation.continueInterrupted.blocked",
      },
      Self::Orchestrator(OrchestratorConsent::WriteGit) => Words {
        title: "settings.automation.writeGit.title",
        description: "settings.automation.writeGit.description",
        unreadable: "settings.automation.writeGit.unreadable",
        not_saved: "settings.automation.writeGit.notSaved",
        blocked: "settings.automation.writeGit.blocked",
      },
    }
  }

  /// The keys of the row's label and the one line under it: the group's heading says the rest of its name (*The
  /// Orchestrator may* · *Publish drafts*).
  const fn short(self) -> (&'static str, &'static str) {
    match self {
      Self::WindowControl => (
        "settings.automation.windowControl.title",
        "settings.automation.windowControl.hint",
      ),
      Self::AnswerApprovals => (
        "settings.automation.answerApprovals.title",
        "settings.automation.answerApprovals.hint",
      ),
      Self::ChangePermissions => (
        "settings.automation.changePermissions.title",
        "settings.automation.changePermissions.hint",
      ),
      Self::Orchestrator(OrchestratorConsent::PublishDrafts) => (
        "settings.automation.publishDrafts.label",
        "settings.automation.publishDrafts.hint",
      ),
      Self::Orchestrator(OrchestratorConsent::AnswerAgents) => (
        "settings.automation.answerAgents.label",
        "settings.automation.answerAgents.hint",
      ),
      Self::Orchestrator(OrchestratorConsent::MergeRuns) => (
        "settings.automation.mergeRuns.label",
        "settings.automation.mergeRuns.hint",
      ),
      Self::Orchestrator(OrchestratorConsent::MergeFinished) => (
        "settings.automation.mergeFinished.label",
        "settings.automation.mergeFinished.hint",
      ),
      Self::Orchestrator(OrchestratorConsent::ContinueInterrupted) => (
        "settings.automation.continueInterrupted.label",
        "settings.automation.continueInterrupted.hint",
      ),
      Self::Orchestrator(OrchestratorConsent::WriteGit) => (
        "settings.automation.writeGit.label",
        "settings.automation.writeGit.hint",
      ),
    }
  }

  /// Its name in the developer log.
  const fn logged(self) -> &'static str {
    match self {
      Self::WindowControl => "Agents can control this window",
      Self::AnswerApprovals => "Let automation answer approvals",
      Self::ChangePermissions => "Let automation change agent permissions",
      Self::Orchestrator(OrchestratorConsent::PublishDrafts) => "Let the Orchestrator publish drafts",
      Self::Orchestrator(OrchestratorConsent::AnswerAgents) => "Let the Orchestrator answer agents",
      Self::Orchestrator(OrchestratorConsent::MergeRuns) => "Let the Orchestrator merge runs",
      Self::Orchestrator(OrchestratorConsent::MergeFinished) => "Let the Orchestrator merge finished runs on its own",
      Self::Orchestrator(OrchestratorConsent::ContinueInterrupted) => "Let the Orchestrator continue interrupted runs",
      Self::Orchestrator(OrchestratorConsent::WriteGit) => "Let the Orchestrator write with git",
    }
  }

  /// Reads it: for window control, what the next start takes. Blocks on SQLite.
  fn read(self, workspace: &Workspace) -> Result<bool, StoreError> {
    match self {
      Self::WindowControl => workspace.mcp_window_control().map(McpSettings::next_start),
      Self::AnswerApprovals => workspace.automation_answers_approvals(),
      Self::ChangePermissions => workspace.automation_changes_permissions(),
      // Application-wide until ORC-599 and ORC-600 move these rows to Project settings › Orchestrator.
      Self::Orchestrator(consent) => workspace.app_wide_orchestrator_allows(consent),
    }
  }

  /// Saves it. Blocks on SQLite.
  fn save(self, workspace: &Workspace, allowed: bool) -> Result<(), StoreError> {
    match self {
      Self::WindowControl => workspace.set_mcp_window_control(allowed),
      Self::AnswerApprovals => workspace.set_automation_answers_approvals(allowed),
      Self::ChangePermissions => workspace.set_automation_changes_permissions(allowed),
      Self::Orchestrator(consent) => workspace.set_app_wide_orchestrator_allows(consent, allowed),
    }
  }
}

/// The tab, scrolling under the header: the MCP server's group, asking *Rotate the token?* when `rotation`, then the
/// switches in their three groups.
pub fn tab(ctx: &mut Ctx, rotation: bool) -> Element {
  let mut row = |key: &str, consent: Consent| ctx.mount_keyed::<ConsentRow>(key, consent);
  let window = [row("window-control", Consent::WindowControl)];
  let approvals = [
    row("answer-approvals", Consent::AnswerApprovals),
    row("change-permissions", Consent::ChangePermissions),
  ];
  let orchestrator = [
    row(
      "publish-drafts",
      Consent::Orchestrator(OrchestratorConsent::PublishDrafts),
    ),
    row(
      "answer-agents",
      Consent::Orchestrator(OrchestratorConsent::AnswerAgents),
    ),
    row("merge-runs", Consent::Orchestrator(OrchestratorConsent::MergeRuns)),
    row(
      "merge-finished",
      Consent::Orchestrator(OrchestratorConsent::MergeFinished),
    ),
    row(
      "continue-interrupted",
      Consent::Orchestrator(OrchestratorConsent::ContinueInterrupted),
    ),
    row("write-git", Consent::Orchestrator(OrchestratorConsent::WriteGit)),
  ];
  let only_you = |ctx: &Ctx, id: &str| marked_words(id, Icon::Hand, &ctx.t("settings.automation.onlyYou"));
  let look = window_rows::look_at_the_window(ctx);
  let pane = frame::pane("settings-automation")
    .child(ctx.mount_keyed::<super::server::ServerSection>("mcp-server", super::server::Props { rotation }))
    .child(
      SettingGroup::new("settings-window", ctx.t("settings.automation.window"))
        .row(look)
        .rows(window),
    )
    .child(
      SettingGroup::new("settings-approvals", ctx.t("settings.automation.approvals"))
        .end(only_you(ctx, "settings-approvals-only-you"))
        .rows(approvals),
    )
    .child(
      SettingGroup::new(
        "settings-orchestrator-may",
        ctx.t("settings.automation.orchestratorMay"),
      )
      .end(only_you(ctx, "settings-orchestrator-may-only-you"))
      .rows(orchestrator),
    );
  frame::column("settings-automation-scroll", pane)
}

type SaveAction = FutureAction<bool, bool, Message>;

pub struct ConsentRow {
  /// The setting as the person last chose it; `None` until they do.
  saved: Signal<Option<bool>>,
  /// A change the row did not take because an MCP client's input brought it.
  blocked: Signal<bool>,
  /// While the row is mounted, the MCP server refuses window input.
  _protected: Protected,
}

impl Component for ConsentRow {
  type Props = Consent;

  fn create(ctx: &mut Ctx) -> Self {
    let gate = ctx
      .use_context::<InputGate>()
      .expect("the session's window provides the MCP input gate");
    Self {
      saved: ctx.signal(None),
      blocked: ctx.signal(false),
      _protected: gate.protect(Guarded::PermissionsSetting),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let consent = *ctx.props::<Consent>();
    let (id, words) = (consent.id(), consent.words());
    let workspace = Services::of(ctx).workspace;
    // Hooks in the same order on every render: the action, then the read, which follows each save.
    let save = save_action(ctx, &workspace, consent);
    let state = save.state().get();
    let read = stored(ctx, &workspace, (consent, state.data));
    let fixed = state.is_pending() || read.is_pending();
    // The person's choice once it saved; the stored value until then, or after a save failed.
    let shown = match (&state.error, self.saved.get()) {
      (None, Some(chosen)) => Some(chosen),
      _ => read.data,
    };
    let problem = problem(ctx, consent, (&read.error, &state.error));
    // The row takes no change before it lived through one pass after mounting: an input meant for what
    // the window showed before cannot land on it.
    let armed = ctx
      .future((), |()| async { Ok::<(), Message>(()) })
      .state()
      .get()
      .data
      .is_some();
    let title = ctx.t(words.title);
    let switch = Toggle::new(id, ToggleKind::Switch, shown.unwrap_or(false), title).hide_label();
    // Until the stored value is known, and while it is read or saved, the switch stays as it is.
    let switch = if shown.is_some() && !fixed && armed {
      self.listen(switch, save, (workspace.clone(), consent))
    } else {
      switch
    };
    let blocked = self
      .blocked
      .get()
      .then(|| field_error(&format!("{id}-blocked"), &ctx.t(words.blocked)).into());
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(id, Outcome::of(&state));
    }
    let (label, hint) = consent.short();
    // What the switch it shows means, then why it cannot be read, was not saved or did not take a change, under the
    // row (`Blocked` `aYFot` in `shpsk`).
    let below = said(ctx, consent, shown)
      .into_iter()
      .chain(problem.map(Element::from))
      .chain(blocked);
    // Window control's one line says what this run has and when a change applies (its eight variants,
    // `window_rows::window_control_hint`), once the stored choice is known.
    let hint = match (consent, shown) {
      (Consent::WindowControl, Some(chosen)) => Some(window_rows::window_control_hint(ctx, chosen)),
      (Consent::WindowControl, None) => None,
      _ => Some(ctx.t(hint).to_string()),
    };
    // While the stored choice is read, the switch is a bone of its size (`15 · App settings · Automation · loading`).
    let control: Element = if shown.is_none() && read.is_pending() {
      Skeleton::new(format!("{id}-reading"), ctx.t("settings.reading"))
        .inline()
        .show(ctx, skeleton::switch())
    } else {
      switch.into()
    };
    let row = SettingRow::new(id, ctx.t(label))
      .info(ctx.t(words.description))
      .control(control)
      .below_all(below);
    let row = match hint {
      Some(hint) => row.hint(hint),
      None => row,
    };
    let row = if consent == Consent::Orchestrator(OrchestratorConsent::MergeFinished) {
      row.dependent()
    } else {
      row
    };
    row.render(ctx)
  }
}

/// What a row says under itself about the switch it shows: what the Orchestrator does once it may publish, and how
/// automation's answers read once it may answer.
fn said(ctx: &Ctx, consent: Consent, shown: Option<bool>) -> Option<Element> {
  let (id, glyph, key) = match (consent, shown) {
    (Consent::Orchestrator(OrchestratorConsent::PublishDrafts), Some(true)) => (
      "settings-publish-drafts-on",
      Icon::MessagesSquare,
      "settings.automation.publishDrafts.on",
    ),
    (Consent::AnswerApprovals, Some(true)) => (
      "settings-answer-approvals-on",
      Icon::ListChecks,
      "settings.automation.answerApprovals.on",
    ),
    _ => return None,
  };
  Some(status_line(id, glyph, &ctx.t(key)).into())
}

impl ConsentRow {
  /// `switch`, saving what a person chooses. Synthetic input never changes this human-only control; its source remains
  /// known even after the caller stops waiting.
  fn listen(&self, switch: Toggle, save: SaveAction, (workspace, consent): (Workspace, Consent)) -> Toggle {
    let (saved, blocked) = (self.saved.clone(), self.blocked.clone());
    switch.on_change(move |chosen| {
      if input_source() == InputSource::Synthetic {
        tracing::warn!(
          setting = consent.logged(),
          "synthetic input reached a person's setting; not taken"
        );
        blocked.set(true);
        return;
      }
      blocked.set(false);
      if !chosen {
        // Off holds from the person's click, before the save runs and if it fails.
        match consent {
          Consent::AnswerApprovals => workspace.revoke_automation_approval_consent(),
          Consent::ChangePermissions => workspace.revoke_automation_permission_consent(),
          Consent::Orchestrator(consent) => workspace.revoke_app_wide_orchestrator_consent(consent),
          // Read when the app starts: this run keeps what it started with, as the row's hint says.
          Consent::WindowControl => {}
        }
      }
      saved.set(Some(chosen));
      save.run(chosen);
    })
  }
}

/// Why `consent` cannot be read or was not saved, under the row's description.
fn problem(ctx: &Ctx, consent: Consent, (read, save): (&Option<Message>, &Option<Message>)) -> Option<Row> {
  let words = consent.words();
  let (key, reason) = match (read, save) {
    (Some(reason), _) => (words.unreadable, reason),
    (None, Some(reason)) => (words.not_saved, reason),
    (None, None) => return None,
  };
  let reason = message_text(ctx, reason);
  Some(field_error(
    &format!("{}-error", consent.id()),
    &ctx.t_args(key, [("reason", &*reason)]),
  ))
}

/// Saves the `consent` the person chose.
fn save_action(ctx: &mut Ctx, workspace: &Workspace, consent: Consent) -> SaveAction {
  let workspace = workspace.clone();
  ctx.future_action(move |allowed: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || consent.save(&workspace, allowed))
        .await?
        .map(|()| allowed)
        .map_err(|error| {
          tracing::error!(%error, setting = consent.logged(), "a person's setting was not saved");
          messages::store_error(&error)
        })
    }
  })
}

/// The `consent` as stored, read when the row shows and again after each save (`saved`).
fn stored(
  ctx: &mut Ctx,
  workspace: &Workspace,
  (consent, saved): (Consent, Option<bool>),
) -> FutureState<bool, Message> {
  let (workspace, hold) = (workspace.clone(), Services::of(ctx).hold);
  let read = ctx.future(saved, move |_| {
    let (workspace, hold) = (workspace.clone(), hold.clone());
    async move {
      hold.released().await;
      background::run(move || consent.read(&workspace))
        .await?
        .map_err(|error| {
          tracing::error!(%error, setting = consent.logged(), "a person's setting cannot be read");
          messages::store_error(&error)
        })
    }
  });
  read.state().get()
}
