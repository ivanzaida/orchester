//! An agent provider's page (`05b` `PGwka`): a CLI's Check again, its … menu (Rename, Edit connection), what its
//! status asks for (the sign-in command to run in a terminal, a model it is too old for, the account to sign in again
//! in Settings), *Runs on* (its program and version, that it runs on its own sign-in, its account), the models its
//! check listed with Refresh, and the profiles that use it. A native connection says native agents are not available
//! yet, its endpoint, key and models, its check and editing unavailable. A CLI on an API provider shows the provider
//! ([`super::provider`]) instead of an account.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::connection::{
  AgentConnection, AgentProvider, ApiEndpoint, CatalogSource, ConnectionAccount, ConnectionStatus, EndpointPreset,
};
use orchester_exec::launch::{Launch, Via};
use orchester_workspace::{connections::data::sign_in_command, display};

use super::{
  Actions, Inspecting, Parts, check_button, detail_row, last_checked, rename::title_with_rename, rename_item, result,
  sentence, sentence_tone, shown, used_by_row,
};
use crate::{
  screens::{
    connections::words::{agent_glyph, checked_version, connection_status, provider_key},
    project_settings::kit::{Group, SettingRow, Tone, command_line, value},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::MenuItem,
    notice::{Notice, NoticeTone},
  },
};

/// The longest path a row shows in full (`C:\…\npm\claude.cmd`).
const PATH_CHARS: usize = 40;
/// How many models a row names before it counts the rest.
const MODELS_SHOWN: usize = 3;
/// Product names of the programs that start a launcher script; identifiers, not translated.
const NODE: &str = "Node.js";
const CMD: &str = "cmd.exe";

