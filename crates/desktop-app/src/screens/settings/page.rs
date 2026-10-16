//! The header of Settings (`Header` `sr6zu` and `Tabs` `e6T2Vd` in `t098v`): the title over what Settings is, the open
//! tab's action at the right (*Add account* on Accounts), and the tabs on their line.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use super::SettingsTab;
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, OrchesterView, Route, Screen},
  theme::{ExtraSpacing, ExtraTypography, SettingsText},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    tabs::{PageTab, counted, page_tabs},
  },
};

/// The title, what Settings is, the action and the tabs; `accounts` is how many accounts there are, once read.
pub(super) fn header(ctx: &mut Ctx, tab: SettingsTab, accounts: Option<usize>) -> Element {
  let navigation = Navigation::of(ctx);
  let title = Column::new()
    .flex(1.0)
    .spacing(ExtraSpacing::Snug.role())
    .child(Text::new(&ctx.t(Screen::Settings.title_key())).variant(ExtraTypography::Display.role()))
    .child(
      Text::new(&ctx.t("settings.sub"))
        .id("settings-sub")
        .variant(SettingsText::Lead.role())
        .width(Dimension::Pct(100.0)),
    );
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
    .on_click(move || {
      navigation.go(Route::Accounts(AccountsView {
        account: None,
        panel: AccountsPanel::Add,
      }));
    })
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
      .on_click(move || navigation.go(Route::Orchester(OrchesterView::connect())))
    })
  });
  let head = Row::new()
    .id("settings-head")
    .width(Dimension::Pct(100.0))
    .align_items(Alignment::End)
    .spacing(SpacingSize::Md)
    .child(title)
    .with_children(action);
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Lg)
    .child(head)
    .child(tabs(ctx, tab, accounts, &navigation))
    .into()
}

fn tabs(ctx: &mut Ctx, tab: SettingsTab, accounts: Option<usize>, navigation: &Navigation) -> Element {
  let not_built = ctx.t("settings.tabs.notBuilt");
  let accounts_label = ctx.t("settings.tabs.accounts");
  let accounts_label = match accounts {
    Some(count) => counted(ctx, &accounts_label, count),
    None => accounts_label,
  };
  let to_automation = navigation.clone();
  let to_accounts = navigation.clone();
  let to_orchester = navigation.clone();
  let on_accounts = matches!(tab, SettingsTab::Accounts(_));
  let on_orchester = matches!(tab, SettingsTab::Orchester(_));
  let tabs = [
    PageTab::unavailable("settings-tab-general", ctx.t("settings.tabs.general"), not_built),
    PageTab::new(
      "settings-tab-automation",
      ctx.t("settings.tabs.automation"),
      tab == SettingsTab::Automation,
      move || to_automation.go(Route::Settings),
    ),
    PageTab::new("settings-tab-accounts", accounts_label, on_accounts, move || {
      if !on_accounts {
        to_accounts.go(Route::Accounts(AccountsView::default()));
      }
    }),
    PageTab::new(
      "settings-tab-orchester",
      ctx.t("settings.tabs.orchester"),
      on_orchester,
      move || {
        if !on_orchester {
          to_orchester.go(Route::Orchester(OrchesterView::default()));
        }
      },
    ),
  ];
  page_tabs(ctx, "settings-tabs", tabs).into()
}
