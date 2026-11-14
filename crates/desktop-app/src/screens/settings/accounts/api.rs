//! API providers in Settings › Accounts (`Agents on accounts` A1 `aOR4w`, B2 `OfUhN`;
//! `docs/design/agent-accounts.md`, "Agents run on accounts"): the person's API accounts, and *Add an API provider* as
//! a page of its own: what it is used with, a name, the base URL, the key and the models. An account for an agent CLI
//! is listed under the CLI's sign-ins; one for Orchester's own agent, in a group of its own, *Model APIs*. The key is
//! typed once, goes to the system keychain with the account and is never shown again; every project can then run a
//! profile on the account. The base URL, key and model fields are the ones a connection's form has
//! ([`ProviderForm`], [`NativeForm`]).
//!
//! Not here yet: an account's own page (rename, replace the key, test, remove), and the decision model's server.

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
  connection::AgentProvider,
  name::Name,
  resource::ModelId,
  secret::SecretValue,
};
use orchester_workspace::{background, connections::api_provider::base_url, messages::Message};

use super::{
  Work,
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
    setting_rows::{SettingGroup, SettingRow, row_tile, status},
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
}

impl DevtoolsInspectable for UsedWith {}

impl UsedWith {
  const ALL: [Self; 3] = [
    Self::Cli(AccountCli::ClaudeCode),
    Self::Cli(AccountCli::Codex),
    Self::Own,
  ];

  /// What its segment's id ends with.
  fn id(self) -> &'static str {
    match self {
      Self::Cli(cli) => cli.as_str(),
      Self::Own => "own",
    }
  }

  fn key(self) -> &'static str {
    match self {
      Self::Cli(cli) => cli_key(cli),
      Self::Own => "accounts.api.own",
    }
  }
}

/// What *Add provider* adds: the name, what reaches the model API, and the key.
type NewApi = (Name, ApiSetup, SecretValue);
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
  pub add: AddAction,
}

impl Draft {
  pub fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let (workspace, vault) = (services.workspace, services.vault);
    let add = ctx.future_action(move |(name, setup, key): NewApi| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        background::run(move || {
          workspace
            .add_api_account(vault.get(), name, setup, &key)
            .map(|_| ())
            .map_err(|error| {
              tracing::warn!(%error, "an API provider was not added");
              error.message()
            })
        })
        .await?
      }
    });
    Self {
      used: ctx.signal(UsedWith::ALL[0]),
      name: ctx.signal(String::new()),
      form: Arc::new(ProviderForm::for_account(ctx)),
      own: Arc::new(NativeForm::for_account(ctx, OWN)),
      add,
    }
  }

  /// Empties the page once what it held was added: the key first.
  pub fn clear(&self) {
    self.form.clear();
    self.own.clear(OWN);
    self.name.set(String::new());
  }

  /// The base URL as typed and the key typed, of the fields in use.
  fn typed(&self) -> (String, Option<SecretValue>) {
    match self.used.get() {
      UsedWith::Cli(_) => (self.form.url().get(), self.form.save_guard().typed_key()),
      UsedWith::Own => (self.own.url().get(), self.own.typed_key()),
    }
  }

  /// What reaches the model API, once the fields say it.
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
    }
  }

  /// The first thing the page still needs before it can add the provider, as the key of the words that say so;
  /// nothing once [`Self::ready`] has what it adds.
  fn needs(&self) -> Option<&'static str> {
    let (url, key) = self.typed();
    if Name::new(&self.name.get()).is_err() {
      Some("accounts.api.needs.name")
    } else if base_url(&url).is_err() {
      Some("accounts.api.needs.url")
    } else if key.is_none() {
      Some("accounts.api.needs.key")
    } else if self.setup().is_none() {
      Some("accounts.api.needs.models")
    } else {
      None
    }
  }

  /// What the fields add, once each of them can be saved.
  fn ready(&self) -> Option<NewApi> {
    let name = Name::new(&self.name.get()).ok()?;
    let setup = self.setup()?;
    let key = self.typed().1?;
    Some((name, setup, key))
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
    .child(footer(ctx, draft))
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
    (UsedWith::Own, true) => t(ctx, "accounts.api.nameShownOwnEmpty"),
    (UsedWith::Own, false) => ta(ctx, "accounts.api.nameShownOwn", [("name", typed.to_owned())]),
  };
  FormField::new("api-add-name-row", label).hint(shown).render(ctx, field)
}

/// Whose change this is, *Cancel* and *Add provider*, with what the last try said over them.
fn footer(ctx: &mut Ctx, draft: &Draft) -> Element {
  let navigation = Navigation::of(ctx);
  let state = draft.add.state().get();
  let save = ActionButton::new("api-add-save", ButtonKind::Primary, t(ctx, "accounts.api.add")).icon(Icon::Check);
  let save = if state.is_pending() {
    save.loading()
  } else if draft.ready().is_some() {
    let draft = draft.clone();
    save.on_click(move || {
      // Read again at the press: the key is no ctx signal, and a field may have changed under it.
      if let Some(new) = draft.ready() {
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
  let needs = draft.needs().filter(|_| !pending).map(|key| {
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
pub(super) fn rows(ctx: &mut Ctx, api: &[ApiAccount], cli: AccountCli) -> Vec<Element> {
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
      row(ctx, account, line)
    })
    .collect()
}

/// *Model APIs*: the accounts of Orchester's own agent, each with the API it calls, its host and how many models are
/// listed for it; nothing while the person has none.
pub(super) fn model_apis(ctx: &mut Ctx, api: &[ApiAccount]) -> Option<Element> {
  let rows: Vec<Element> = api
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
      Some(row(ctx, account, line))
    })
    .collect();
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

/// One API account's row: a cloud mark, its name over `line`, and that its key is kept.
fn row(ctx: &mut Ctx, account: &ApiAccount, line: String) -> Element {
  let id = format!("api-row-{}", account.id());
  let state = status(
    &format!("{id}-state"),
    (PaletteColor::Success, PaletteColor::TextSecondary),
    &t(ctx, "accounts.api.keySet"),
  );
  SettingRow::new(id, account.name().as_str().to_owned())
    .lead(row_tile(icon(Icon::Cloud, IconSize::Md, PaletteColor::TextSecondary)))
    .hint(line)
    .control(state)
    .render(ctx)
}

/// The API accounts an agent CLI runs on, `cli`'s, in the order they were added.
fn of_cli(api: &[ApiAccount], cli: AccountCli) -> impl Iterator<Item = &ApiAccount> {
  api
    .iter()
    .filter(move |account| matches!(account.setup(), ApiSetup::Cli { cli: of, .. } if *of == cli))
}
