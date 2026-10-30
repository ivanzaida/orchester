//! A row of *Your teams* and of *Invites* (`Row · Acme` in `allZn`, the invite in `Y5TzXV`): a glyph tile, the title
//! with its badge, the address or where the invite is from under it, and the row's actions at its end. Rows sit in one
//! outlined box, a hairline between two.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::cloud::CloudInvite;
use orchester_core::time::Timestamp;
use orchester_workspace::sharing::team_join::TeamServer;

use super::ways;
use crate::{
  screens::settings::orchester::{day_of, shown_address},
  theme::{ExtraSpacing, ExtraTypography, StartupSpacing, StartupText},
  ui::{
    chip::{BadgeTone, badge},
    icon::Icon,
  },
};

/// One row: its glyph, its title row and the line under it, then `meta` and `actions` at its end.
pub(super) struct Line {
  pub id: String,
  pub glyph: Icon,
  pub title: Element,
  pub sub: Element,
  pub meta: Option<String>,
  pub actions: Vec<Element>,
}

/// A section (`allZn`): its heading over its rows in one box, the box saying `none` when it has no row, and `below`
/// under the box.
pub(super) fn section(id: &str, (heading, none): (&str, &str), rows: Vec<Line>, below: Vec<Element>) -> Element {
  let count = rows.len();
  let empty = (count == 0).then(|| {
    Row::new()
      .id(format!("{id}-none"))
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
      .child(Text::new(none).variant(StartupText::Sub.role()))
  });
  let list = Column::new()
    .id(format!("{id}-list"))
    .width(Dimension::Pct(100.0))
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .with_children(
      rows
        .into_iter()
        .enumerate()
        .map(|(index, line)| row(line, index + 1 == count)),
    )
    .with_children(empty);
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(StartupSpacing::Row.role())
    .child(
      Text::new(heading)
        .id(format!("{id}-heading"))
        .variant(StartupText::Section.role()),
    )
    .child(list)
    .with_children(below)
    .into()
}

fn row(line: Line, last: bool) -> Element {
  let Line {
    id,
    glyph,
    title,
    sub,
    meta,
    actions,
  } = line;
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Fine.role())
    .child(title)
    .child(sub);
  let row = Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(ways::tile(glyph))
    .child(text)
    .with_children(meta.map(|meta| {
      Text::new(&meta)
        .id(format!("{id}-meta"))
        .variant(ExtraTypography::Prose.role())
        .nowrap()
    }))
    .with_children(actions);
  if last {
    row.into()
  } else {
    row
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .into()
  }
}

/// A team's title (`Acme`) and its role there, *Member* or *Owner*.
pub(super) fn team_title(ctx: &Ctx, id: &str, team: &TeamServer) -> Element {
  let (tone, key) = if team.owner {
    (BadgeTone::Neutral, "startup.join.teams.owner")
  } else {
    (BadgeTone::Success, "startup.join.teams.member")
  };
  title_row(
    id,
    &team.server.name,
    Some(badge(&format!("{id}-role"), tone, &ctx.t(key)).into()),
  )
}

/// A team's address under its title, in Geist Mono.
pub(super) fn team_address(id: &str, team: &TeamServer) -> Element {
  Text::new(shown_address(&team.server.address))
    .id(format!("{id}-address"))
    .variant(StartupText::RowAddress.role())
    .nowrap()
    .into()
}

/// What an invite is to (`Nordlicht invited you to Billing`). The cloud keeps no sent time or seen state, so there is
/// no *New* badge.
pub(super) fn invite_title(ctx: &Ctx, id: &str, invite: &CloudInvite) -> Element {
  let server = invite.server.name.as_str();
  let title = match &invite.project {
    Some(project) => ctx.t_args(
      "startup.join.invites.toProject",
      [("server", server), ("project", project.name.as_str())],
    ),
    None => ctx.t_args("startup.join.invites.toServer", [("server", server)]),
  };
  title_row(id, &title, None)
}

/// Who the invite is from, when it expires and where it leads.
pub(super) fn invite_sub(ctx: &Ctx, id: &str, invite: &CloudInvite) -> Element {
  let address = invite
    .project
    .as_ref()
    .map_or(invite.server.address.as_str(), |project| project.address.as_str());
  let expires = day_of(ctx, Timestamp::from_unix_millis(invite.expires_at));
  Text::new(&ctx.t_args(
    "startup.join.invites.sub",
    [
      ("inviter", invite.inviter_display_name.as_str()),
      ("expires", expires.as_str()),
      ("address", shown_address(address)),
    ],
  ))
  .id(format!("{id}-sub"))
  .variant(StartupText::Sub.role())
  .nowrap()
  .into()
}

fn title_row(id: &str, title: &str, badge: Option<Element>) -> Element {
  Row::new()
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Text::new(title)
        .id(format!("{id}-title"))
        .variant(StartupText::RowTitle.role())
        .nowrap(),
    )
    .with_children(badge)
    .into()
}
