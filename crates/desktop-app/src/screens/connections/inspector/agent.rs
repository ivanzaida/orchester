//! An agent provider's inspector. A CLI (`VIpsx`, the states of `uBsQM`): its provider, program file and what
//! starts it, version, where it runs, how it signs in and when it was checked; the models its check listed,
//! a model it is too old for (`Claude Code 2.1.89 can't run …`); the profiles that run on it; and in the
//! footer what its status means, the sign-in command to run in a terminal, Test connection and Edit
//! connection. A native connection (`cC6TI`) says that native agents are not available yet and shows its
//! endpoint, key and models, its test and editing unavailable. A CLI on an API provider shows the provider
//! ([`super::provider`]) instead of an account.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Row, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{
  AgentConnection, AgentProvider, ApiEndpoint, CatalogSource, ConnectionStatus, EndpointPreset,
};
use orchester_exec::launch::{Launch, Via};
use orchester_workspace::{connections::data::sign_in_command, display};

use super::{Actions, Inspecting, Parts, check_button, footer, footer_action, header, last_checked, shown, used_by};
use crate::{
  screens::connections::words::{agent_glyph, checked_version, connection_status, provider_key},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    chip::{BadgeTone, TagChip, badge},
    code_box::CodeBox,
    icon::Icon,
    inspector::{self, PropertyValue, property},
    notice::{Notice, NoticeTone},
    unavailable::reason_line,
  },
};

/// The longest path a property shows in full (`C:\…\npm\claude.cmd`).
const PATH_CHARS: usize = 26;
/// Product names of the programs that start a launcher script; identifiers, not translated.
const NODE: &str = "Node.js";
const CMD: &str = "cmd.exe";

pub(super) fn parts(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
  let status = shown(
    inspecting,
    connection_status(connection, connection.status_with(&inspecting.listed.secrets)),
  );
  let header = header(
    ctx,
    (agent_glyph(connection.provider()), connection.name().as_str()),
    status,
    None,
  );
  let uses: Vec<String> = inspecting
    .listed
    .used_by(connection.id())
    .map(|used| {
      ctx
        .t_args(
          "connections.usedByLink",
          [("role", &*used.role), ("model", used.model.as_str())],
        )
        .to_string()
    })
    .collect();
  let used = used_by(ctx, uses, actions, "connections.notUsed");
  match connection.api() {
    Some(api) => native(ctx, connection, api, (header, used), inspecting),
    None => cli(ctx, connection, (header, used), inspecting, actions),
  }
}

fn cli(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  (header, used): (Element, Element),
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
  if let Some(api) = connection.api_provider() {
    return super::provider::parts(ctx, (connection, api), (header, used), inspecting, actions);
  }
  let mut body: Vec<Element> = too_old(ctx, connection).into_iter().collect();
  body.push(properties(ctx, connection, inspecting).into());
  body.push(super::super::account::inspector_rows(
    ctx,
    inspecting.accounts,
    connection,
  ));
  body.push(models(ctx, connection, inspecting, actions));
  body.push(used);
  let status = connection.status_with(&inspecting.listed.secrets);
  let account = inspecting
    .accounts
    .of_connection(connection.account(), connection.provider());
  // Edit connection chooses the connection's account (`M42rM`); its name and program are not edited here yet.
  let (edit, id) = (actions.edit.clone(), connection.id());
  let edit = footer_action(
    inspecting,
    ActionButton::new("connection-edit", ButtonKind::Secondary, ctx.t("connections.edit")),
    Icon::Pencil,
  )
  .opens(edit(id));
  // On no account: a check would run the CLI on Default's folder and say nothing of this connection; Edit connection
  // chooses one.
  if connection.account() == &orchester_core::connection::ConnectionAccount::Unassigned {
    let sentence = ctx.t("connections.detail.noAccount");
    return Parts {
      header,
      body,
      footer: Some(footer(
        ctx,
        inspecting,
        (&sentence, Vec::new()),
        (vec![vec![edit.into()]], Vec::new()),
      )),
    };
  }
  // Signed out on one of the person's accounts: the fix is in Settings › Accounts, not a command here (`M42rM`).
  if let Some(account) = account.filter(|_| status == ConnectionStatus::SignInRequired) {
    let sentence = signed_out_sentence(ctx, account);
    let open = footer_action(inspecting, open_in_settings(ctx, account), Icon::ArrowUpRight);
    let buttons = vec![open.into(), edit.into()];
    return Parts {
      header,
      body,
      footer: Some(footer(
        ctx,
        inspecting,
        (&sentence, Vec::new()),
        (vec![buttons], Vec::new()),
      )),
    };
  }
  let command = (status == ConnectionStatus::SignInRequired)
    .then(|| sign_in_command(connection.provider()).map(str::to_owned))
    .flatten()
    .map(|command| {
      CodeBox::new(
        "connection-sign-in-command",
        ctx.t("connections.runInTerminal"),
        command,
      )
      .render(ctx)
    });
  let buttons = vec![
    footer_action(
      inspecting,
      check_button(inspecting, ("connection-test", ctx.t("connections.test")), actions),
      Icon::PlugZap,
    )
    .into(),
    edit.into(),
  ];
  let more = command.into_iter().collect();
  let reasons = Vec::new();
  let sentence = ctx.t(detail_key(status, connection.provider()));
  Parts {
    header,
    body,
    footer: Some(footer(ctx, inspecting, (&sentence, more), (vec![buttons], reasons))),
  }
}

