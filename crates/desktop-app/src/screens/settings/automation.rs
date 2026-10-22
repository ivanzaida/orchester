//! *What automation may do* (`Form section title` `De1nE` in `oD7hy`, `lye6f`) opens with *Look at the window*, which
//! every client with the token may always do (`super::window_rows`), then a row (`Form row` `XaNIz` with a switch) per
//! consent only a person gives: *Agents can control this window* (read when the app starts, so a change applies from
//! the next start, as the hint under it says), *Let automation answer approvals*, *Let automation change agent
//! permissions*, *Let the Orchestrator publish drafts*, *Let the Orchestrator answer agents*, *Let the Orchestrator
//! merge runs*, *Let the Orchestrator merge finished runs on its own* (ORC-69; its description says it needs *merge
//! runs* and *answer agents* too) and *Let the Orchestrator continue interrupted runs* (the same row;
//! `docs/design/orchestrator-toolset.md`, "Decisions" 3 and 5). Each is an application setting that is off until a
//! person turns it on here (window control: the build's default until then). While *answer approvals* is off, the MCP
//! server's approval tools refuse (`automation_not_allowed`); while *change agent permissions* is off, its setup tools
//! refuse whatever would widen what agents may do (`person_only`), including matching window confirmations; while one
//! of the Orchestrator's is off, the Orchestrator chat proposes what it names to the person, or, for an interrupted or
//! finished run, leaves it to the person. *Let Orchester estimate story points* (ORC-49, same row, no design frame
//! yet) lets Orchester's estimator size tasks on auto mode's reviewer, spending tokens. A setting is read and saved off
//! the UI thread; while either runs, its switch cannot be flipped. While a row is mounted, the app's MCP server refuses
//! window input (`mcp::input_gate`), and the row refuses lurq's synthetic input, so only a person flips it. The section
//! ends with the Orchestrator's daily report and its time (`super::daily_report`).

use lurq::{
  app::{
    component::Component,
    ctx::{Ctx, FutureAction, FutureState},
    events::{InputSource, input_source},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::StoreError;
use orchester_workspace::{
  background,
  messages::{self, Message},
  workspace::{OrchestratorConsent, Workspace},
};

use super::{
  save_state::{Outcome, SaveState},
  window_rows,
};
use crate::{
  mcp::{
    McpSettings,
    input_gate::{Guarded, InputGate, Protected},
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, SettingsSpacing, SettingsText},
  ui::{
    field::field_error,
    icon::Icon,
    inspector::note,
    message::message_text,
    toggle::{Toggle, ToggleKind},
  },
};

/// A consent of *What automation may do*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Consent {
  /// *Agents can control this window*: what the app takes when it starts next.
  WindowControl,
  /// *Let automation answer approvals*.
  AnswerApprovals,
  /// *Let automation change agent permissions*.
  ChangePermissions,
  /// *Let the Orchestrator publish drafts*, *answer agents*, *merge runs* or *continue interrupted runs*.
  Orchestrator(OrchestratorConsent),
  /// *Let Orchester estimate story points* (ORC-49): off until a person turns it on.
  EstimatePoints,
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
      Self::EstimatePoints => "settings-estimate-points",
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
      Self::EstimatePoints => Words {
        title: "settings.automation.estimatePoints.title",
        description: "settings.automation.estimatePoints.description",
        unreadable: "settings.automation.estimatePoints.unreadable",
        not_saved: "settings.automation.estimatePoints.notSaved",
        blocked: "settings.automation.estimatePoints.blocked",
      },
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
      Self::EstimatePoints => "Let Orchester estimate story points",
    }
  }

  /// Reads it: for window control, what the next start takes. Blocks on SQLite.
  fn read(self, workspace: &Workspace) -> Result<bool, StoreError> {
    match self {
      Self::WindowControl => workspace.mcp_window_control().map(McpSettings::next_start),
      Self::AnswerApprovals => workspace.automation_answers_approvals(),
      Self::ChangePermissions => workspace.automation_changes_permissions(),
      Self::Orchestrator(consent) => workspace.orchestrator_allows(consent),
      Self::EstimatePoints => workspace.estimates_story_points(),
    }
  }

  /// Saves it. Blocks on SQLite.
  fn save(self, workspace: &Workspace, allowed: bool) -> Result<(), StoreError> {
    match self {
      Self::WindowControl => workspace.set_mcp_window_control(allowed),
      Self::AnswerApprovals => workspace.set_automation_answers_approvals(allowed),
      Self::ChangePermissions => workspace.set_automation_changes_permissions(allowed),
      Self::Orchestrator(consent) => workspace.set_orchestrator_allows(consent, allowed),
      Self::EstimatePoints => workspace.set_estimates_story_points(allowed),
    }
  }
}

