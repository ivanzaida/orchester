//! The shown account (`Account details` `vP6kR` in `t098v`): a header with its glyph, name and state, what it is
//! (CLI, email, plan) and *Check again*, *Rename* and *Remove*; the tabs *Account* and *Used by*; and the open tab's
//! body. *Rename* turns the name into a field in the header (`yEJPZ`); *Remove* asks in place over *Used by*
//! (`HzJSR`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{account::AgentAccount, name::Name};
use orchester_workspace::accounts::AccountUses;

use super::{
  Parts, Work, facts,
  list::state_line,
  remove, used_by,
  words::{cli_key, name, plan, t, ta},
  work::Change,
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::{AccountsSpacing, AccountsText, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    field::{FieldInput, field_error},
    icon::{Icon, icon},
    message::message_text,
    tabs::{PageTab, page_tabs},
  },
};

/// The header's glyph tile (`kGUGy`).
const GLYPH_TILE: f32 = 32.0;
/// The name field while renaming (`yEJPZ`).
const NAME_FIELD_WIDTH: f32 = 252.0;

/// The details of the shown account, or nothing when there is none.
pub(super) fn panel(ctx: &mut Ctx, parts: &Parts, work: &Work, uses: Option<&AccountUses>) -> Element {
  let Some(account) = parts.selected.and_then(|id| parts.account(id)) else {
    return Column::new().into();
  };
  let tab = match parts.view.panel {
    AccountsPanel::UsedBy | AccountsPanel::Remove => AccountsPanel::UsedBy,
    _ => AccountsPanel::Account,
  };
  let body = match tab {
    AccountsPanel::UsedBy => used_by_body(ctx, parts, work, account, uses),
    _ => facts::body(ctx, parts, work, account),
  };
  frame("account-details")
    .child(header(ctx, parts, work, account))
    .child(tabs(ctx, account, tab, uses))
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .padding(Padding::all(AccountsSpacing::Panel.role()))
          .spacing(AccountsSpacing::Parts.role())
          .child(body),
      )
      .id("account-details-scroll")
      .flex(1.0),
    )
    .into()
}

/// The right panel's card (`vP6kR`).
pub(super) fn frame(id: &str) -> Column {
  Column::new()
    .id(id.to_owned())
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
}

/// The panel's glyph tile (`kGUGy`).
pub(super) fn glyph(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Nav, PaletteColor::TextSecondary))
}

/// The header's row (`jCbMF`).
pub(super) fn header_row(id: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Panel.role(), SpacingSize::Md))
    .spacing(AccountsSpacing::Cell.role())
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
}

fn header(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let text = if parts.view.panel == AccountsPanel::Rename {
    rename_field(ctx, work, account)
  } else {
    title(ctx, parts, account)
  };
  header_row("account-header")
    .child(glyph(Icon::User))
    .child(text)
    .child(actions(ctx, parts, work, account))
    .into()
}

/// The name with the state after it, and what the account is under them (`apjWO`).
fn title(ctx: &Ctx, parts: &Parts, account: &AgentAccount) -> Element {
  let identity = account.last_check().and_then(|check| check.known_identity());
  let facts: Vec<String> = std::iter::once(t(ctx, cli_key(account.cli())))
    .chain(identity.and_then(|identity| identity.email.clone()))
    .chain(
      identity
        .and_then(|identity| identity.plan.as_deref())
        .map(|plan_name| ta(ctx, "accounts.details.planFact", [("plan", plan(plan_name))])),
    )
    .collect();
  let separator = t(ctx, "accounts.details.factSeparator");
  let mut line = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center);
  for (index, fact) in facts.iter().enumerate() {
    if index > 0 {
      line = line.child(Text::new(&separator).variant(ExtraTypography::Note.role()));
    }
    line = line.child(Text::new(fact).variant(ExtraTypography::Note.role()).nowrap());
  }
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(SpacingSize::Xs)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&name(ctx, account))
            .id("account-title")
            .variant(AccountsText::Title.role())
            .nowrap(),
        )
        .child(state_line(ctx, "account-title-state", parts.shown(account))),
    )
    .child(line.id("account-facts"))
    .into()
}

