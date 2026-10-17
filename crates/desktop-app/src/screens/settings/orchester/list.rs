//! The list (`Server list` `A8j4cm` in `Cu20b`): a card with the *Orchester Cloud* group over the account's row and
//! the *Team servers* group over a row per server (`Account group head` `I6Ebu`, `Account list row` `LW6D5`: a 28
//! glyph tile, the name with the state at its end, the address and how it is trusted, and the projects linked on this
//! computer), the shown one in the `hover` fill, and the note under the card. Without servers the group holds *No team
//! servers yet* with *Connect a server* (`Aowlk`).

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, Style, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_workspace::{
  display,
  sharing::{Connection, LinkedProject, ServerEntry},
};

use super::{
  Invites, Snapshot,
  words::{Look, counted, server_look},
};
use crate::{
  navigation::{Navigation, link::Open},
  routes::{OrchesterPanel, OrchesterView, Route},
  theme::{AccountsSpacing, AccountsText, ExtraColor, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector,
    pressable::opening,
  },
};

/// The list's width (`A8j4cm`) and a row's glyph tile (`LW6D5`).
const WIDTH: f32 = 380.0;
const GLYPH_TILE: f32 = 28.0;

pub(super) fn column(ctx: &mut Ctx, snapshot: &Snapshot, (invites, view): (&Invites, OrchesterView)) -> Element {
  let navigation = Navigation::of(ctx);
  let cloud_meta = if snapshot.account.is_some() {
    ctx.t("cloud.list.signedIn")
  } else {
    ctx.t("cloud.list.notSignedIn")
  };
  let servers_meta = counted(
    ctx,
    ("cloud.list.servers.one", "cloud.list.servers.other"),
    snapshot.servers.len(),
  );
  let mut card = Column::new()
    .id("orchester-list")
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(group_head(
      "orchester-group-cloud",
      Icon::Cloud,
      &ctx.t("cloud.list.cloud"),
      &cloud_meta,
    ))
    .child(cloud_row(ctx, snapshot, invites, view.server.is_none(), &navigation))
    .child(group_head(
      "orchester-group-servers",
      Icon::Server,
      &ctx.t("cloud.list.teamServers"),
      &servers_meta,
    ));
  if snapshot.servers.is_empty() {
    card = card.child(no_servers(ctx, &navigation));
  }
  for (entry, linked) in snapshot.servers.iter().zip(&snapshot.linked) {
    let selected = view.server == Some(entry.stored.id);
    card = card.child(server_row(ctx, (entry, linked), selected, &navigation));
  }
  let note = if view.server.is_some() {
    ctx.t("cloud.list.serverNote")
  } else {
    ctx.t("cloud.list.note")
  };
  Column::new()
    .width(Dimension::Px(WIDTH))
    .flex_shrink(0.0)
    .spacing(AccountsSpacing::Cell.role())
    .child(card)
    .child(inspector::note("orchester-note", Icon::Info, &note))
    .into()
}

fn group_head(id: &str, glyph: Icon, name: &str, meta: &str) -> Row {
  Row::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Edge.role(), SpacingSize::Sm))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceBase)
    .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .child(icon(glyph, IconSize::Sm, PaletteColor::TextMuted))
    .child(Text::new(name).variant(AccountsText::GroupName.role()))
    .child(Spacer::new().flex(1.0))
    .child(
      Text::new(meta)
        .id(format!("{id}-meta"))
        .variant(TypographyStyle::Caption),
    )
}

fn cloud_row(ctx: &Ctx, snapshot: &Snapshot, invites: &Invites, selected: bool, navigation: &Navigation) -> Element {
  let host = snapshot.address.address.authority();
  let (name, second, third, state) = match &snapshot.account {
    Some(account) => {
      let waiting = invites.waiting();
      let third = if waiting > 0 {
        counted(
          ctx,
          ("cloud.list.invitesWaiting.one", "cloud.list.invitesWaiting.other"),
          waiting,
        )
      } else {
        host
      };
      let state = Look {
        icon: Icon::CircleCheck,
        icon_color: PaletteColor::Success,
        key: "cloud.state.signedIn",
        color: PaletteColor::TextSecondary,
        name: "signed-in",
      };
      (account.display_name.clone(), account.email.clone(), third, state)
    }
    None => {
      let state = Look {
        icon: Icon::CircleDashed,
        icon_color: PaletteColor::TextMuted,
        key: "cloud.state.notSignedIn",
        color: PaletteColor::TextMuted,
        name: "signed-out",
      };
      (
        ctx.t("cloud.list.cloud").to_string(),
        ctx.t("cloud.list.signInFor").to_string(),
        host,
        state,
      )
    }
  };
  let glyph = if snapshot.account.is_some() {
    Icon::User
  } else {
    Icon::Cloud
  };
  row(
    ctx,
    ("orchester-row-cloud", glyph, selected),
    (&name, &second, &third),
    &state,
    navigation.to(Route::Orchester(OrchesterView::default())),
  )
}

