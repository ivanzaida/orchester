//! An account's page (`03b · Account` `xN1Jy`): the crumb back to Accounts; the head with the CLI maker's mark, the
//! account's name and state, *Check again* and *…* (*Rename*, *Sign in (again) in a terminal*, *Remove*); what its
//! state means and how to fix it under the head (`state`); *Remove …?* asked in place there (`remove`); its sign-in
//! facts (`facts`), its plan limits (`limits`) and what uses it (`used_by`). *Rename* turns the name into a field in
//! the head with *Save* and *Cancel* (`Rename` `GWyCi` in `poD7g`). Default can't be removed: its *Remove* says why.

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  account::{AccountState, AgentAccount},
  name::Name,
};
use orchester_workspace::{accounts::AccountUses, display};

use super::{
  Parts, Work, facts, limits,
  list::state_line,
  remove, sign_in, state, used_by,
  words::{Shown, cli_key, name, t, ta},
  work::Change,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::IconSize,
  ui::{
    agent_mark::{AgentMark, mark_icon},
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::Icon,
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    message::message_text,
    setting_rows::{group_note, head_tile},
  },
};

/// The name's field while renaming (`Field` `j7t2Ea` in `poD7g`).
const NAME_FIELD_WIDTH: f32 = 240.0;

/// The page of the account the route names; `uses` is what uses it and whether that is still being read.
pub(super) fn page(ctx: &mut Ctx, parts: &Parts, work: &Work, (uses, reading): (Option<&AccountUses>, bool)) -> Column {
  let navigation = Navigation::of(ctx);
  let pane = super::super::frame::pane("account-page").child(super::super::frame::crumb(
    "account-crumb",
    &t(ctx, "settings.tabs.accounts"),
    navigation.to(Route::Accounts(AccountsView::default())),
  ));
  let Some(account) = parts.selected.and_then(|id| parts.account(id)) else {
    return pane;
  };
  let shown = parts.shown(account);
  let head = if parts.view.panel == AccountsPanel::Rename {
    rename_head(ctx, work, account)
  } else {
    head(ctx, parts, work, account)
  };
  let question = (parts.view.panel == AccountsPanel::Remove && !account.is_default())
    .then(|| remove::confirmation(ctx, (parts, work), account, (uses, reading)));
  // The sign-in in place says where the account stands while it shows; the state's notice would say it twice.
  let state = if sign_in::follows(parts, work, account) {
    Some(sign_in::panel(ctx, (parts, work), account, false))
  } else {
    state::notice(ctx, (parts, work), account, shown)
  };
  pane
    .child(head)
    .with_children(state)
    .with_children(question)
    .child(facts::group(ctx, account))
    .child(limits::group(ctx, work, account))
    .child(used_by::group(ctx, uses))
}

/// The account as its page heads it: *Claude Code · Work*.
fn title(ctx: &Ctx, account: &AgentAccount) -> String {
  ta(
    ctx,
    "accounts.details.title",
    [("cli", t(ctx, cli_key(account.cli()))), ("name", name(ctx, account))],
  )
}

/// The mark's tile at the head (`Tile` `KG1Td`).
fn tile(ctx: &Ctx, account: &AgentAccount) -> Element {
  head_tile(mark_icon(ctx, AgentMark::of_cli(account.cli()), IconSize::Lg.size())).into()
}

/// The head: the title, the state with when it was last checked, *Check again* and *…*.
fn head(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let shown = parts.shown(account);
  let checked = account.last_check().map(|check| display::clock(check.at));
  let state = state_line(ctx, "account-title-state", shown);
  let line = match checked {
    Some(time) if matches!(shown, Shown::State(_)) => Row::new()
      .spacing(crate::theme::ExtraSpacing::Snug.role())
      .align_items(Alignment::Center)
      .child(state)
      .child(
        lurq::components::Text::new(&ta(ctx, "accounts.details.checkedAt", [("time", time)]))
          .id("account-checked-at")
          .variant(lurq::app::theme::TypographyStyle::Description)
          .nowrap(),
      )
      .into(),
    _ => state,
  };
  let check = super::state::check_again(
    ctx,
    (parts, work),
    account.id(),
    ("account-check-again", ButtonKind::Secondary),
  );
  let more = more(ctx, parts, work, account);
  let tile = tile(ctx, account);
  super::super::frame::head(
    "account-head",
    Some(tile),
    (&title(ctx, account), Some(line)),
    vec![check.into(), more],
  )
  .into()
}