/// The section: its title, what automation may always do, and a row per consent.
pub fn section(ctx: &mut Ctx) -> Element {
  let rows = [
    ("window-control", Consent::WindowControl),
    ("answer-approvals", Consent::AnswerApprovals),
    ("change-permissions", Consent::ChangePermissions),
    (
      "publish-drafts",
      Consent::Orchestrator(OrchestratorConsent::PublishDrafts),
    ),
    (
      "answer-agents",
      Consent::Orchestrator(OrchestratorConsent::AnswerAgents),
    ),
    ("merge-runs", Consent::Orchestrator(OrchestratorConsent::MergeRuns)),
    (
      "merge-finished",
      Consent::Orchestrator(OrchestratorConsent::MergeFinished),
    ),
    (
      "continue-interrupted",
      Consent::Orchestrator(OrchestratorConsent::ContinueInterrupted),
    ),
    ("estimate-points", Consent::EstimatePoints),
  ];
  let rows: Vec<Element> = rows
    .into_iter()
    .map(|(key, consent)| ctx.mount_keyed::<ConsentRow>(key, consent))
    .collect();
  // The daily report's time is the section's last row (`super::daily_report`).
  let report = ctx.mount_keyed::<super::daily_report::DailyReportSection>("daily-report", ());
  Column::new()
    .id("settings-automation")
    .width(Dimension::Pct(100.0))
    .child(section_title(&ctx.t("settings.automation.section")))
    .child(window_rows::look_at_the_window(ctx))
    .with_children(rows)
    .child(report)
    .into()
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
    let switch = Toggle::new(id, ToggleKind::Switch, shown.unwrap_or(false), title.clone()).hide_label();
    // Until the stored value is known, and while it is read or saved, the switch stays as it is.
    let switch = if shown.is_some() && !fixed && armed {
      self.listen(switch, save, (workspace.clone(), consent))
    } else {
      switch
    };
    let blocked = self
      .blocked
      .get()
      .then(|| field_error(&format!("{id}-blocked"), &ctx.t(words.blocked)));
    if let Some(saves) = SaveState::of(ctx) {
      saves.report(id, Outcome::of(&state));
    }
    let hint = hint(ctx, consent, shown);
    // Why it cannot be read, was not saved or did not take a change, under the row (`Error · not taken` `WYp6D`).
    let errors = (problem.is_some() || blocked.is_some()).then(|| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_bottom(SpacingSize::Sm)
        .spacing(ExtraSpacing::Snug.role())
        .with_children(problem)
        .with_children(blocked)
    });
    let description = ctx.t(words.description);
    Column::new()
      .width(Dimension::Pct(100.0))
      .child(row(id, (&title, &description), switch))
      .with_children(hint)
      .with_children(errors)
  }
}

/// What a row says under its line about the switch it shows: what the Orchestrator does once it may publish (`Hint ·
/// publishes directly` `Bj5aS` in `IJOOq`), how automation's answers read once it may answer (`O0z7RK` in `HNQBR`),
/// and when window control applies (`Hint` `iqm0X` in `oD7hy`).
fn hint(ctx: &mut Ctx, consent: Consent, shown: Option<bool>) -> Option<Column> {
  let (id, glyph, words) = match (consent, shown) {
    (Consent::Orchestrator(OrchestratorConsent::PublishDrafts), Some(true)) => (
      "settings-publish-drafts-on",
      Icon::MessagesSquare,
      ctx.t("settings.automation.publishDrafts.on").to_string(),
    ),
    (Consent::AnswerApprovals, Some(true)) => (
      "settings-answer-approvals-on",
      Icon::ListChecks,
      ctx.t("settings.automation.answerApprovals.on").to_string(),
    ),
    (Consent::WindowControl, Some(chosen)) => (
      "settings-window-control-hint",
      Icon::Info,
      window_rows::window_control_hint(ctx, chosen),
    ),
    _ => return None,
  };
  Some(
    Column::new()
      .width(Dimension::Pct(100.0))
      .padding_bottom(ExtraSpacing::Snug.role())
      .child(note(id, glyph, &words)),
  )
}

/// The row of switch `id`: its title and description, and the switch at its end, over its line (`Form row` `XaNIz`).
pub(super) fn row(id: &str, (title, description): (&str, &str), switch: Toggle) -> Row {
  Row::new()
    .id(format!("{id}-row"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::vertical(SpacingSize::Md))
    .spacing(SpacingSize::Xl)
    .align_items(Alignment::Center)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Fine.role())
        .child(Text::new(title).variant(ExtraTypography::Strong.role()))
        .child(
          Text::new(description)
            .variant(ExtraTypography::Prose.role())
            .width(Dimension::Pct(100.0)),
        ),
    )
    .child(switch)
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
          Consent::Orchestrator(consent) => workspace.revoke_orchestrator_consent(consent),
          // Read as stored before each estimate: off holds once it is saved.
          Consent::EstimatePoints => {}
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

/// A form section's title over its line.
pub(super) fn section_title(title: &str) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding_bottom(SettingsSpacing::SectionTitle.role())
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(Text::new(title).variant(SettingsText::SectionTitle.role()))
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
  let workspace = workspace.clone();
  let read = ctx.future(saved, move |_| {
    let workspace = workspace.clone();
    async move {
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
