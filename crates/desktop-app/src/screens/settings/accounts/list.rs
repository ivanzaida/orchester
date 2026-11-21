//! The list of accounts (`03 · Accounts` `t8i6m`; `Agents on accounts` A1 `aOR4w`): a group per CLI, its heading with
//! how many accounts it has (and *Add account* at the first one's end, a menu of two), the CLI's program under it, a
//! row per account with the CLI maker's mark in its tile, its name, who is signed in under it, its state and a chevron:
//! a press opens its page. The accounts the person added come first, Default (the CLI's own folder) last. The note
//! under the groups says whose accounts they are.

use lurq::{
  app::{ctx::Ctx, theme::TypographyStyle},
  components::{Column, Row},
  node::Element,
};
use orchester_core::account::{AccountCli, AccountState, AgentAccount};

use super::{
  Parts, Work, api, program,
  provider_rows::Rows,
  words::{Shown, cli_key, identity, look, name, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  services::accounts,
  theme::{IconSize, SettingsSpacing},
  ui::{
    agent_mark::AgentMark,
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_at_end, menu_panel},
    setting_rows::{SettingGroup, SettingRow, group_note, opens_mark, row_tile, status},
    skeleton::{self, Skeleton, fading},
  },
};

/// The width of the menu *Add account* opens (`Menu` `Qa7Oo` on A2 `It53u`).
const ADD_MENU_WIDTH: f32 = 240.0;

/// The list page's column.
pub(super) fn page(ctx: &mut Ctx, parts: &Parts, work: &Work) -> Column {
  let navigation = Navigation::of(ctx);
  let api = accounts::api_listed(ctx);
  let decision = accounts::decision_listed(ctx);
  let here = accounts::on_this_computer(ctx);
  let servers = &work.servers;
  let api_rows = &Rows {
    navigation: navigation.clone(),
    missing: here.as_ref().map(|here| here.keys_missing.clone()),
  };
  let mut pane = super::frame::pane("settings-accounts");
  for (index, cli) in AccountCli::ALL.into_iter().enumerate() {
    // Default last: the accounts the person added come first.
    let mut accounts: Vec<&AgentAccount> = parts.accounts.iter().filter(|account| account.cli() == cli).collect();
    accounts.sort_by_key(|account| account.is_default());
    let count = if accounts.len() == 1 {
      t(ctx, "accounts.group.one")
    } else {
      ta(ctx, "accounts.group.other", [("count", accounts.len().to_string())])
    };
    let count = match api::count(ctx, &api.0, cli) {
      Some(providers) => ta(
        ctx,
        "accounts.api.group",
        [("accounts", count), ("providers", providers)],
      ),
      None => count,
    };
    let mut rows: Vec<_> = accounts
      .into_iter()
      .map(|account| row(ctx, parts, account, &navigation))
      .collect();
    rows.extend(api::rows(ctx, &api.0, cli, api_rows));
    let mut group = SettingGroup::new(format!("accounts-group-{}", cli.as_str()), t(ctx, cli_key(cli)))
      .meta(count)
      .rows(rows);
    if let Some(line) = program::line(ctx, here.as_ref(), cli, &work.program) {
      group = group.line(line);
    }
    if index == 0 {
      group = group.end(add_menu(ctx, work, &navigation));
    }
    pane = pane.child(group);
  }
  pane
    .with_children(api::model_apis(ctx, &api.0, (servers, &decision.0), api_rows))
    .with_children(servers.question(ctx, &decision.0))
    .child(group_note("accounts-note", &t(ctx, "accounts.note")))
}

/// *Add account* and the menu of two it opens (`Agents on accounts` A2 `It53u`): *Sign in to another account*, and
/// *Add an API provider*.
fn add_menu(ctx: &mut Ctx, work: &Work, navigation: &Navigation) -> Element {
  let (open, anchor) = work.add_menu.clone();
  let toggle = open.clone();
  let button = ActionButton::new("settings-add-account", ButtonKind::Ghost, t(ctx, "accounts.addAccount"))
    .icon(Icon::Plus)
    .in_heading()
    .element_ref(anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open));
  let menu = open.get().then(|| {
    let to = |panel: AccountsPanel| {
      let navigation = navigation.clone();
      move || navigation.go(Route::Accounts(AccountsView::panel(panel)))
    };
    let items = [
      MenuItem::new(
        "settings-add-sign-in",
        t(ctx, "accounts.addMenu.signIn"),
        to(AccountsPanel::Add),
      )
      .icon(Icon::UserRound),
      MenuItem::new(
        "settings-add-api",
        t(ctx, "accounts.addMenu.apiProvider"),
        to(AccountsPanel::AddApi),
      )
      .icon(Icon::Cloud),
    ];
    let panel = items
      .into_iter()
      .fold(menu_panel("settings-add-menu").width(ADD_MENU_WIDTH), |panel, item| {
        panel.child(item.render(ctx))
      });
    menu_at_end(anchor, open.clone(), panel)
  });
  Row::new().child(button).with_children(menu).into()
}