fn server_row(
  ctx: &Ctx,
  (entry, linked): (&ServerEntry, &[LinkedProject]),
  selected: bool,
  navigation: &Navigation,
) -> Element {
  let id = entry.stored.id;
  let second = ctx.t_args(trust_key(entry), [("host", entry.address.authority())]);
  row(
    ctx,
    (&format!("orchester-row-{id}"), Icon::Server, selected),
    (&entry.stored.name, &second, &third_line(ctx, entry, linked)),
    &server_look(entry),
    navigation.to(Route::Orchester(OrchesterView::server(id))),
  )
}

/// How the server is trusted, after its address: *certificate checked*, *system trust* or *plain HTTP*.
pub(super) fn trust_key(entry: &ServerEntry) -> &'static str {
  match (&entry.stored.certificate_pin, entry.stored.plain_http_allowed) {
    (Some(_), _) => "cloud.list.trust.pinned",
    (None, false) if entry.address.is_https() => "cloud.list.trust.system",
    (None, _) => "cloud.list.trust.plain",
  }
}

/// What the row says last: what is wrong, or the projects on this computer, or how it was joined.
fn third_line(ctx: &Ctx, entry: &ServerEntry, linked: &[LinkedProject]) -> String {
  let names = linked
    .iter()
    .map(|project| project.name.as_str().to_owned())
    .collect::<Vec<_>>()
    .join(", ");
  if entry.certificate_changed {
    return ctx.t("cloud.list.line.certificateChanged").to_string();
  }
  match entry.connection {
    None | Some(Connection::SignedOut) => ctx.t("cloud.list.line.signedOut").to_string(),
    Some(Connection::Reconnecting { attempt, .. }) => ctx
      .t_args("cloud.list.line.reconnecting", [("attempt", attempt.to_string())])
      .to_string(),
    Some(Connection::Offline { since, .. }) => ctx
      .t_args("cloud.list.line.offline", [("since", display::clock(since))])
      .to_string(),
    Some(Connection::Checking | Connection::Connected) if names.is_empty() => {
      ctx.t("cloud.list.line.noProjects").to_string()
    }
    Some(Connection::Checking | Connection::Connected) => ctx
      .t_args("cloud.list.line.projects", [("projects", names)])
      .to_string(),
  }
}

/// The glyph tile of a row (`LW6D5`).
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

/// The state, icon then word, in the state's colors.
pub(super) fn state_line(ctx: &Ctx, id: &str, look: &Look) -> Row {
  Row::new()
    .id(id.to_owned())
    .flex_shrink(0.0)
    .spacing(ExtraSpacing::Pair.role())
    .align_items(Alignment::Center)
    .describe("state", look.name)
    .child(icon(look.icon, IconSize::Sm, look.icon_color.clone()))
    .child(
      Text::new(&ctx.t(look.key))
        .variant(AccountsText::State.role())
        .color(look.color.clone())
        .nowrap(),
    )
}

fn row(
  ctx: &Ctx,
  (id, glyph, selected): (&str, Icon, bool),
  (name, second, third): (&str, &str, &str),
  look: &Look,
  open: Open,
) -> Element {
  let lines = Column::new()
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
        .child(state_line(ctx, &format!("{id}-state"), look)),
    )
    .child(
      Text::new(second)
        .id(format!("{id}-identity"))
        .variant(AccountsText::Identity.role())
        .width(Dimension::Pct(100.0))
        .nowrap(),
    )
    .child(
      Text::new(third)
        .id(format!("{id}-line"))
        .variant(ExtraTypography::Note.role())
        .width(Dimension::Pct(100.0))
        .nowrap(),
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
    .child(tile(glyph))
    .child(lines);
  let row = if selected {
    row.background(ExtraColor::Hover.role())
  } else {
    row
  };
  opening(open)
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .describe("selected", if selected { "true" } else { "false" })
    .hovered_style(Style::new().background(ExtraColor::Hover.role()))
    .child(row)
    .into()
}

/// *No team servers yet* (`Connected servers · none yet` in `Aowlk`).
fn no_servers(ctx: &Ctx, navigation: &Navigation) -> Element {
  let navigation = navigation.clone();
  Column::new()
    .id("orchester-no-servers")
    .width(Dimension::Pct(100.0))
    .padding(Padding::all(AccountsSpacing::Panel.role()))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(tile(Icon::Server))
    .child(Text::new(&ctx.t("cloud.list.none.title")).variant(ExtraTypography::Strong.role()))
    .child(
      Text::new(&ctx.t("cloud.list.none.body"))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      ActionButton::new(
        "orchester-no-servers-connect",
        ButtonKind::Primary,
        ctx.t("settings.connectServer"),
      )
      .icon(Icon::Plus)
      .opens(navigation.to(Route::Orchester(OrchesterView {
        server: None,
        panel: OrchesterPanel::Connect,
      }))),
    )
    .into()
}