pub(super) fn properties(
  ctx: &Ctx,
  connection: &AgentConnection,
  inspecting: &Inspecting<'_>,
) -> lurq::components::Column {
  let status = connection.status_with(&inspecting.listed.secrets);
  let mut rows = vec![property(
    "connection-provider",
    &ctx.t("connections.property.provider"),
    &ctx.t(provider_key(connection.provider())),
    PropertyValue::Plain,
  )];
  if let Some(program) = connection.executable() {
    let kind = if status == ConnectionStatus::ExecutableMissing {
      PropertyValue::MonoDanger
    } else {
      PropertyValue::Mono
    };
    rows.push(property(
      "connection-program",
      &ctx.t("connections.property.programFile"),
      &display::shorten_path(program, PATH_CHARS),
      kind,
    ));
  }
  rows.extend(launch(ctx, inspecting.listed.launch_of(connection.id())));
  let (version, kind) = match (checked_version(connection), connection.last_check()) {
    (Some(version), _) => (version.into(), PropertyValue::Plain),
    (None, Some(_)) => (ctx.t("connections.unknown"), PropertyValue::Muted),
    (None, None) => (ctx.t("connections.notCheckedYet"), PropertyValue::Muted),
  };
  rows.push(property(
    "connection-version",
    &ctx.t("connections.property.version"),
    &version,
    kind,
  ));
  rows.push(property(
    "connection-execution",
    &ctx.t("connections.property.execution"),
    &ctx.t("connections.thisComputer"),
    PropertyValue::Plain,
  ));
  let authentication = match connection.api_provider() {
    Some(_) => "connections.apiProvider.authentication",
    None => "connections.cliLogin",
  };
  rows.push(property(
    "connection-authentication",
    &ctx.t("connections.property.authentication"),
    &ctx.t(authentication),
    PropertyValue::Plain,
  ));
  if connection.api_provider().is_some() {
    rows.push(property(
      "connection-plan-limits",
      &ctx.t("connections.account.planLimits"),
      &ctx.t("connections.apiProvider.planLimits"),
      PropertyValue::Muted,
    ));
  }
  let (checked, kind) = last_checked(ctx, inspecting.now, connection.last_check().map(|check| check.at));
  rows.push(property(
    "connection-checked",
    &ctx.t("connections.property.lastChecked"),
    &checked,
    kind,
  ));
  lurq::components::Column::new()
    .id("connection-properties")
    .width(Dimension::Pct(100.0))
    .with_children(rows)
}

/// *Runs with* and *Script*, when what starts is not the program file itself: `cmd.exe` for a command
/// script, Node.js and its script for an npm launcher of one, the program an npm launcher starts in its
/// place.
fn launch(ctx: &Ctx, launch: Option<&Launch>) -> Vec<Row> {
  let Some(launch) = launch else {
    return Vec::new();
  };
  let runs = ctx.t("connections.property.runsWith");
  match launch.via() {
    Via::Direct => Vec::new(),
    Via::CommandScript => vec![property("connection-runs-with", &runs, CMD, PropertyValue::Plain)],
    Via::LauncherProgram => vec![property(
      "connection-runs-with",
      &runs,
      &display::shorten_path(launch.program(), PATH_CHARS),
      PropertyValue::Mono,
    )],
    Via::NodeScript { script } => vec![
      property("connection-runs-with", &runs, NODE, PropertyValue::Plain),
      property(
        "connection-script",
        &ctx.t("connections.property.script"),
        &display::shorten_path(script, PATH_CHARS),
        PropertyValue::Mono,
      ),
    ],
  }
}

/// That the CLI is too old for one of its models, as a run found when the provider refused it.
fn too_old(ctx: &Ctx, connection: &AgentConnection) -> Option<Element> {
  let (model, too_old) = connection
    .models()
    .iter()
    .find_map(|model| connection.cannot_run(model.id()).map(|too_old| (model.id(), too_old)))?;
  let title = ctx.t_args(
    "connections.cliTooOld.heading",
    [
      ("cli", &*ctx.t(provider_key(connection.provider()))),
      ("installed", too_old.installed.as_str()),
      ("model", model.as_str()),
      ("required", too_old.required.as_str()),
    ],
  );
  Some(
    Notice::new("connection-too-old", NoticeTone::Warning, title)
      .body(ctx.t("connections.cliTooOld.detail"))
      .into(),
  )
}

