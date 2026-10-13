//! The list of accounts (`Account list` `TjN4v` in `t098v`): a card with a group per CLI (`Account group head`
//! `I6Ebu`: its glyph, name and how many accounts) over a row per account (`Account list row` `LW6D5`: a 28 glyph
//! tile, the name with the state at its end, who is signed in and the folder), the shown one in the `hover` fill,
//! and the note under the card. While an account is being added, its row reads *Not added yet* under its CLI.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::account::{AccountCli, AgentAccount};

use super::{
  Parts,
  words::{Shown, cli_key, identity, look, name, row_folder, t, ta},
};
use crate::{
  navigation::Navigation,
  routes::{AccountsPanel, AccountsView, Route},
  theme::{AccountsSpacing, AccountsText, ExtraColor, ExtraTypography, IconSize},
  ui::{
    icon::{Icon, icon},
    inspector,
    pressable::pressable,
  },
};

/// The list's width (`TjN4v`) and a row's glyph tile (`FMNVY`).
pub(super) const WIDTH: f32 = 380.0;
const GLYPH_TILE: f32 = 28.0;

/// A row of an account being added: its CLI and the name typed so far.
pub(super) struct Adding {
  pub cli: AccountCli,
  pub name: String,
}

pub(super) fn column(ctx: &mut Ctx, parts: &Parts, adding: Option<&Adding>) -> Element {
  let navigation = Navigation::of(ctx);
  let mut card = Column::new()
    .id("accounts-list")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip();
  for cli in AccountCli::ALL {
    let accounts: Vec<&AgentAccount> = parts.accounts.iter().filter(|account| account.cli() == cli).collect();
    let adding = adding.filter(|adding| adding.cli == cli);
    card = card.child(group_head(ctx, cli, accounts.len()));
    for account in accounts {
      card = card.child(row(ctx, parts, account, &navigation));
    }
    if let Some(adding) = adding {
      card = card.child(adding_row(ctx, adding));
    }
  }
  Column::new()
    .width(Dimension::Px(WIDTH))
    .flex_shrink(0.0)
    .spacing(AccountsSpacing::Cell.role())
    .child(card)
    .child(inspector::note("accounts-note", Icon::Info, &t(ctx, "accounts.note")))
    .into()
}

fn group_head(ctx: &Ctx, cli: AccountCli, count: usize) -> Row {
  let count = if count == 1 {
    t(ctx, "accounts.group.one")
  } else {
    ta(ctx, "accounts.group.other", [("count", count.to_string())])
  };
  Row::new()
    .id(format!("accounts-group-{}", cli.as_str()))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Edge.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceBase)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(Icon::SquareTerminal, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(&t(ctx, cli_key(cli))).variant(AccountsText::GroupName.role()))
    .child(Spacer::new().flex(1.0))
    .child(Text::new(&count).variant(TypographyStyle::Caption))
}

/// The state, icon then word, in the state's colors.
pub(super) fn state_line(ctx: &Ctx, id: &str, shown: Shown) -> Row {
  let look = look(shown);
  Row::new()
    .id(id.to_owned())
    .flex_shrink(0.0)
    .spacing(crate::theme::ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("state", look.key.rsplit('.').next().unwrap_or_default())
    .child(icon(look.icon, IconSize::Sm, look.icon_color))
    .child(
      Text::new(&t(ctx, look.key))
        .variant(AccountsText::State.role())
        .color(look.color)
        .nowrap(),
    )
}

/// The glyph tile of an account (`Glyph` `FMNVY`).
fn tile(glyph: Icon) -> Row {
  Row::new()
    .width(Dimension::Px(GLYPH_TILE))
    .height(Dimension::Px(GLYPH_TILE))
    .flex_shrink(0.0)
    .align_items(Alignment::Center)
    .justify(Justify::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Sm)
    .child(icon(glyph, IconSize::Md, PaletteColor::TextSecondary))
}

fn row(ctx: &Ctx, parts: &Parts, account: &AgentAccount, navigation: &Navigation) -> Element {
  let id = format!("account-row-{}", account.id());
  let shown = parts.shown(account);
  let selected = parts.selected == Some(account.id()) && parts.view.panel != AccountsPanel::Add;
  let text = lines(
    &id,
    (
      &name(ctx, account),
      &identity(ctx, account, &parts.accounts, shown),
      &row_folder(account),
    ),
    state_line(ctx, &format!("{id}-state"), shown),
  );
  let row = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      AccountsSpacing::Edge.role(),
      AccountsSpacing::Cell.role(),
    ))
    .spacing(AccountsSpacing::Cell.role())
    .align_items(Alignment::Start)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(tile(Icon::User))
    .child(text);
  let row = if selected {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  let navigation = navigation.clone();
  let account_id = account.id();
  pressable(move || navigation.replace(Route::Accounts(AccountsView::of(account_id, AccountsPanel::Account))))
    .id(id)
    .width(Dimension::Pct(100.0))
    .describe("selected", if selected { "true" } else { "false" })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}

/// The row of an account *Add account* has not added yet (`Freelance · Not added yet` in `IOyJl`).
fn adding_row(ctx: &Ctx, adding: &Adding) -> Element {
  let state = Row::new()
    .id("account-row-adding-state")
    .spacing(crate::theme::ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::CircleDashed, IconSize::Sm, PaletteColor::TextMuted))
    .child(
      Text::new(&t(ctx, "accounts.status.notAdded"))
        .variant(AccountsText::State.role())
        .color(PaletteColor::TextMuted),
    );
  let identity = ta(ctx, "accounts.identity.adding", [("cli", t(ctx, cli_key(adding.cli)))]);
  Row::new()
    .id("account-row-adding")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      AccountsSpacing::Edge.role(),
      AccountsSpacing::Cell.role(),
    ))
    .spacing(AccountsSpacing::Cell.role())
    .align_items(Alignment::Start)
    .background(ExtraColor::Hover.role())
    .child(tile(Icon::Plus))
    .child(lines("account-row-adding", (&adding.name, &identity, ""), state))
    .into()
}

/// The name with the state at its end, then who is signed in and the folder (`qcuSX`).
fn lines(id: &str, (name, identity, folder): (&str, &str, &str), state: Row) -> Column {
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(AccountsSpacing::Lines.role())
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(name)
            .id(format!("{id}-name"))
            .variant(ExtraTypography::OptionTitle.role())
            .flex(1.0)
            .nowrap(),
        )
        .child(state),
    )
    .child(
      Text::new(identity)
        .id(format!("{id}-identity"))
        .variant(AccountsText::Identity.role())
        .width(Dimension::Pct(100.0))
        .nowrap(),
    )
    .with_children((!folder.is_empty()).then(|| {
      Text::new(folder)
        .id(format!("{id}-folder"))
        .variant(ExtraTypography::MonoNote.role())
        .nowrap()
    }))
}
