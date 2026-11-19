//! An API provider's page (`Agents on accounts` B1 `gZTuX`), which its row in the list opens
//! ([`super::provider_rows`]): the crumb back to Accounts; the head with a cloud mark, the provider's name, whether its
//! key is on this computer and what it is used with, *Replace key…* and *…* (*Rename*, *Edit*, *Remove*); what reaches
//! the model API (`Provider`); and the agent profiles that run on it (`Used by`). *Rename* turns the name into a field
//! in the head; *Edit* turns the base URL and the models of an agent CLI's provider into fields. *Remove* is refused
//! while a profile runs on the provider, and says so. The key is never shown: *Replace key…* takes a new one
//! ([`super::api_rows`]). A model API Orchester's own agent calls has *Test*, which lists its models with the key.
//!
//! Not here: another base URL or other models for Orchester's own agent, whose *Edit* says so.

use std::sync::Arc;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{ApiAccount, ApiSetup},
  connection::{AgentProvider, ApiProvider},
  id::{ApiAccountId, SecretId},
  name::Name,
  resource::ModelId,
};
use orchester_workspace::{accounts::AccountUses, background, connections::api_provider::base_url, messages::Message};

use super::{
  Work,
  provider_rows::Rows,
  used_by,
  words::{cli_key, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  screens::{connections::provider_form::ProviderForm, settings::frame},
  services::Services,
  theme::{ExtraColor, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    message::message_text,
    setting_rows::{SettingGroup, SettingRow, group_note, head_tile, mono_value, value},
  },
};

/// The name's field while renaming, as an account's (`Field` `j7t2Ea` in `poD7g`).
const NAME_FIELD_WIDTH: f32 = 240.0;
/// The … menu's width (the `Menu` component `Qa7Oo`).
const MENU_WIDTH: f32 = 220.0;

/// A change to a provider that its page makes.
pub(super) enum Change {
  Rename(ApiAccountId, Name),
  /// What reaches the model API: another base URL or other models.
  Edit(ApiAccountId, ApiSetup),
}

/// The provider a change was made to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Changed(pub ApiAccountId);

impl DevtoolsInspectable for Changed {}

/// What the page holds besides the provider: the name being typed, its menu, the fields of *Edit* and its actions.
#[derive(Clone)]
pub(super) struct Page {
  pub rename: Signal<String>,
  pub more: (Signal<bool>, ElementRef),
  /// The base URL and the models of an agent CLI's provider as *Edit* shows them, and the provider they were
  /// filled from (its id as text; empty while none).
  pub form: Arc<ProviderForm>,
  seeded: Signal<String>,
  /// Renames or edits a provider, and says which one it was.
  pub change: FutureAction<Change, Changed, Message>,
  /// Lists the models of a model API Orchester's own agent calls, with its key.
  pub test: FutureAction<ApiAccountId, (), Message>,
}

impl Page {
  pub fn new(ctx: &mut Ctx) -> Self {
    let services = Services::of(ctx);
    let workspace = services.workspace.clone();
    let change = ctx.future_action(move |change: Change| {
      let workspace = workspace.clone();
      async move {
        background::run(move || {
          let changed = match change {
            Change::Rename(id, name) => workspace.rename_api_account(id, name),
            Change::Edit(id, setup) => workspace.change_api_account(id, setup),
          };
          changed.map(|account| Changed(account.id())).map_err(|error| {
            tracing::warn!(%error, "an API provider was not changed");
            error.message()
          })
        })
        .await?
      }
    });
    let (workspace, vault) = (services.workspace, services.vault);
    let test = ctx.future_action(move |id: ApiAccountId| {
      let (workspace, vault) = (workspace.clone(), vault.clone());
      async move {
        workspace
          .check_api_account(vault.get(), id)
          .await
          .map(drop)
          .map_err(|error| error.message())
      }
    });
    Self {
      rename: ctx.signal(String::new()),
      more: (ctx.signal(false), ElementRef::default()),
      form: Arc::new(ProviderForm::for_account(ctx)),
      seeded: ctx.signal(String::new()),
      change,
      test,
    }
  }

  /// Fills *Edit*'s fields from `account` once for each time it opens.
  fn seed(&self, account: &ApiAccount) {
    let id = account.id().to_string();
    if self.seeded.get() == id {
      return;
    }
    if let ApiSetup::Cli { base_url, models, .. } = account.setup() {
      // The key's id only stands in the provider the fields are filled from; *Edit* shows no key.
      let filled = ApiProvider::new(
        base_url.clone(),
        SecretId::from_uuid(account.id().as_uuid()),
        models.clone(),
      );
      self.form.seed(filled.ok().as_ref());
    }
    self.seeded.set(id);
  }

  /// Leaves *Edit* or *Rename*: the fields are filled again the next time.
  pub fn close(&self) {
    self.seeded.set(String::new());
    self.rename.set(String::new());
  }
}

/// What the provider is used with, as its page says it: the agent and the API it speaks.
fn used_with(ctx: &Ctx, account: &ApiAccount) -> String {
  match account.setup() {
    ApiSetup::Cli { cli, .. } => ta(
      ctx,
      "accounts.api.wire.cli",
      [
        ("cli", t(ctx, cli_key(*cli))),
        (
          "api",
          t(
            ctx,
            if cli.provider() == AgentProvider::CodexCli {
              "accounts.api.wire.responses"
            } else {
              "accounts.api.wire.messages"
            },
          ),
        ),
      ],
    ),
    ApiSetup::Native { provider, .. } => ta(
      ctx,
      "accounts.api.wire.own",
      [(
        "api",
        t(
          ctx,
          if *provider == AgentProvider::OpenAiApi {
            "accounts.api.openai"
          } else {
            "accounts.api.anthropic"
          },
        ),
      )],
    ),
  }
}

/// The agent the provider is used with, by its name alone.
fn agent(ctx: &Ctx, account: &ApiAccount) -> String {
  match account.setup() {
    ApiSetup::Cli { cli, .. } => t(ctx, cli_key(*cli)),
    ApiSetup::Native { .. } => t(ctx, "accounts.api.own"),
  }
}

/// The page of the provider the route names; `uses` is what runs on it and whether that is still being read.
pub(super) fn page(
  ctx: &mut Ctx,
  work: &Work,
  (account, rows, panel): (Option<&ApiAccount>, &Rows, AccountsPanel),
  (uses, reading): (Option<&AccountUses>, bool),
) -> Column {
  let navigation = Navigation::of(ctx);
  let pane = frame::pane("provider-page").child(frame::crumb(
    "provider-crumb",
    &t(ctx, "settings.tabs.accounts"),
    navigation.to(Route::Accounts(AccountsView::default())),
  ));
  let Some(account) = account else {
    return pane;
  };
  let editing = panel == AccountsPanel::Edit && matches!(account.setup(), ApiSetup::Cli { .. });
  let head = if panel == AccountsPanel::Rename {
    rename_head(ctx, work, account)
  } else {
    head(ctx, work, (account, rows), (uses, reading))
  };
  let provider = if editing {
    work.provider.seed(account);
    edit_group(ctx, work, account)
  } else {
    facts(ctx, account, rows)
  };
  let failed = work
    .provider
    .test
    .state()
    .get()
    .error
    .map(|error| field_error("provider-test-error", &message_text(ctx, &error)));
  let accounts = [account.clone()];
  pane
    .child(head)
    .with_children(failed)
    .child(provider)
    .child(used_by::group(ctx, uses, "provider-used-by"))
    .with_children(work.api_rows.questions(ctx, &accounts))
    .child(group_note("provider-note", &t(ctx, "accounts.api.pageNote")))
}

/// The head: the name, how the key stands and what it is used with, *Test* for a model API Orchester calls,
/// *Replace key…* and *…*.
fn head(
  ctx: &mut Ctx,
  work: &Work,
  (account, rows): (&ApiAccount, &Rows),
  uses: (Option<&AccountUses>, bool),
) -> Element {
  let line = rows.state_line(ctx, "provider-state", account, agent(ctx, account));
  let id = account.id();
  let replace = {
    let rows = work.api_rows.clone();
    ActionButton::new(
      "provider-replace-key",
      ButtonKind::Secondary,
      t(ctx, "accounts.api.replaceKey"),
    )
    .icon(Icon::KeyRound)
    .on_click(move || rows.ask_replace(id))
  };
  let test = matches!(account.setup(), ApiSetup::Native { .. }).then(|| {
    let button =
      ActionButton::new("provider-test", ButtonKind::Ghost, t(ctx, "accounts.api.test")).icon(Icon::RefreshCw);
    if work.provider.test.state().get().is_pending() {
      button.loading().into()
    } else {
      let test = work.provider.test.clone();
      button.on_click(move || test.run(id)).into()
    }
  });
  let more = more(ctx, work, account, uses);
  let tile = head_tile(icon(Icon::Cloud, IconSize::Lg, ExtraColor::TextSecondary.role())).into();
  let actions: Vec<Element> = test.into_iter().chain([replace.into(), more]).collect();
  frame::head(
    "provider-head",
    Some(tile),
    (account.name().as_str(), Some(line)),
    actions,
  )
  .into()
}

/// *…* and its menu: *Rename*, *Edit* and *Remove*, which is refused while a profile runs on the provider.
fn more(ctx: &mut Ctx, work: &Work, account: &ApiAccount, (uses, reading): (Option<&AccountUses>, bool)) -> Element {
  let (open, anchor) = work.provider.more.clone();
  let toggle = open.clone();
  let button = IconButton::new("provider-more", Icon::Ellipsis, t(ctx, "accounts.actions.more"))
    .element(anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open));
  let shown = open.get();
  let button = if shown { button.active() } else { button }.render(ctx);
  let menu = shown.then(|| {
    let navigation = Navigation::of(ctx);
    let id = account.id();
    let to = |panel: AccountsPanel| {
      let navigation = navigation.clone();
      move || navigation.go(Route::Accounts(AccountsView::of_provider(id, panel)))
    };
    let rename = MenuItem::new(
      "provider-rename",
      t(ctx, "accounts.actions.rename"),
      to(AccountsPanel::Rename),
    )
    .icon(Icon::Pencil);
    let edit = if matches!(account.setup(), ApiSetup::Cli { .. }) {
      MenuItem::new("provider-edit", t(ctx, "accounts.api.edit"), to(AccountsPanel::Edit))
    } else {
      MenuItem::unavailable(
        "provider-edit",
        t(ctx, "accounts.api.edit"),
        t(ctx, "accounts.api.editOwn"),
      )
    }
    .icon(Icon::SlidersHorizontal);
    let in_use = uses.is_some_and(|uses| !uses.profiles.is_empty() || !uses.running.is_empty());
    let remove = if in_use {
      MenuItem::unavailable(
        "provider-remove",
        t(ctx, "accounts.actions.remove"),
        t(ctx, "accounts.api.removeInUse"),
      )
    } else if reading || uses.is_none() {
      MenuItem::unavailable(
        "provider-remove",
        t(ctx, "accounts.actions.remove"),
        t(ctx, "accounts.api.removeReading"),
      )
    } else {
      let rows = work.api_rows.clone();
      MenuItem::new("provider-remove", t(ctx, "accounts.actions.remove"), move || {
        rows.ask_remove(id);
      })
    }
    .icon(Icon::Trash2);
    let panel = [rename, edit, remove].into_iter().fold(
      menu_panel("provider-more-menu").width(Dimension::Px(MENU_WIDTH)),
      |panel, item| panel.child(item.render(ctx, &open)),
    );
    menu_at_end(anchor, open.clone(), panel)
  });
  Row::new().child(button).with_children(menu).into()
}

