//! API providers in Settings › Accounts (`Agents on accounts` A1 `aOR4w`, B2 `OfUhN`;
//! `docs/design/agent-accounts.md`, "Agents run on accounts"): the person's API accounts, and *Add an API provider* as
//! a page of its own: what it is used with, a name, the base URL, the key and the models. An account for an agent CLI
//! is listed under the CLI's sign-ins; one for Orchester's own agent, in a group of its own, *Model APIs*. The key is
//! typed once, goes to the system keychain with the account and is never shown again; every project can then run a
//! profile on the account. The base URL, key and model fields are the ones a connection's form has
//! ([`ProviderForm`], [`NativeForm`]).
//!
//! A decision model's server is added here too, and stands under *Model APIs* ([`super::decision`]).
//!
//! A row's own actions, *Replace key…* and *Remove*, are [`super::api_rows`].
//!
//! Not here yet: renaming an account, another base URL or other models for it, and testing it.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountCli, ApiAccount, ApiSetup},
  connection::{AgentProvider, DecisionConnection},
  name::Name,
  resource::ModelId,
  secret::SecretValue,
};
use orchester_workspace::{background, connections::api_provider::base_url, messages::Message};

use super::{
  Work,
  api_rows::ApiRows,
  decision::{NewServer, Servers},
  words::{cli_key, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::{
    connections::{native_form::NativeForm, provider_form::ProviderForm},
    project_settings::kit::FormField,
    settings::frame,
  },
  services::Services,
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    message::message_text,
    pressable::Focus,
    setting_rows::{SettingGroup, SettingRow},
    tabs::{Segment, SegmentFace, segmented},
  },
};

/// The model API Orchester's own agent runs on in this version: Anthropic's. The OpenAI API's native agent is not
/// run yet (`AgentProvider::runs_agents`), so no account is offered for it.
const OWN: AgentProvider = AgentProvider::AnthropicApi;

/// What a new API provider is used with: an agent CLI, or Orchester's own agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UsedWith {
  Cli(AccountCli),
  Own,
  /// A decision model's server, which reviews what agents ask to do.
  Decision,
}

impl DevtoolsInspectable for UsedWith {}

impl UsedWith {
  const ALL: [Self; 4] = [
    Self::Cli(AccountCli::ClaudeCode),
    Self::Cli(AccountCli::Codex),
    Self::Own,
    Self::Decision,
  ];

  /// What its segment's id ends with.
  fn id(self) -> &'static str {
    match self {
      Self::Cli(cli) => cli.as_str(),
      Self::Own => "own",
      Self::Decision => "decision",
    }
  }

  fn key(self) -> &'static str {
    match self {
      Self::Cli(cli) => cli_key(cli),
      Self::Own => "accounts.api.own",
      Self::Decision => "accounts.api.decision.used",
    }
  }
}

/// What *Add provider* adds: an API account (its name, what reaches the model API, and the key), or a decision
/// model's server.
pub(super) enum NewApi {
  Account(Name, ApiSetup, SecretValue),
  Server(NewServer),
}
pub(super) type AddAction = FutureAction<NewApi, (), Message>;

/// What the person has typed so far for a new API provider. Cloning shares it.
#[derive(Clone)]
pub(super) struct Draft {
  pub used: Signal<UsedWith>,
  pub name: Signal<String>,
  /// The fields of an agent CLI's account.
  pub form: Arc<ProviderForm>,
  /// The fields of an account of Orchester's own agent.
  pub own: Arc<NativeForm>,
  /// The key of a decision model's server, typed as an account's is.
  pub decision_key: Arc<ProviderForm>,
  pub add: AddAction,
}

