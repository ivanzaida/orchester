//! *What automation may do* (`Form section title` `De1nE`) with a row (`Form row` `XaNIz` with a switch) per consent
//! only a person gives: *Let automation change agent permissions*, *Let the Orchestrator publish drafts*, *Let the
//! Orchestrator answer agents* and *Let the Orchestrator merge runs* (the same row;
//! `docs/design/orchestrator-toolset.md`, "Decisions" 3 and 5). Each is an application setting that is off until a
//! person turns it on here. While the first is off, the MCP server's setup tools refuse whatever would widen what
//! agents may do (`person_only`), including matching window confirmations; while one of the Orchestrator's is off, the
//! Orchestrator chat proposes what it names to the person. A setting is read and saved off the UI thread; while either
//! runs, its switch cannot be flipped. While a row is mounted, the app's MCP server refuses window input
//! (`mcp::input_gate`), and the row refuses lurq's synthetic input, so only a person flips it.

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

use crate::{
  mcp::input_gate::{Guarded, InputGate, Protected},
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
  /// *Let automation change agent permissions*.
  ChangePermissions,
  /// *Let the Orchestrator publish drafts*, *answer agents* or *merge runs*.
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
      Self::ChangePermissions => "settings-change-permissions",
      Self::Orchestrator(OrchestratorConsent::PublishDrafts) => "settings-publish-drafts",
      Self::Orchestrator(OrchestratorConsent::AnswerAgents) => "settings-answer-agents",
      Self::Orchestrator(OrchestratorConsent::MergeRuns) => "settings-merge-runs",
    }
  }

  const fn words(self) -> Words {
    match self {
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
    }
  }

  /// Its name in the developer log.
  const fn logged(self) -> &'static str {
    match self {
      Self::ChangePermissions => "Let automation change agent permissions",
      Self::Orchestrator(OrchestratorConsent::PublishDrafts) => "Let the Orchestrator publish drafts",
      Self::Orchestrator(OrchestratorConsent::AnswerAgents) => "Let the Orchestrator answer agents",
      Self::Orchestrator(OrchestratorConsent::MergeRuns) => "Let the Orchestrator merge runs",
    }
  }

  /// Reads it. Blocks on SQLite.
  fn read(self, workspace: &Workspace) -> Result<bool, StoreError> {
    match self {
      Self::ChangePermissions => workspace.automation_changes_permissions(),
      Self::Orchestrator(consent) => workspace.orchestrator_allows(consent),
    }
  }

  /// Saves it. Blocks on SQLite.
  fn save(self, workspace: &Workspace, allowed: bool) -> Result<(), StoreError> {
    match self {
      Self::ChangePermissions => workspace.set_automation_changes_permissions(allowed),
      Self::Orchestrator(consent) => workspace.set_orchestrator_allows(consent, allowed),
    }
  }
}

/// The section: its title over a row per consent.
pub fn section(ctx: &mut Ctx) -> Element {
  let rows = [
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
  ];
  let rows: Vec<Element> = rows
    .into_iter()
    .map(|(key, consent)| ctx.mount_keyed::<ConsentRow>(key, consent))
    .collect();
  Column::new()
    .id("settings-automation")
    .width(Dimension::Pct(100.0))
    .child(section_title(&ctx.t("settings.automation.section")))
    .with_children(rows)
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
    // What a switch that is on says beside it (`Hint · publishes directly` `Bj5aS` in `IJOOq`).
    let publishes = consent == Consent::Orchestrator(OrchestratorConsent::PublishDrafts);
    let on = (publishes && shown == Some(true)).then(|| {
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding_bottom(ExtraSpacing::Snug.role())
        .child(note(
          &format!("{id}-on"),
          Icon::MessagesSquare,
          &ctx.t("settings.automation.publishDrafts.on"),
        ))
    });
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
      .with_children(on)
      .with_children(errors)
  }
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
          Consent::ChangePermissions => workspace.revoke_automation_permission_consent(),
          Consent::Orchestrator(consent) => workspace.revoke_orchestrator_consent(consent),
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