/// The models the last check listed, a model the CLI is too old for marked, where the list came from, and
/// Refresh models.
pub(super) fn models(
  ctx: &Ctx,
  connection: &AgentConnection,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Element {
  let chips = Row::new()
    .id("connection-models")
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Snug.role())
    .wrap()
    .with_children(connection.models().iter().enumerate().map(|(index, model)| -> Element {
      let id = format!("connection-model-{index}");
      match connection.cannot_run(model.id()) {
        Some(too_old) => {
          let word = ctx.t_args(
            "connections.modelNeeds",
            [("model", model.id().as_str()), ("required", too_old.required.as_str())],
          );
          badge(&id, BadgeTone::Warning, &word).into()
        }
        // An API provider's model ids are typed: shown as written, in Geist Mono (`y2qhJ`).
        None if connection.api_provider().is_some() => TagChip::new(id, model.id().as_str()).mono().into(),
        None => TagChip::new(id, model.id().as_str()).into(),
      }
    }));
  let note = match connection.catalog().map(|catalog| catalog.source()) {
    _ if connection.api_provider().is_some() => "connections.modelSource.apiProvider",
    _ if connection.models().is_empty() && connection.last_check().is_some_and(|check| check.result.is_verified()) => {
      "connections.noModelsListed"
    }
    Some(CatalogSource::Provider) => "connections.modelSource.provider",
    Some(CatalogSource::DocumentedAliases) => "connections.modelSource.aliases",
    Some(CatalogSource::ProviderAndCurrentIds) => "connections.modelSource.providerAndCurrentIds",
    None => "connections.modelsAfterCheck",
  };
  let refresh = check_button(
    inspecting,
    ("connection-refresh-models", ctx.t("connections.refreshModels")),
    actions,
  )
  .icon(Icon::RefreshCw);
  inspector::section("connection-models-section", SpacingSize::Sm)
    .child(inspector::section_title(
      "connection-models-title",
      &ctx.t("connections.availableModels"),
      None,
    ))
    .with_children((!connection.models().is_empty()).then_some(chips))
    .child(Text::new(&ctx.t(note)).variant(ExtraTypography::Note.role()))
    // An API provider's models are typed in Edit connection; a check of the CLI lists none of them.
    .with_children(connection.api_provider().is_none().then(|| Row::new().child(refresh)))
    .into()
}

fn native(
  ctx: &Ctx,
  connection: &AgentConnection,
  api: &ApiEndpoint,
  (header, used): (Element, Element),
  inspecting: &Inspecting<'_>,
) -> Parts {
  let provider = ctx.t(provider_key(connection.provider()));
  let unavailable = Notice::new(
    "connection-native-unavailable",
    NoticeTone::Info,
    ctx.t_args(
      "connections.nativeNotAvailable.heading",
      [("connection", connection.name().as_str()), ("provider", &*provider)],
    ),
  )
  .icon(Icon::Lock)
  .body(ctx.t("connections.nativeNotAvailable.detail"));
  let rows = native_properties(ctx, connection, api, inspecting);
  let models = native_models(ctx, api);
  let reason: Arc<str> = ctx.t("connections.native.testUnavailable");
  let buttons = vec![
    footer_action(
      inspecting,
      ActionButton::new("connection-test", ButtonKind::Secondary, ctx.t("connections.test")),
      Icon::PlugZap,
    )
    .unavailable(reason.clone())
    .into(),
    footer_action(
      inspecting,
      ActionButton::new("connection-edit", ButtonKind::Secondary, ctx.t("connections.edit")),
      Icon::Pencil,
    )
    .unavailable(reason.clone())
    .into(),
  ];
  Parts {
    header,
    body: vec![
      unavailable.into(),
      lurq::components::Column::new()
        .id("connection-properties")
        .width(Dimension::Pct(100.0))
        .with_children(rows)
        .into(),
      models.into(),
      used,
    ],
    footer: Some(footer(
      ctx,
      inspecting,
      (&reason, Vec::new()),
      (vec![buttons], Vec::new()),
    )),
  }
}