/// The head while renaming: the name as a field with *Save* and *Cancel*.
fn rename_head(ctx: &mut Ctx, work: &Work, account: &ApiAccount) -> Element {
  let page = &work.provider;
  if page.rename.get().is_empty() {
    page.rename.set(account.name().as_str().to_owned());
  }
  let id = account.id();
  let state = page.change.state().get();
  let save = ActionButton::new(
    "provider-rename-save",
    ButtonKind::Primary,
    t(ctx, "accounts.rename.save"),
  );
  let save = if state.is_pending() {
    save.loading()
  } else if let Ok(name) = Name::new(&page.rename.get()) {
    let change = page.change.clone();
    save.on_click(move || change.run(Change::Rename(id, name.clone())))
  } else {
    save.disabled()
  };
  let cancel = cancel(ctx, work, "provider-rename-cancel", id);
  let pending = state.is_pending();
  let error = state
    .error
    .filter(|_| !pending)
    .map(|error| field_error("provider-rename-error", &message_text(ctx, &error)));
  let field = FieldInput::new(
    "provider-rename-field",
    t(ctx, "accounts.actions.rename"),
    page.rename.clone(),
  )
  .width(Dimension::Px(NAME_FIELD_WIDTH))
  .render(ctx);
  let tile: Element = head_tile(icon(Icon::Cloud, IconSize::Lg, ExtraColor::TextSecondary.role())).into();
  Column::new()
    .id("provider-head")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(tile)
        .child(field)
        .child(save)
        .child(cancel),
    )
    .with_children(error)
    .into()
}

