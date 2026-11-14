//! API providers in Settings › Accounts (`Agents on accounts` A1 `aOR4w`, B2 `OfUhN`;
//! `docs/design/agent-accounts.md`, "Agents run on accounts"): the person's API accounts for an agent CLI, listed
//! under the CLI's sign-ins, and *Add an API provider* as a page of its own: the CLI, a name, the base URL, the key
//! and the models it serves. The key is typed once, goes to the system keychain with the account and is never shown
//! again; every project can then run a profile on the account. The base URL, key and model fields are the ones a
//! connection's form has ([`ProviderForm`]).
//!
//! Not here yet: an account's own page (rename, replace the key, remove) and the model APIs of Orchester's own agent.

use std::sync::Arc;

use lurq::{
  app::{
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
  name::Name,
  resource::ModelId,
  secret::SecretValue,
};
use orchester_workspace::{background, connections::api_provider::base_url, messages::Message};

use super::{
  Work,
  add::CliChoice,
  words::{cli_key, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsView, Route},
  screens::{connections::provider_form::ProviderForm, project_settings::kit::FormField, settings::frame},
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

/// What *Add provider* adds: the name, what reaches the model API, and the key.
type NewApi = (Name, ApiSetup, SecretValue);
pub(super) type AddAction = FutureAction<NewApi, (), Message>;

/// What the person has typed so far for a new API provider. Cloning shares it.
#[derive(Clone)]
pub(super) struct Draft {
  pub cli: Signal<CliChoice>,
  pub name: Signal<String>,
  pub form: Arc<ProviderForm>,
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
      cli: ctx.signal(CliChoice(AccountCli::ALL[0])),
      name: ctx.signal(String::new()),
      form: Arc::new(ProviderForm::for_account(ctx)),
      add,
    }
  }

  /// Empties the page once what it held was added: the key first.
  pub fn clear(&self) {
    self.form.clear();
    self.name.set(String::new());
  }

  /// What the fields add, once each of them can be saved.
  fn ready(&self) -> Option<NewApi> {
    let name = Name::new(&self.name.get()).ok()?;
    let url = base_url(&self.form.url().get()).ok()?;
    let models: Vec<ModelId> = self
      .form
      .model_ids()
      .iter()
      .filter_map(|model| ModelId::new(model).ok())
      .collect();
    let setup = ApiSetup::cli(self.cli.get().0, url, models).ok()?;
    let key = self.form.save_guard().typed_key()?;
    Some((name, setup, key))
  }
}

pub(super) fn page(ctx: &mut Ctx, work: &Work) -> Column {
  let draft = &work.api;
  let cli = draft.cli.get().0;
  let navigation = Navigation::of(ctx);
  let used_with = SettingGroup::new("api-add-used-with", t(ctx, "accounts.api.usedWith")).row(cli_row(ctx, draft, cli));
  let provider = SettingGroup::new("api-add-provider", t(ctx, "accounts.api.provider"))
    .row(name_row(ctx, draft, cli))
    .row(draft.form.url_row(ctx, cli.provider()))
    .row(draft.form.typed_key_row(ctx))
    .row(draft.form.models_row(ctx));
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
    .with_children(draft.form.warning(ctx))
    .child(footer(ctx, draft))
}

/// *Used with*: Claude Code or Codex. Orchester's own agent on a model API is not offered here yet.
fn cli_row(ctx: &mut Ctx, draft: &Draft, chosen: AccountCli) -> Element {
  let segments = AccountCli::ALL.map(|cli| {
    let signal = draft.cli.clone();
    Segment::new(
      format!("api-add-cli-{}", cli.as_str()),
      SegmentFace::Text(ctx.t(cli_key(cli))),
      chosen == cli,
      move || signal.set(CliChoice(cli)),
    )
  });
  SettingRow::new("api-add-cli", t(ctx, "accounts.api.cli"))
    .hint(t(ctx, "accounts.api.cliHint"))
    .control(segmented("api-add-cli-choice", segments, Focus::Keyboard))
    .render(ctx)
}

/// *Name*: how a profile is offered it (*Claude Code · DeepSeek*). It is drawn as the base URL, the key and the
/// models under it are, so the four fields of the group are one width.
fn name_row(ctx: &mut Ctx, draft: &Draft, cli: AccountCli) -> Element {
  let typed = draft.name.get();
  let typed = typed.trim();
  let label = t(ctx, "accounts.add.name");
  let field = FieldInput::new("api-add-name", label.clone(), draft.name.clone()).render(ctx);
  let cli = t(ctx, cli_key(cli));
  let shown = if typed.is_empty() {
    ta(ctx, "accounts.add.nameShownEmpty", [("cli", cli)])
  } else {
    ta(
      ctx,
      "accounts.add.nameShown",
      [("cli", cli), ("name", typed.to_owned())],
    )
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
      let id = format!("api-row-{}", account.id());
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
    })
    .collect()
}

/// The API accounts an agent CLI runs on, `cli`'s, in the order they were added.
fn of_cli(api: &[ApiAccount], cli: AccountCli) -> impl Iterator<Item = &ApiAccount> {
  api
    .iter()
    .filter(move |account| matches!(account.setup(), ApiSetup::Cli { cli: of, .. } if *of == cli))
}