impl Draft {
  pub fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace, services.vault);
    let add = ctx.future_action(move |new: NewApi| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        let (adding, keys) = (workspace.clone(), vault.clone());
        let server = background::run(move || {
          let added = match new {
            NewApi::Account(name, setup, key) => adding.add_api_account(keys.get(), name, setup, &key).map(|_| None),
            NewApi::Server((name, url, key)) => adding
              .add_decision_server(keys.get(), name, url, key.as_ref())
              .map(|server| Some(server.id())),
          };
          added.map_err(|error| {
            tracing::warn!(%error, "an API provider was not added");
            error.message()
          })
        })
        .await??;
        // A decision model's server is asked for its models at once: its row then says how it stands. The server is
        // added whatever it answers.
        if let Some(server) = server
          && let Err(error) = workspace.check_decision_server(vault.get(), server).await
        {
          tracing::warn!(%error, "a decision model server just added was not checked");
        }
        Ok(())
      }
    });
    Self {
      used: ctx.signal(UsedWith::ALL[0]),
      name: ctx.signal(String::new()),
      form: Arc::new(ProviderForm::for_account(ctx)),
      own: Arc::new(NativeForm::for_account(ctx, OWN)),
      decision_key: Arc::new(ProviderForm::for_account(ctx)),
      add,
    }
  }

  /// Empties the page once what it held was added: the key first.
  pub fn clear(&self) {
    self.form.clear();
    self.own.clear(OWN);
    self.decision_key.clear();
    self.name.set(String::new());
    // The page starts over whole: what the last provider was used with is not the next one's.
    self.used.set(UsedWith::ALL[0]);
  }

  /// What reaches the model API, once the fields of an API account say it; `None` for a decision model's server.
  fn setup(&self) -> Option<ApiSetup> {
    match self.used.get() {
      UsedWith::Cli(cli) => {
        let url = base_url(&self.form.url().get()).ok()?;
        let models: Vec<ModelId> = self
          .form
          .model_ids()
          .iter()
          .filter_map(|model| ModelId::new(model).ok())
          .collect();
        ApiSetup::cli(cli, url, models).ok()
      }
      UsedWith::Own => self.own.account_setup(OWN),
      UsedWith::Decision => None,
    }
  }

  /// The first thing the page still needs before it can add the provider, as the key of the words that say so;
  /// nothing once [`Self::ready`] has what it adds. A decision model's server needs no key.
  fn needs(&self, servers: &Servers) -> Option<&'static str> {
    let named = Name::new(&self.name.get()).is_ok();
    let (url, key) = match self.used.get() {
      UsedWith::Cli(_) => (
        base_url(&self.form.url().get()).is_ok(),
        self.form.save_guard().typed_key().is_some(),
      ),
      UsedWith::Own => (base_url(&self.own.url().get()).is_ok(), self.own.typed_key().is_some()),
      UsedWith::Decision => (servers.typed_url().is_some(), true),
    };
    if !named {
      Some("accounts.api.needs.name")
    } else if !url {
      Some("accounts.api.needs.url")
    } else if !key {
      Some("accounts.api.needs.key")
    } else if self.ready(servers).is_none() {
      Some("accounts.api.needs.models")
    } else {
      None
    }
  }

  /// What the fields add, once each of them can be saved.
  fn ready(&self, servers: &Servers) -> Option<NewApi> {
    let name = Name::new(&self.name.get()).ok()?;
    let key = match self.used.get() {
      UsedWith::Cli(_) => self.form.save_guard().typed_key(),
      UsedWith::Own => self.own.typed_key(),
      UsedWith::Decision => {
        return Some(NewApi::Server((
          name,
          servers.typed_url()?,
          self.decision_key.save_guard().typed_key(),
        )));
      }
    };
    Some(NewApi::Account(name, self.setup()?, key?))
  }
}