/// *Cancel* of *Rename* and *Edit*: back to the page as it was.
fn cancel(ctx: &mut Ctx, work: &Work, id: &str, account: ApiAccountId) -> ActionButton {
  let (navigation, page) = (Navigation::of(ctx), work.provider.clone());
  ActionButton::new(id.to_owned(), ButtonKind::Ghost, t(ctx, "accounts.rename.cancel")).on_click(move || {
    page.close();
    navigation.close_panel(Route::Accounts(AccountsView::of_provider(
      account,
      AccountsPanel::Account,
    )));
  })
}

/// *Provider*: what the provider is used with, where its requests go, how its key stands, its models and where the
/// project's data goes.
fn facts(ctx: &mut Ctx, account: &ApiAccount, rows: &Rows) -> Element {
  let setup = account.setup();
  let host = setup.base_url().host().to_owned();
  let models: Vec<&str> = match setup {
    ApiSetup::Cli { models, .. } => models.iter().map(ModelId::as_str).collect(),
    ApiSetup::Native { models, .. } => models.models().iter().map(|model| model.id().as_str()).collect(),
  };
  let models = if models.is_empty() {
    t(ctx, "accounts.api.noModels")
  } else {
    models.join(" · ")
  };
  let missing = rows
    .missing
    .as_ref()
    .is_some_and(|missing| missing.contains(&account.id()));
  let key = t(
    ctx,
    if missing {
      "accounts.api.keyMissingHere"
    } else {
      "accounts.api.keyKept"
    },
  );
  let key_row = SettingRow::new("provider-key", t(ctx, "accounts.api.apiKey"));
  let key_row = if missing {
    key_row.control(value("provider-key-value", &key).color(PaletteColor::Destructive))
  } else {
    key_row.control(value("provider-key-value", &key))
  };
  let used = used_with(ctx, account);
  let url_hint = ta(ctx, "accounts.api.baseUrlHint", [("agent", agent(ctx, account))]);
  let goes_to = ta(ctx, "accounts.api.dataGoesToValue", [("host", host)]);
  SettingGroup::new("provider-facts", t(ctx, "accounts.api.provider"))
    .row(
      SettingRow::new("provider-used-with", t(ctx, "accounts.api.usedWith"))
        .control(value("provider-used-with-value", &used))
        .render(ctx),
    )
    .row(
      SettingRow::new("provider-url", t(ctx, "accounts.api.baseUrl"))
        .hint(url_hint)
        .control(mono_value("provider-url-value", setup.base_url().as_str()))
        .render(ctx),
    )
    .row(key_row.render(ctx))
    .row(
      SettingRow::new("provider-models", t(ctx, "accounts.api.models"))
        .control(value("provider-models-value", &models))
        .render(ctx),
    )
    .row(
      SettingRow::new("provider-data", t(ctx, "accounts.api.dataGoesTo"))
        .control(value("provider-data-value", &goes_to))
        .render(ctx),
    )
    .into()
}