/// *…* and its menu: *Rename*, the sign-in, and *Remove*, which Default can't do and says why.
fn more(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let (open, anchor) = work.more.clone();
  let toggle = open.clone();
  let button = IconButton::new("account-more", Icon::Ellipsis, t(ctx, "accounts.actions.more"))
    .element(anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open));
  let shown = open.get();
  let button = if shown { button.active() } else { button }.render(ctx);
  let menu = shown.then(|| {
    let items = menu_items(ctx, parts, work, account);
    let panel = items.into_iter().fold(menu_panel("account-more-menu"), |panel, item| {
      panel.child(item.render(ctx, &open))
    });
    menu_at_end(anchor, open.clone(), panel)
  });
  Row::new().child(button).with_children(menu).into()
}

fn menu_items(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Vec<MenuItem> {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let to = |panel: AccountsPanel| {
    let navigation = navigation.clone();
    move || navigation.go(Route::Accounts(AccountsView::of(id, panel)))
  };
  let rename = MenuItem::new(
    "account-rename",
    t(ctx, "accounts.actions.rename"),
    to(AccountsPanel::Rename),
  )
  .icon(Icon::Pencil);
  let remove = if account.is_default() {
    MenuItem::unavailable(
      "account-remove",
      t(ctx, "accounts.actions.remove"),
      remove::default_reason(ctx, account),
    )
  } else {
    MenuItem::new(
      "account-remove",
      t(ctx, "accounts.actions.remove"),
      to(AccountsPanel::Remove),
    )
  }
  .icon(Icon::Trash2);
  std::iter::once(rename)
    .chain(std::iter::once(super::state::sign_in_item(ctx, parts, work, account)))
    .chain(std::iter::once(remove))
    .collect()
}

/// The head while renaming: the name as a field with *Save* and *Cancel*, and what renaming changes under it.
fn rename_head(ctx: &mut Ctx, work: &Work, account: &AgentAccount) -> Element {
  if work.rename.get().is_empty() {
    work.rename.set(name(ctx, account));
  }
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let valid = Name::new(&work.rename.get()).ok();
  let save = ActionButton::new(
    "account-rename-save",
    ButtonKind::Primary,
    t(ctx, "accounts.rename.save"),
  );
  let save = if work.change.state().get().is_pending() {
    save.loading()
  } else if let Some(name) = valid {
    let change = work.change.clone();
    save.on_click(move || change.run(Change::Rename(id, name.clone())))
  } else {
    save.disabled()
  };
  let cancel = {
    let rename = work.rename.clone();
    ActionButton::new(
      "account-rename-cancel",
      ButtonKind::Ghost,
      t(ctx, "accounts.rename.cancel"),
    )
    .on_click(move || {
      rename.set(String::new());
      navigation.close_panel(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)));
    })
  };
  let error = work
    .change_error()
    .map(|error| field_error("account-rename-error", &message_text(ctx, &error)));
  let field = FieldInput::new(
    "account-rename-field",
    t(ctx, "accounts.actions.rename"),
    work.rename.clone(),
  )
  .width(Dimension::Px(NAME_FIELD_WIDTH))
  .render(ctx);
  let tile = tile(ctx, account);
  Column::new()
    .id("account-head")
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Sm)
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(lurq::app::theme::SpacingSize::Md)
        .align_items(Alignment::Center)
        .child(tile)
        .child(field)
        .child(save)
        .child(cancel),
    )
    .with_children(error)
    .child(group_note("account-rename-note", &t(ctx, "accounts.rename.note")))
    .into()
}

/// Whether the account is signed in now, as its CLI said.
pub(super) fn signed_in(account: &AgentAccount) -> bool {
  account.state() == AccountState::SignedIn
}
