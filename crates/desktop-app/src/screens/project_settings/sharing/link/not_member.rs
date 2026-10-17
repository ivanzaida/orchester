//! An address with no project the person is a member of (`Shared there · you are not a member` `XxvCx` in `oOGMT`): a
//! server answers a non-member as if there were no project, so the notice says both, and that nothing was uploaded.
//! Under it what can be known: the owners can't (the server names them only to members, so none is invented), the
//! person's username there to give whoever shared the address, with *Copy*, and *Check again*, which asks the server
//! again and says when it last did.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::time::Timestamp;
use orchester_workspace::sharing::ProjectAddress;

use super::{Choice, Link, Props};
use crate::{
  screens::{
    project_settings::page,
    settings::orchester::{moment, shown_address},
  },
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, OrchesterText},
  ui::{
    button::{ActionButton, ButtonKind},
    code_box::Copier,
    detail_row::{DetailValue, detail_row, detail_row_of},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// *Check again*, which only a person presses.
const CHECK: &str = "sharing-not-member-check";

impl Link {
  /// The notice and what can be known, for `address` as the server answered at `checked`; `again` asks again.
  pub(super) fn not_member(
    &self,
    ctx: &mut Ctx,
    props: &Props,
    (address, checked): (&str, Timestamp),
    again: Option<Choice>,
  ) -> Element {
    let server = ProjectAddress::parse(address).ok();
    let entry = server.as_ref().and_then(|address| {
      props
        .facts
        .servers
        .iter()
        .find(|entry| entry.address == *address.server())
    });
    let name = entry.map_or_else(
      || {
        server
          .as_ref()
          .map(|address| address.server().authority())
          .unwrap_or_default()
      },
      |entry| entry.stored.name.clone(),
    );
    let names = [("server", name.as_str()), ("address", shown_address(address))];
    let notice = Notice::new(
      "sharing-no-project",
      NoticeTone::Info,
      ctx.t_args("sharing.link.notMember.title", names),
    )
    .icon(Icon::UserX)
    .body(ctx.t_args("sharing.link.notMember.body", names));
    let username = entry
      .and_then(|entry| entry.stored.account.as_ref())
      .map(|account| account.username.clone());
    Column::new()
      .id("sharing-not-member")
      .width(Dimension::Pct(100.0))
      .padding(lurq::node::padding::Padding::new().top(SpacingSize::Md))
      .spacing(SpacingSize::Sm)
      .child(notice)
      .child(self.known(ctx, username, (checked, again)))
      .child(page::wide_note(
        "sharing-not-member-note",
        Icon::Info,
        &ctx.t_args("sharing.link.notMember.note", [("server", name.as_str())]),
      ))
      .into()
  }

  /// Owners (not known), the person's username with *Copy*, and *Check again* with when it was checked.
  fn known(&self, ctx: &mut Ctx, username: Option<String>, (checked, again): (Timestamp, Option<Choice>)) -> Element {
    let owners = detail_row(
      "sharing-not-member-owners",
      &ctx.t("sharing.link.notMember.owners"),
      &ctx.t("sharing.link.notMember.ownersUnknown"),
      DetailValue::Text,
    );
    let username_row = username.map(|username| {
      let copier = Copier::of(ctx);
      let copied = username.clone();
      let value = Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(
          Text::new(&username)
            .id("sharing-not-member-username")
            .variant(OrchesterText::Address.role())
            .color(PaletteColor::TextPrimary),
        )
        .child(
          ActionButton::new("sharing-not-member-copy", ButtonKind::Ghost, ctx.t("common.copy"))
            .icon(Icon::Copy)
            .on_click(move || {
              if !copier.copy(&copied) {
                tracing::warn!("the username could not be copied");
              }
            }),
        );
      detail_row_of(
        "sharing-not-member-username-row",
        &ctx.t("sharing.link.notMember.username"),
        value,
      )
    });
    let check =
      ActionButton::new(CHECK, ButtonKind::Secondary, ctx.t("sharing.link.notMember.checkAgain")).icon(Icon::RefreshCw);
    let check = match again {
      _ if self.resolve.state().get().is_pending() => check.loading(),
      // It asks the server again and may open what linking does: a person's, as *Link to …* is.
      Some(choice) => {
        let (resolve, person) = (self.resolve.clone(), self.person.clone());
        check.on_click(move || {
          if person.allows(CHECK) {
            resolve.run(choice.clone());
          }
        })
      }
      None => check.disabled(),
    };
    Column::new()
      .id("sharing-not-member-known")
      .width(Dimension::Pct(100.0))
      .padding(ExtraSpacing::Inset.role())
      .spacing(SpacingSize::Sm)
      .background(PaletteColor::SurfaceRaised)
      .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .rounded(ExtraRadius::Tile.role())
      .child(owners)
      .with_children(username_row)
      .child(
        Row::new()
          .spacing(SpacingSize::Md)
          .align_items(Alignment::Center)
          .child(check)
          .child(
            Text::new(&ctx.t_args("sharing.link.notMember.checked", [("time", moment(ctx, checked))]))
              .id("sharing-not-member-checked")
              .variant(ExtraTypography::Note.role()),
          ),
      )
      .with_children(self.person.reason(ctx, CHECK))
      .into()
  }
}
