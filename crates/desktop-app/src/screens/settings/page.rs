//! The page of Settings (`Header` `sr6zu` and `Tabs` `e6T2Vd` in `t098v`), in the page it shares with Project
//! settings: the title over what Settings is, the open tab's action at the right (*Add account* on Accounts), the
//! tabs on their line and the open tab under them.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};

use super::SettingsTab;
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, OrchesterView, Route, Screen},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    settings_page::SettingsPage,
    tabs::{PageTab, counted, page_tabs},
  },
};

/// Settings on `tab`, showing `body`; `accounts` is how many accounts there are, once read.
pub(super) fn page(ctx: &mut Ctx, tab: SettingsTab, accounts: Option<usize>, body: Element) -> Element {
  let navigation = Navigation::of(ctx);
  // While adding an account the panel is the way on (`IOyJl`): no second *Add account*.
  let adding = matches!(tab, SettingsTab::Accounts(view) if view.panel == AccountsPanel::Add);
  let action = (matches!(tab, SettingsTab::Accounts(_)) && !adding).then(|| {
    let navigation = navigation.clone();
    ActionButton::new(
      "settings-add-account",
      ButtonKind::Primary,
      ctx.t("accounts.addAccount"),
    )
    .icon(Icon::Plus)
    .opens(navigation.to(Route::Accounts(AccountsView {
      account: None,
      panel: AccountsPanel::Add,
    })))
  });
  let action = action.or_else(|| {
    matches!(tab, SettingsTab::Orchester(_)).then(|| {
      let navigation = navigation.clone();
      ActionButton::new(
        "settings-connect-server",
        ButtonKind::Primary,
        ctx.t("settings.connectServer"),
      )
      .icon(Icon::Plus)
      .opens(navigation.to(Route::Orchester(OrchesterView::connect())))
    })
  });
  SettingsPage::new(
    format!("screen-{}", Screen::Settings.name()),
    ctx.t(Screen::Settings.title_key()),
    ctx.t("settings.sub"),
  )
  .action(action.map(Element::from))
  .tabs(tabs(ctx, tab, accounts, &navigation))
  .body(body)
  .into()
}

fn tabs(ctx: &mut Ctx, tab: SettingsTab, accounts: Option<usize>, navigation: &Navigation) -> Element {
  let not_built = ctx.t("settings.tabs.notBuilt");
  let accounts_label = ctx.t("settings.tabs.accounts");
  let accounts_label = match accounts {
    Some(count) => counted(ctx, &accounts_label, count),
    None => accounts_label,
  };
  // Each tab is a place of its own; the current one keeps the panel it shows, so a press on it opens nothing.
  let tab_to = |id: &str, label: Arc<str>, current: bool, route: Route| {
    if current {
      PageTab::current(id.to_owned(), label)
    } else {
      PageTab::to(id.to_owned(), label, false, navigation.to(route))
    }
  };
  let tabs = [
    PageTab::unavailable("settings-tab-general", ctx.t("settings.tabs.general"), not_built),
    tab_to(
      "settings-tab-automation",
      ctx.t("settings.tabs.automation"),
      tab == SettingsTab::Automation,
      Route::Settings,
    ),
    tab_to(
      "settings-tab-accounts",
      accounts_label,
      matches!(tab, SettingsTab::Accounts(_)),
      Route::Accounts(AccountsView::default()),
    ),
    tab_to(
      "settings-tab-orchester",
      ctx.t("settings.tabs.orchester"),
      matches!(tab, SettingsTab::Orchester(_)),
      Route::Orchester(OrchesterView::default()),
    ),
  ];
  page_tabs(ctx, "settings-tabs", tabs).into()
}