fn actions(ctx: &mut Ctx, parts: &Parts, work: &Work, account: &AgentAccount) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let check = {
    let work = work.clone();
    ActionButton::new(
      "account-check-again",
      ButtonKind::Secondary,
      t(ctx, "accounts.actions.checkAgain"),
    )
    .icon(Icon::RefreshCw)
    .on_click(move || work.check_again(vec![id]))
  };
  let check = if parts.checking.contains(&id) {
    check.loading()
  } else {
    check
  };
  let to = |panel: AccountsPanel| {
    let navigation = navigation.clone();
    move || navigation.replace(Route::Accounts(AccountsView::of(id, panel)))
  };
  let rename = ActionButton::new("account-rename", ButtonKind::Ghost, t(ctx, "accounts.actions.rename"))
    .icon(Icon::Pencil)
    .on_click(to(AccountsPanel::Rename));
  let remove =
    ActionButton::new("account-remove", ButtonKind::Ghost, t(ctx, "accounts.actions.remove")).icon(Icon::Trash2);
  let remove = if account.is_default() {
    remove.unavailable(remove::default_reason(ctx, account))
  } else {
    remove.on_click(to(AccountsPanel::Remove))
  };
  Row::new()
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(check)
    .child(rename)
    .child(remove)
    .into()
}

/// The name as a field, *Save* and *Cancel*, and what renaming changes (`yEJPZ`).
fn rename_field(ctx: &mut Ctx, work: &Work, account: &AgentAccount) -> Element {
  if work.rename.get().is_empty() {
    work.rename.set(name(ctx, account));
  }
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let typed = work.rename.get();
  let valid = Name::new(&typed).ok();
  let save = {
    let change = work.change.clone();
    let save = ActionButton::new(
      "account-rename-save",
      ButtonKind::Secondary,
      t(ctx, "accounts.rename.save"),
    )
    .icon(Icon::Check);
    match valid {
      Some(name) => save.on_click(move || change.run(Change::Rename(id, name.clone()))),
      None => save.disabled(),
    }
  };
  let save = if work.change.state().get().is_pending() {
    save.loading()
  } else {
    save
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
      navigation.replace(Route::Accounts(AccountsView::of(id, AccountsPanel::Account)));
    })
  };
  let error = work
    .change_error()
    .map(|error| field_error("account-rename-error", &message_text(ctx, &error)));
  Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          FieldInput::new(
            "account-rename-field",
            t(ctx, "accounts.actions.rename"),
            work.rename.clone(),
          )
          .width(Dimension::Px(NAME_FIELD_WIDTH))
          .render(ctx),
        )
        .child(save)
        .child(cancel),
    )
    .with_children(error)
    .child(
      Text::new(&t(ctx, "accounts.rename.note"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0)),
    )
    .into()
}

fn tabs(ctx: &mut Ctx, account: &AgentAccount, tab: AccountsPanel, uses: Option<&AccountUses>) -> Element {
  let navigation = Navigation::of(ctx);
  let id = account.id();
  let to = |panel: AccountsPanel| {
    let navigation = navigation.clone();
    move || navigation.replace(Route::Accounts(AccountsView::of(id, panel)))
  };
  let used = uses.map_or(0, |uses| uses.connections.len());
  let tabs = [
    PageTab::new(
      "account-tab-account",
      t(ctx, "accounts.details.account"),
      tab == AccountsPanel::Account,
      to(AccountsPanel::Account),
    ),
    PageTab::new(
      "account-tab-used-by",
      ta(ctx, "accounts.details.usedBy", [("count", used.to_string())]),
      tab == AccountsPanel::UsedBy,
      to(AccountsPanel::UsedBy),
    ),
  ];
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::horizontal(AccountsSpacing::Panel.role()))
    .child(page_tabs(ctx, "account-tabs", tabs))
    .into()
}

/// *Used by*, with *Remove …?* over it while the person removes the account.
fn used_by_body(
  ctx: &mut Ctx,
  parts: &Parts,
  work: &Work,
  account: &AgentAccount,
  uses: Option<&AccountUses>,
) -> Element {
  let confirm =
    (parts.view.panel == AccountsPanel::Remove).then(|| remove::confirmation(ctx, parts, work, account, uses));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(AccountsSpacing::Parts.role())
    .with_children(confirm)
    .child(used_by::list(ctx, uses))
    .into()
}