pub(super) fn page(ctx: &mut Ctx, work: &Work) -> Column {
  let draft = &work.api;
  let used = draft.used.get();
  let navigation = Navigation::of(ctx);
  let used_with =
    SettingGroup::new("api-add-used-with", t(ctx, "accounts.api.usedWith")).row(used_row(ctx, draft, used));
  let provider = SettingGroup::new("api-add-provider", t(ctx, "accounts.api.provider")).row(name_row(ctx, draft, used));
  let (provider, warning) = match used {
    UsedWith::Cli(cli) => (
      provider
        .row(draft.form.url_row(ctx, cli.provider()))
        .row(draft.form.typed_key_row(ctx))
        .row(draft.form.models_row(ctx)),
      draft.form.warning(ctx),
    ),
    UsedWith::Own => (
      provider.rows(draft.own.account_rows(ctx, OWN)),
      Some(draft.own.warning(ctx)),
    ),
    UsedWith::Decision => (
      provider
        .row(work.servers.url_row(ctx))
        .row(draft.decision_key.typed_key_row(ctx)),
      None,
    ),
  };
  frame::pane("api-add-page")
    .child(frame::crumb(
      "api-add-crumb",
      &t(ctx, "settings.tabs.accounts"),
      navigation.to(Route::Accounts(AccountsView::default())),
    ))
    .child(frame::head(
      "api-add-head",
      None,
      (
        &t(ctx, "accounts.api.title"),
        Some(frame::lead("api-add-lead", &t(ctx, "accounts.api.sub")).into()),
      ),
      Vec::new(),
    ))
    .child(used_with)
    .child(provider)
    .with_children(warning)
    .child(footer(ctx, draft, &work.servers))
}

/// *Used with*: Claude Code, Codex, or Orchester's own agent.
fn used_row(ctx: &mut Ctx, draft: &Draft, chosen: UsedWith) -> Element {
  let segments = UsedWith::ALL.map(|used| {
    let signal = draft.used.clone();
    Segment::new(
      format!("api-add-cli-{}", used.id()),
      SegmentFace::Text(ctx.t(used.key())),
      chosen == used,
      move || signal.set(used),
    )
  });
  SettingRow::new("api-add-cli", t(ctx, "accounts.api.cli"))
    .hint(t(ctx, "accounts.api.cliHint"))
    .control(segmented("api-add-cli-choice", segments, Focus::Keyboard))
    .render(ctx)
}

/// *Name*: how a profile is offered it (*Claude Code · DeepSeek*; an account of Orchester's own agent by its name
/// alone). It is drawn as the base URL, the key and the models under it are, so the fields of the group are one
/// width.
fn name_row(ctx: &mut Ctx, draft: &Draft, used: UsedWith) -> Element {
  let typed = draft.name.get();
  let typed = typed.trim();
  let label = t(ctx, "accounts.add.name");
  let field = FieldInput::new("api-add-name", label.clone(), draft.name.clone()).render(ctx);
  let shown = match (used, typed.is_empty()) {
    (UsedWith::Cli(cli), true) => ta(ctx, "accounts.add.nameShownEmpty", [("cli", t(ctx, cli_key(cli)))]),
    (UsedWith::Cli(cli), false) => ta(
      ctx,
      "accounts.add.nameShown",
      [("cli", t(ctx, cli_key(cli))), ("name", typed.to_owned())],
    ),
    (UsedWith::Own | UsedWith::Decision, true) => t(ctx, "accounts.api.nameShownOwnEmpty"),
    (UsedWith::Own | UsedWith::Decision, false) => ta(ctx, "accounts.api.nameShownOwn", [("name", typed.to_owned())]),
  };
  FormField::new("api-add-name-row", label).hint(shown).render(ctx, field)
}

/// Whose change this is, *Cancel* and *Add provider*, with what the last try said over them.
fn footer(ctx: &mut Ctx, draft: &Draft, servers: &Servers) -> Element {
  let navigation = Navigation::of(ctx);
  let state = draft.add.state().get();
  let save = ActionButton::new("api-add-save", ButtonKind::Primary, t(ctx, "accounts.api.add")).icon(Icon::Check);
  let save = if state.is_pending() {
    save.loading()
  } else if draft.ready(servers).is_some() {
    let (draft, servers) = (draft.clone(), servers.clone());
    save.on_click(move || {
      // Read again at the press: the key is no ctx signal, and a field may have changed under it.
      if let Some(new) = draft.ready(&servers) {
        draft.add.run(new);
      }
    })
  } else {
    save.disabled()
  };
  let cancel = ActionButton::new("api-add-cancel", ButtonKind::Ghost, t(ctx, "accounts.add.cancel"))
    .icon(Icon::X)
    .on_click(move || navigation.close_panel(Route::Accounts(AccountsView::default())));
  let pending = state.is_pending();
  let error = state
    .error
    .filter(|_| !pending)
    .map(|error| field_error("api-add-error", &message_text(ctx, &error)));
  // Why *Add provider* cannot be pressed yet, beside it: the next thing to type.
  let needs = draft.needs(servers).filter(|_| !pending).map(|key| {
    Text::new(&t(ctx, key))
      .id("api-add-needs")
      .variant(TypographyStyle::Description)
      .color(PaletteColor::TextSecondary)
      .nowrap()
  });
  let reason = Row::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(icon(Icon::Info, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&t(ctx, "accounts.api.personNote"))
        .id("api-add-note")
        .variant(TypographyStyle::Description)
        .color(PaletteColor::TextMuted)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    );
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .with_children(error)
    .child(
      Row::new()
        .id("api-add-footer")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(reason)
        .with_children(needs)
        .child(cancel)
        .child(save),
    )
    .into()
}