/// A native connection's provider, preset, endpoint, key, where it runs and when it was checked.
fn native_properties(
  ctx: &Ctx,
  connection: &AgentConnection,
  api: &ApiEndpoint,
  inspecting: &Inspecting<'_>,
) -> [Row; 6] {
  let provider = ctx.t(provider_key(connection.provider()));
  let key = inspecting
    .listed
    .secrets
    .iter()
    .find(|secret| secret.id() == api.key())
    .map_or_else(
      || ctx.t("connections.native.keyGone"),
      |secret| {
        ctx.t_args(
          "connections.native.key",
          [
            ("name", secret.name().as_str()),
            ("environment", secret.environment().as_str()),
          ],
        )
      },
    );
  let (checked, kind) = last_checked(ctx, inspecting.now, connection.last_check().map(|check| check.at));
  [
    property(
      "connection-provider",
      &ctx.t("connections.property.provider"),
      &provider,
      PropertyValue::Plain,
    ),
    property(
      "connection-preset",
      &ctx.t("connections.property.preset"),
      &ctx.t(preset_key(api.preset())),
      PropertyValue::Plain,
    ),
    property(
      "connection-endpoint",
      &ctx.t("connections.property.endpoint"),
      api.base_url().as_str(),
      PropertyValue::Mono,
    ),
    property(
      "connection-authentication",
      &ctx.t("connections.property.authentication"),
      &key,
      PropertyValue::Plain,
    ),
    property(
      "connection-execution",
      &ctx.t("connections.property.execution"),
      &ctx.t("connections.native.execution"),
      PropertyValue::Plain,
    ),
    property(
      "connection-checked",
      &ctx.t("connections.property.lastChecked"),
      &checked,
      kind,
    ),
  ]
}

/// A native connection's models, which this version neither edits nor fetches.
fn native_models(ctx: &Ctx, api: &ApiEndpoint) -> lurq::components::Column {
  inspector::section("connection-models-section", SpacingSize::Sm)
    .child(inspector::section_title(
      "connection-models-title",
      &ctx.t("connections.models"),
      None,
    ))
    .child(
      Row::new()
        .id("connection-models")
        .width(Dimension::Pct(100.0))
        .spacing(ExtraSpacing::Snug.role())
        .wrap()
        .with_children(
          api
            .models()
            .models()
            .iter()
            .enumerate()
            .map(|(index, model)| TagChip::new(format!("connection-model-{index}"), model.id().as_str()).mono()),
        ),
    )
    .child(reason_line(
      "connection-models-reason",
      &ctx.t("connections.native.modelsUnavailable"),
    ))
}

const fn preset_key(preset: EndpointPreset) -> &'static str {
  match preset {
    EndpointPreset::Official => "connections.native.preset.official",
    EndpointPreset::DeepSeek => "connections.native.preset.deepseek",
    EndpointPreset::Custom => "connections.native.preset.custom",
  }
}

/// What a status means and what to do about it. Signing in names the CLI's own command, which Orchester
/// never runs itself.
const fn detail_key(status: ConnectionStatus, provider: AgentProvider) -> &'static str {
  match status {
    ConnectionStatus::NotChecked => "connections.detail.notChecked",
    ConnectionStatus::Connected => "connections.detail.connected",
    ConnectionStatus::SignInRequired => match provider {
      AgentProvider::ClaudeCodeCli => "connections.detail.signInClaude",
      _ => "connections.detail.signInCodex",
    },
    ConnectionStatus::SignInUnknown => "connections.detail.signInUnknown",
    ConnectionStatus::ExecutableMissing => "connections.detail.executableMissing",
    ConnectionStatus::NotRecognized => "connections.detail.notRecognized",
    ConnectionStatus::CouldNotRun
    | ConnectionStatus::KeyMissing
    | ConnectionStatus::KeyRejected
    | ConnectionStatus::BillingProblem
    | ConnectionStatus::EndpointUnreachable => "connections.detail.couldNotRun",
  }
}

/// What a connection signed out on `account` means and where to fix it (`M42rM`).
fn signed_out_sentence(ctx: &Ctx, account: &orchester_core::account::AgentAccount) -> std::sync::Arc<str> {
  let name = crate::services::accounts::account_name(ctx, account);
  let expired = account.state(orchester_core::connection::invocation::Platform::current())
    == orchester_core::account::AccountState::Expired;
  match account.email().filter(|_| expired) {
    Some(email) => ctx.t_args(
      "connections.account.signedOutSentence",
      [("account", name.as_str()), ("email", email)],
    ),
    None => ctx.t_args("connections.account.notSignedInSentence", [("account", name.as_str())]),
  }
}

/// *Open … in Settings*: the account's page in Settings › Accounts, where it signs in.
fn open_in_settings(ctx: &mut Ctx, account: &orchester_core::account::AgentAccount) -> ActionButton {
  let navigation = crate::navigation::Navigation::of(ctx);
  let id = account.id();
  let name = crate::services::accounts::account_name(ctx, account);
  ActionButton::new(
    "connection-open-account",
    ButtonKind::Secondary,
    ctx.t_args("connections.account.openInSettings", [("account", name.as_str())]),
  )
  .bounded_label()
  .opens(
    navigation.to(crate::routes::Route::Accounts(crate::routes::AccountsView::of(
      id,
      crate::routes::AccountsPanel::Account,
    ))),
  )
}
