//! Decision model servers in Settings › Accounts (`docs/design/agent-accounts.md`, "Agents run on accounts", decision
//! 3; the person, 2026-11-14: they stand under *Model APIs* with the APIs of Orchester's own agent). A server is added
//! on the *Add an API provider* page (its base URL and, where it takes one, its key, which the app keeps), and listed
//! with its host, the models its last check listed and how that check went; *Check* asks it for its models again and
//! *Remove* takes it and its key away, after a question. Every project then has it to review with (Project settings ›
//! Runs).
//!
//! Not here yet: renaming a server, another base URL or key for it, and its test, which runs in a project.

use lurq::{
  app::{
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_core::{
  connection::{DecisionCheckResult, DecisionConnection, DecisionServerUrl},
  id::DecisionConnectionId,
  name::Name,
  secret::SecretValue,
};
use orchester_workspace::{background, messages::Message};

use super::words::{t, ta};
use crate::{
  screens::project_settings::kit::FormField,
  services::Services,
  theme::{IconSize, ProjectSettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    dialog::Dialog,
    field::FieldInput,
    icon::{Icon, icon},
    setting_rows::{SettingRow, row_tile, status},
  },
};

/// What *Add provider* adds for a decision model: its name, where it answers, and its key when it takes one.
pub(super) type NewServer = (Name, DecisionServerUrl, Option<SecretValue>);

/// The decision servers' part of the tab: the base URL typed for a new one, and what its rows do.
#[derive(Clone)]
pub(super) struct Servers {
  /// The base URL of the server being added, as typed.
  pub url: Signal<String>,
  pub check: FutureAction<DecisionConnectionId, (), Message>,
  pub remove: FutureAction<DecisionConnectionId, (), Message>,
  /// The server *Remove* asks about, by its id as text; empty while it asks about none.
  asked: Signal<String>,
  /// Whether the question shows.
  asking: Signal<bool>,
}

impl Servers {
  pub fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace.clone(), services.vault.clone());
    let check = ctx.future_action(move |id: DecisionConnectionId| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        workspace
          .check_decision_server(vault.get(), id)
          .await
          .map(|_| ())
          .map_err(|error| error.message())
      }
    });
    let (workspace, vault) = (services.workspace, services.vault);
    let remove = ctx.future_action(move |id: DecisionConnectionId| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        background::run(move || {
          workspace
            .remove_decision_server(vault.get(), id)
            .map_err(|error| error.message())
        })
        .await?
      }
    });
    Self {
      url: ctx.signal(String::new()),
      check,
      remove,
      asked: ctx.signal(String::new()),
      asking: ctx.signal(false),
    }
  }

  /// The base URL typed, once it is one.
  pub fn typed_url(&self) -> Option<DecisionServerUrl> {
    DecisionServerUrl::new(&self.url.get()).ok()
  }

  /// *Base URL* of the server being added.
  pub fn url_row(&self, ctx: &mut Ctx) -> Element {
    let label = t(ctx, "accounts.api.decision.url");
    let field = FieldInput::new("api-add-decision-url", label.clone(), self.url.clone())
      .placeholder(t(ctx, "accounts.api.decision.urlPlaceholder"))
      .mono()
      .render(ctx);
    FormField::new("api-add-decision-url-row", label)
      .hint(t(ctx, "accounts.api.decision.urlHint"))
      .render(ctx, field)
  }

  /// The rows of `servers` in *Model APIs*: each with its host and models, how its last check went, *Check* and
  /// *Remove*.
  pub fn rows(&self, ctx: &mut Ctx, servers: &[DecisionConnection]) -> Vec<Element> {
    let busy = self.check.state().get().is_pending() || self.remove.state().get().is_pending();
    servers.iter().map(|server| self.row(ctx, server, busy)).collect()
  }

  fn row(&self, ctx: &mut Ctx, server: &DecisionConnection, busy: bool) -> Element {
    let id = format!("decision-server-{}", server.id());
    let count = server.models().len();
    let key = match count {
      0 => "accounts.api.decision.lineNone",
      1 => "accounts.api.decision.lineOne",
      _ => "accounts.api.decision.line",
    };
    let line = ta(
      ctx,
      key,
      [
        ("host", server.base_url().host().to_owned()),
        ("count", count.to_string()),
      ],
    );
    let (colors, words) = state(server);
    let (check, server_id) = (self.check.clone(), server.id());
    let check_button = ActionButton::new(
      format!("{id}-check"),
      ButtonKind::Ghost,
      t(ctx, "accounts.api.decision.check"),
    )
    .icon(Icon::RefreshCw)
    .small();
    let check_button = if busy {
      check_button.disabled()
    } else {
      check_button.on_click(move || check.run(server_id))
    };
    let (asked, asking) = (self.asked.clone(), self.asking.clone());
    let remove_button = ActionButton::new(
      format!("{id}-remove"),
      ButtonKind::Ghost,
      t(ctx, "accounts.api.decision.remove"),
    )
    .icon(Icon::Trash2)
    .small();
    let remove_button = if busy {
      remove_button.disabled()
    } else {
      remove_button.on_click(move || {
        asked.set(server_id.to_string());
        asking.set(true);
      })
    };
    let end = Row::new()
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .child(status(&format!("{id}-state"), colors, &t(ctx, words)))
      .child(check_button)
      .child(remove_button);
    let glyph = if server.is_on_this_computer() {
      Icon::Monitor
    } else {
      Icon::Scale
    };
    SettingRow::new(id, server.name().as_str().to_owned())
      .lead(row_tile(icon(glyph, IconSize::Md, PaletteColor::TextSecondary)))
      .hint(line)
      .control(end)
      .render(ctx)
  }

  /// *Remove …?* over the window, for the server asked about; nothing while none is.
  pub fn question(&self, ctx: &Ctx, servers: &[DecisionConnection]) -> Option<Element> {
    let asked = self.asked.get();
    let server = servers.iter().find(|server| server.id().to_string() == asked)?;
    let (remove, close, id) = (self.remove.clone(), self.asking.clone(), server.id());
    let confirm = ActionButton::new(
      "decision-server-remove-confirm",
      ButtonKind::Danger,
      t(ctx, "accounts.api.decision.remove"),
    )
    .on_click(move || {
      close.set(false);
      remove.run(id);
    });
    Some(
      Dialog::new(
        "decision-server-remove-dialog",
        Icon::Trash2,
        ta(
          ctx,
          "accounts.api.decision.removeTitle",
          [("name", server.name().as_str().to_owned())],
        ),
        t(ctx, "accounts.add.cancel"),
      )
      .part(Text::new(&t(ctx, "accounts.api.decision.removeBody")).variant(ProjectSettingsText::Hint.role()))
      .action(confirm)
      .render(ctx, &self.asking),
    )
  }
}

/// How the server's last check went, as its row says it: the colors of its dot and word, and the word's key.
fn state(server: &DecisionConnection) -> ((PaletteColor, PaletteColor), &'static str) {
  let quiet = (PaletteColor::TextMuted, PaletteColor::TextSecondary);
  let warns = (PaletteColor::Warning, PaletteColor::Warning);
  match server.last_check().map(|check| &check.result) {
    None => (quiet, "accounts.api.decision.state.notChecked"),
    Some(DecisionCheckResult::Connected | DecisionCheckResult::ModelGone { .. }) => (
      (PaletteColor::Success, PaletteColor::TextSecondary),
      "accounts.api.decision.state.connected",
    ),
    Some(DecisionCheckResult::KeyRefused) => (warns, "accounts.api.decision.state.keyRefused"),
    Some(DecisionCheckResult::NothingAnswers) => (warns, "accounts.api.decision.state.nothingAnswers"),
    Some(DecisionCheckResult::NotSystemOneApi) => (warns, "accounts.api.decision.state.notDecisionApi"),
  }
}