/// How many API providers a CLI's group has, after its sign-ins; nothing for none.
pub(super) fn count(ctx: &Ctx, api: &[ApiAccount], cli: AccountCli) -> Option<String> {
  match of_cli(api, cli).count() {
    0 => None,
    1 => Some(t(ctx, "accounts.api.one")),
    many => Some(ta(ctx, "accounts.api.other", [("count", many.to_string())])),
  }
}

/// The rows of `cli`'s API providers, under its sign-ins: the name, *API provider*, its host and how many models it
/// serves, and that its key is kept.
pub(super) fn rows(ctx: &mut Ctx, api: &[ApiAccount], cli: AccountCli, actions: &ApiRows) -> Vec<Element> {
  of_cli(api, cli)
    .map(|account| {
      let models = match account.setup() {
        ApiSetup::Cli { models, .. } => models.len(),
        ApiSetup::Native { .. } => 0,
      };
      let line = ta(
        ctx,
        if models == 1 {
          "accounts.api.lineOne"
        } else {
          "accounts.api.line"
        },
        [
          ("host", account.setup().base_url().host().to_owned()),
          ("count", models.to_string()),
        ],
      );
      actions.row(ctx, account, line)
    })
    .collect()
}

/// *Model APIs*: the accounts of Orchester's own agent, each with the API it calls, its host and how many models are
/// listed for it, then the decision model servers; nothing while the person has neither.
pub(super) fn model_apis(
  ctx: &mut Ctx,
  api: &[ApiAccount],
  (servers, decision): (&Servers, &[DecisionConnection]),
  actions: &ApiRows,
) -> Option<Element> {
  let mut rows: Vec<Element> = api
    .iter()
    .filter_map(|account| {
      let ApiSetup::Native { provider, models, .. } = account.setup() else {
        return None;
      };
      let count = models.models().len();
      let key = match count {
        0 => "accounts.api.ownLineNone",
        1 => "accounts.api.ownLineOne",
        _ => "accounts.api.ownLine",
      };
      let api = t(
        ctx,
        if *provider == AgentProvider::OpenAiApi {
          "accounts.api.openai"
        } else {
          "accounts.api.anthropic"
        },
      );
      let line = ta(
        ctx,
        key,
        [
          ("api", api),
          ("host", account.setup().base_url().host().to_owned()),
          ("count", count.to_string()),
        ],
      );
      Some(actions.row(ctx, account, line))
    })
    .collect();
  rows.extend(servers.rows(ctx, decision));
  if rows.is_empty() {
    return None;
  }
  let count = match rows.len() {
    1 => t(ctx, "accounts.api.modelApisOne"),
    many => ta(ctx, "accounts.api.modelApisOther", [("count", many.to_string())]),
  };
  Some(
    SettingGroup::new("accounts-group-model-apis", t(ctx, "accounts.api.modelApis"))
      .meta(count)
      .rows(rows)
      .into(),
  )
}

/// The API accounts an agent CLI runs on, `cli`'s, in the order they were added.
fn of_cli(api: &[ApiAccount], cli: AccountCli) -> impl Iterator<Item = &ApiAccount> {
  api
    .iter()
    .filter(move |account| matches!(account.setup(), ApiSetup::Cli { cli: of, .. } if *of == cli))
}