/// *Provider* while editing: the base URL and the models as fields, with *Save* and *Cancel* under them.
fn edit_group(ctx: &mut Ctx, work: &Work, account: &ApiAccount) -> Element {
  let page = &work.provider;
  let ApiSetup::Cli { cli, .. } = account.setup() else {
    return Column::new().into();
  };
  let cli = *cli;
  let id = account.id();
  let typed = || -> Option<ApiSetup> {
    let url = base_url(&page.form.url().get()).ok()?;
    let models: Vec<ModelId> = page
      .form
      .model_ids()
      .iter()
      .filter_map(|model| ModelId::new(model).ok())
      .collect();
    ApiSetup::cli(cli, url, models).ok()
  };
  let state = page.change.state().get();
  let save = ActionButton::new(
    "provider-edit-save",
    ButtonKind::Primary,
    t(ctx, "accounts.rename.save"),
  )
  .icon(Icon::Check);
  let save = if state.is_pending() {
    save.loading()
  } else if typed().is_some() {
    let (form, change) = (page.form.clone(), page.change.clone());
    save.on_click(move || {
      // Read again at the press: a field may have changed under it.
      let url = base_url(&form.url().get()).ok();
      let models: Vec<ModelId> = form
        .model_ids()
        .iter()
        .filter_map(|model| ModelId::new(model).ok())
        .collect();
      if let Some(setup) = url.and_then(|url| ApiSetup::cli(cli, url, models).ok()) {
        change.run(Change::Edit(id, setup));
      }
    })
  } else {
    save.disabled()
  };
  let pending = state.is_pending();
  let error = state
    .error
    .filter(|_| !pending)
    .map(|error| field_error("provider-edit-error", &message_text(ctx, &error)));
  let footer = Row::new()
    .id("provider-edit-footer")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(
      lurq::components::Text::new(&t(ctx, "accounts.api.editNote"))
        .id("provider-edit-note")
        .variant(lurq::app::theme::TypographyStyle::Description)
        .color(PaletteColor::MutedForeground)
        .flex(1.0)
        .min_width(Dimension::Px(0.0)),
    )
    .child(cancel(ctx, work, "provider-edit-cancel", id))
    .child(save);
  let group = SettingGroup::new("provider-edit", t(ctx, "accounts.api.provider"))
    .row(page.form.url_row(ctx, cli.provider()))
    .row(page.form.models_row(ctx));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(group)
    .with_children(error)
    .child(footer)
    .into()
}