pub(super) fn parts(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Parts {
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
  let used = used_by_row(ctx, &uses, actions, "connections.notUsed");
  match connection.api() {
    Some(api) => native(ctx, connection, api, used, (inspecting, actions)),
    None => match connection.api_provider() {
      Some(api) => super::provider::parts(ctx, (connection, api), used, inspecting, actions),
      None => cli(ctx, connection, used, (inspecting, actions)),
    },
  }
}

/// Rename and Edit connection, the … menu of an agent connection; Edit chooses a CLI connection's account.
pub(super) fn menu(
  ctx: &Ctx,
  connection: &AgentConnection,
  (inspecting, actions): (&Inspecting<'_>, &Actions),
) -> Vec<MenuItem> {
  let edit = MenuItem::opens(
    "connection-edit",
    ctx.t("connections.edit"),
    (actions.edit)(connection.id()),
  )
  .icon(Icon::Pencil);
  vec![rename_item(ctx, inspecting, actions), edit]
}

fn cli(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  used: Element,
  (inspecting, actions): (&Inspecting<'_>, &Actions),
) -> Parts {
  let status = connection.status_with(&inspecting.listed.secrets);
  let shown_status = shown(inspecting, connection_status(connection, status));
  let mut notices: Vec<Element> = too_old(ctx, connection).into_iter().collect();
  notices.extend(what_to_do(ctx, connection, status, shown_status, inspecting));
  notices.extend(result::agent(ctx, connection, inspecting));
  let runs_on = Group::new("connection-runs-on", ctx.t("connections.runsOn"))
    .rows(program_rows(ctx, connection, inspecting))
    .row(
      SettingRow::new("connection-authentication", ctx.t("connections.runsOn"))
        .hint(ctx.t("connections.runsOnHint"))
        .control(value(
          "connection-authentication-value",
          &ctx.t("connections.ownSignIn"),
        ))
        .render(ctx),
    )
    .row(super::super::account::account_row(ctx, inspecting.accounts, connection));
  Parts {
    glyph: agent_glyph(connection),
    title: title_with_rename(
      ctx,
      (inspecting, actions),
      (connection.id(), connection.name().as_str()),
    ),
    status: shown_status,
    status_detail: checked_detail(ctx, connection, inspecting),
    actions: vec![
      check_button(
        inspecting,
        ("connection-test", ctx.t("connections.checkAgain")),
        actions,
      )
      .into(),
    ],
    menu: menu(ctx, connection, (inspecting, actions)),
    notices,
    groups: vec![runs_on.into(), models(ctx, connection, inspecting, actions), used],
    question: None,
  }
}

/// *checked 2 min ago* after the status, once a check ran.
pub(super) fn checked_detail(ctx: &Ctx, connection: &AgentConnection, inspecting: &Inspecting<'_>) -> Option<Arc<str>> {
  let at = connection.last_check()?.at;
  Some(ctx.t_args(
    "connections.checkedAt",
    [("time", &*last_checked(ctx, inspecting.now, Some(at)))],
  ))
}

/// What the status asks for: a CLI on no account chooses one in Edit connection; one signed out on a person's account
/// signs in again in Settings › Accounts; one whose CLI is signed out runs its sign-in command in a terminal; any other
/// status that asks for something says what.
fn what_to_do(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  status: ConnectionStatus,
  shown: crate::screens::connections::words::Shown,
  inspecting: &Inspecting<'_>,
) -> Vec<Element> {
  if connection.account() == &ConnectionAccount::Unassigned {
    return vec![sentence(
      "connection-inspector-sentence",
      &ctx.t("connections.detail.noAccount"),
      sentence_tone(shown),
    )];
  }
  let account = inspecting
    .accounts
    .of_connection(connection.account(), connection.provider());
  if let Some(account) = account.filter(|_| status == ConnectionStatus::SignInRequired) {
    let words = signed_out_sentence(ctx, account);
    let open = open_in_settings(ctx, account);
    // The sentence wraps at the column's width, whatever the account's name; *Open … in Settings* under it.
    return vec![
      Column::new()
        .id("connection-inspector-sentence-row")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .child(sentence("connection-inspector-sentence", &words, sentence_tone(shown)))
        .child(Row::new().child(open))
        .into(),
    ];
  }
  if matches!(status, ConnectionStatus::Connected | ConnectionStatus::NotChecked) {
    return Vec::new();
  }
  let mut said = vec![sentence(
    "connection-inspector-sentence",
    &ctx.t(detail_key(status, connection.provider())),
    sentence_tone(shown),
  )];
  if let Some(command) = (status == ConnectionStatus::SignInRequired)
    .then(|| sign_in_command(connection.provider()))
    .flatten()
  {
    said.push(command_line(
      ctx,
      "connection-sign-in-command",
      ctx.t("connections.runInTerminal"),
      command.to_owned(),
    ));
  }
  said
}

/// *Program*: the program file and what starts it in its hint, its version as the value; *Script* when a launcher
/// runs one.
pub(super) fn program_rows(ctx: &mut Ctx, connection: &AgentConnection, inspecting: &Inspecting<'_>) -> Vec<Element> {
  let status = connection.status_with(&inspecting.listed.secrets);
  let launch = inspecting.listed.launch_of(connection.id());
  let path = connection
    .executable()
    .map(|program| display::shorten_path(program, PATH_CHARS))
    .unwrap_or_default();
  let hint = match launch.map(Launch::via) {
    Some(Via::CommandScript) => ctx.t_args(
      "connections.programRunsWith",
      [("path", path.as_str()), ("runner", CMD)],
    ),
    Some(Via::NodeScript { .. }) => ctx.t_args(
      "connections.programRunsWith",
      [("path", path.as_str()), ("runner", NODE)],
    ),
    Some(Via::LauncherProgram) => ctx.t_args(
      "connections.programRunsWith",
      [
        ("path", path.as_str()),
        (
          "runner",
          &display::shorten_path(launch.map_or(std::path::Path::new(""), Launch::program), PATH_CHARS),
        ),
      ],
    ),
    _ => path.as_str().into(),
  };
  let (version, tone) = match (checked_version(connection), connection.last_check()) {
    (Some(version), _) => (version.into(), Tone::Muted),
    (None, Some(_)) => (ctx.t("connections.unknown"), Tone::Muted),
    (None, None) => (ctx.t("connections.notCheckedYet"), Tone::Muted),
  };
  let hint_tone = if status == ConnectionStatus::ExecutableMissing {
    Tone::Danger
  } else {
    tone
  };
  let mut rows = vec![
    SettingRow::new("connection-program", ctx.t("connections.property.program"))
      .toned(hint, hint_tone)
      .info(ctx.t(provider_key(connection.provider())))
      .control(value("connection-version", &version))
      .render(ctx),
  ];
  if let Some(Via::NodeScript { script }) = launch.map(Launch::via) {
    rows.push(detail_row(
      ctx,
      "connection-script",
      (
        &ctx.t("connections.property.script"),
        &display::shorten_path(script, PATH_CHARS),
      ),
      true,
    ));
  }
  rows
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

/// *Models* (`OWJtS`): the models the last check listed (a model the CLI is too old for marked) in the row's hint,
/// where the list came from in its (i), and Refresh; a CLI on an API provider types its models in Edit connection.
pub(super) fn models(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  inspecting: &Inspecting<'_>,
  actions: &Actions,
) -> Element {
  let names: Vec<String> = connection
    .models()
    .iter()
    .map(|model| match connection.cannot_run(model.id()) {
      Some(too_old) => ctx
        .t_args(
          "connections.modelNeeds",
          [("model", model.id().as_str()), ("required", too_old.required.as_str())],
        )
        .to_string(),
      None => model.id().as_str().to_owned(),
    })
    .collect();
  let source = match connection.catalog().map(|catalog| catalog.source()) {
    _ if connection.api_provider().is_some() => "connections.modelSource.apiProvider",
    _ if connection.models().is_empty() && connection.last_check().is_some_and(|check| check.result.is_verified()) => {
      "connections.noModelsListed"
    }
    Some(CatalogSource::Provider) => "connections.modelSource.provider",
    Some(CatalogSource::DocumentedAliases) => "connections.modelSource.aliases",
    Some(CatalogSource::ProviderAndCurrentIds) => "connections.modelSource.providerAndCurrentIds",
    None => "connections.modelsAfterCheck",
  };
  let too_old = connection
    .models()
    .iter()
    .any(|model| connection.cannot_run(model.id()).is_some());
  let hint = if names.is_empty() {
    ctx.t(source)
  } else {
    listed(ctx, &names)
  };
  let row = SettingRow::new("connection-models", ctx.t("connections.modelsItLists"))
    .toned(hint, if too_old { Tone::Warning } else { Tone::Muted })
    .info(ctx.t(source));
  // An API provider's models are typed in Edit connection; a check of the CLI lists none of them.
  let row = if connection.api_provider().is_none() {
    row.control(
      check_button(
        inspecting,
        ("connection-refresh-models", ctx.t("connections.refreshModels")),
        actions,
      )
      .compact(),
    )
  } else {
    row
  };
  Group::new("connection-models-section", ctx.t("connections.models"))
    .row(row.render(ctx))
    .into()
}

/// `names`, the first few and how many more.
fn listed(ctx: &Ctx, names: &[String]) -> Arc<str> {
  if names.len() <= MODELS_SHOWN {
    return names.join(", ").into();
  }
  ctx.t_args(
    "connections.listedMore",
    [
      ("names", names[..MODELS_SHOWN].join(", ")),
      ("more", (names.len() - MODELS_SHOWN).to_string()),
    ],
  )
}

fn native(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  api: &ApiEndpoint,
  used: Element,
  (inspecting, actions): (&Inspecting<'_>, &Actions),
) -> Parts {
  let provider = ctx.t(provider_key(connection.provider()));
  let reason: Arc<str> = ctx.t("connections.native.testUnavailable");
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
  let rows = native_rows(ctx, connection, api, inspecting);
  let models: Vec<String> = api
    .models()
    .models()
    .iter()
    .map(|model| model.id().as_str().to_owned())
    .collect();
  let models = Group::new("connection-models-section", ctx.t("connections.models")).row(
    SettingRow::new("connection-models", ctx.t("connections.models"))
      .hint(listed(ctx, &models))
      .info(ctx.t("connections.native.modelsUnavailable"))
      .render(ctx),
  );
  let test = ActionButton::new("connection-test", ButtonKind::Secondary, ctx.t("connections.test"))
    .icon(Icon::PlugZap)
    .unavailable(reason.clone());
  let edit = MenuItem::unavailable("connection-edit", ctx.t("connections.edit"), reason).icon(Icon::Pencil);
  Parts {
    glyph: agent_glyph(connection),
    title: title_with_rename(
      ctx,
      (inspecting, actions),
      (connection.id(), connection.name().as_str()),
    ),
    status: shown(
      inspecting,
      connection_status(connection, connection.status_with(&inspecting.listed.secrets)),
    ),
    status_detail: checked_detail(ctx, connection, inspecting),
    actions: vec![test.into()],
    menu: vec![rename_item(ctx, inspecting, actions), edit],
    notices: vec![unavailable.into()],
    groups: vec![
      Group::new("connection-properties", ctx.t("connections.connectionGroup"))
        .rows(rows)
        .into(),
      models.into(),
      used,
    ],
    question: None,
  }
}

/// A native connection's provider, preset, endpoint, key and where it runs.
fn native_rows(
  ctx: &mut Ctx,
  connection: &AgentConnection,
  api: &ApiEndpoint,
  inspecting: &Inspecting<'_>,
) -> Vec<Element> {
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
  let rows = [
    ("connection-provider", "connections.property.provider", provider, false),
    (
      "connection-preset",
      "connections.property.preset",
      ctx.t(preset_key(api.preset())),
      false,
    ),
    (
      "connection-endpoint",
      "connections.property.endpoint",
      api.base_url().as_str().into(),
      true,
    ),
    (
      "connection-authentication",
      "connections.property.authentication",
      key,
      false,
    ),
    (
      "connection-execution",
      "connections.property.execution",
      ctx.t("connections.native.execution"),
      false,
    ),
  ];
  rows
    .into_iter()
    .map(|(id, label, shown, mono)| detail_row(ctx, id, (&ctx.t(label), &shown), mono))
    .collect()
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
fn signed_out_sentence(ctx: &Ctx, account: &orchester_core::account::AgentAccount) -> Arc<str> {
  let name = crate::services::accounts::account_name(ctx, account);
  let expired = account.state() == orchester_core::account::AccountState::Expired;
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
  .icon(Icon::ArrowUpRight)
  .bounded_label()
  .opens(
    navigation.to(crate::routes::Route::Accounts(crate::routes::AccountsView::of(
      id,
      crate::routes::AccountsPanel::Account,
    ))),
  )
}