/// Each CLI's accounts while they are read: each one's name, who is signed in under it and its state.
const READING: [(AccountCli, skeleton::RowBones); 2] = [
  (AccountCli::ClaudeCode, &[(30.0, 144.0, 52.0), (34.0, 270.0, 52.0)]),
  (AccountCli::Codex, &[(24.0, 180.0, 56.0), (34.0, 298.0, 44.0)]),
];

/// The list while the accounts are read (`14 · App settings · Accounts · loading` `SXKNI`): Add account works, each
/// CLI's group keeps its heading and every row the CLI maker's mark; the counts and each account's name, its folder or
/// e-mail line and its sign-in state are bones, the lower group fading. *Reading the accounts…* (`accounts.reading`)
/// names it.
pub(in crate::screens::settings) fn reading(ctx: &mut Ctx) -> Element {
  let navigation = Navigation::of(ctx);
  let groups: Vec<Column> = READING
    .iter()
    .enumerate()
    .map(|(index, (cli, accounts))| {
      let rows: Vec<_> = accounts
        .iter()
        .enumerate()
        .map(|(at, (name, line, state))| {
          let end = lurq::components::Row::new()
            .spacing(lurq::app::theme::SpacingSize::Md)
            .align_items(lurq::layout::Alignment::Center)
            .child(skeleton::status(ctx, *state))
            .child(opens_mark());
          SettingRow::new(format!("account-reading-{}-{at}", cli.as_str()), "")
            .label_reading(*name)
            .hint_reading(*line)
            .lead(mark_tile(ctx, *cli))
            .control(end)
            .render(ctx)
        })
        .collect();
      let mut group = SettingGroup::new(format!("accounts-reading-{}", cli.as_str()), t(ctx, cli_key(*cli)))
        .meta_reading(skeleton::line(ctx, TypographyStyle::Description, 60.0))
        .rows(rows);
      if index == 0 {
        group = group.end(
          ActionButton::new("settings-add-account", ButtonKind::Ghost, t(ctx, "accounts.addAccount"))
            .icon(Icon::Plus)
            .in_heading()
            .opens(navigation.to(Route::Accounts(AccountsView::panel(AccountsPanel::Add)))),
        );
      }
      Column::new()
        .width(lurq::node::dimension::Dimension::Pct(100.0))
        .child(group)
    })
    .collect();
  let pane = Column::new()
    .width(lurq::node::dimension::Dimension::Pct(100.0))
    .spacing(SettingsSpacing::Page.role())
    .with_children(fading(groups))
    .child(group_note("accounts-note", &t(ctx, "accounts.note")));
  Skeleton::new("settings-accounts-loading", t(ctx, "accounts.reading")).show(ctx, pane)
}

/// An account's state as a dot and its word, in the state's colors.
pub(super) fn state_line(ctx: &Ctx, id: &str, shown: Shown) -> Element {
  let look = look(shown);
  status(id, (look.icon_color, look.color), &t(ctx, look.key))
    .describe("state", look.key.rsplit('.').next().unwrap_or_default())
    .into()
}

/// The CLI maker's mark in a row's tile.
pub(super) fn mark_tile(ctx: &Ctx, cli: AccountCli) -> Element {
  row_tile(AgentMark::of_cli(cli).render(ctx, IconSize::Md)).into()
}

fn row(ctx: &mut Ctx, parts: &Parts, account: &AgentAccount, navigation: &Navigation) -> Element {
  let id = format!("account-row-{}", account.id());
  let shown = parts.shown(account);
  let line = identity(ctx, account, &parts.accounts, shown);
  // Default says it is the CLI's own folder before who is signed in (`t8LAtj`); signed out, its line says so already.
  let signed_out = matches!(
    shown,
    Shown::State(AccountState::NotSignedIn | AccountState::NotChecked)
  );
  let line = if account.is_default() && !signed_out {
    ta(ctx, "accounts.identity.ownFolder", [("identity", line)])
  } else {
    line
  };
  let state = lurq::components::Row::new()
    .spacing(lurq::app::theme::SpacingSize::Md)
    .align_items(lurq::layout::Alignment::Center)
    .child(state_line(ctx, &format!("{id}-state"), shown))
    .child(opens_mark());
  SettingRow::new(id, name(ctx, account))
    .lead(mark_tile(ctx, account.cli()))
    .hint(line)
    .control(state)
    .opens(navigation.to(Route::Accounts(AccountsView::of(account.id(), AccountsPanel::Account))))
    .render(ctx)
}
