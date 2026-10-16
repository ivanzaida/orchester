//! The notice in All projects for waiting invites (`Invite arriving · the notice in All projects` in `jUMAF`): who
//! invited the person to what, with *Decline* (asked in place) and *Accept*, the same answers as Settings ›
//! Orchester › Invites; more than one invite says how many and opens that tab.

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::cloud::CloudInvite;
use orchester_core::time::Timestamp;

use super::{Invites, follow, invites, words::day};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::{Icon, icon},
  },
};

pub struct InvitesNotice {
  answers: invites::Answers,
}

impl Component for InvitesNotice {
  type Props = ();

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      answers: invites::Answers::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let services = Services::of(ctx);
    let revision = follow(ctx, (&services.servers, &services.cloud));
    let account = account(ctx, revision);
    let listed = super::invites(ctx, revision, &services.cloud, account.as_ref());
    let mut column = Column::new()
      .id("all-projects-invites")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm);
    column = column.with_children(invites::outcome(ctx, &self.answers, true));
    let Invites::Listed(waiting) = listed else {
      return column;
    };
    match waiting.as_slice() {
      [] => {}
      [invite] => column = column.child(self.one(ctx, invite)),
      several => column = column.child(many(ctx, several.len())),
    }
    column
  }
}

impl InvitesNotice {
  /// The notice (`All projects notice` `p4WXZT`): a 16 `info` mail icon centred beside who invited the person to
  /// what in 13/600 and, 3 under it, the role, the address and the expiry in 12.5 `text_secondary`, with *Decline*
  /// and *Accept* at the end; `info_muted`, radius 10, 12 over and under and 14 at its sides, no stroke.
  fn one(&self, ctx: &mut Ctx, invite: &CloudInvite) -> Element {
    let id = format!("all-projects-invite-{}", invite.id);
    let address = invite
      .project
      .as_ref()
      .map_or_else(|| invite.server.address.clone(), |project| project.address.clone());
    let role = ctx.t(match invite.role {
      Some(orchester_api::MemberRole::Owner) => "cloud.notice.role.owner",
      Some(orchester_api::MemberRole::Member) => "cloud.notice.role.member",
      None => "cloud.notice.role.server",
    });
    let body = ctx.t_args(
      "cloud.notice.body",
      [
        ("role", role.to_string()),
        ("address", super::words::shown_address(&address).to_owned()),
        ("expires", day(ctx, Timestamp::from_unix_millis(invite.expires_at))),
      ],
    );
    let declining = self.answers.declining.get().0 == Some(invite.id);
    let text = Column::new()
      .id(format!("{id}-text"))
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .spacing(ExtraSpacing::Fine.role())
      .child(
        Text::new(&invites::notice_title(ctx, invite))
          .id(format!("{id}-title"))
          .variant(ExtraTypography::FormSection.role())
          .width(Dimension::Pct(100.0)),
      )
      .child(
        Text::new(&body)
          .id(format!("{id}-body"))
          .variant(ExtraTypography::Lead.role())
          .width(Dimension::Pct(100.0)),
      );
    let notice = Row::new()
      .id(id.clone())
      .width(Dimension::Pct(100.0))
      .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
      .spacing(SpacingSize::Md)
      .align_items(Alignment::Center)
      .background(PaletteColor::InfoMuted)
      .rounded(RadiusSize::Lg)
      .child(icon(Icon::Mail, IconSize::Lg, PaletteColor::Info).id(format!("{id}-icon")))
      .child(text)
      .with_children((!declining).then(|| self.buttons(ctx, &id, invite)));
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Section.role())
      .child(notice)
      .with_children(self.answers.person.reason(ctx, &format!("{id}-accept")));
    if declining {
      column = column.child(invites::decline_question(ctx, &id, invite, &self.answers));
    }
    column.into()
  }

  /// *Decline*, asked in place, and *Accept*.
  fn buttons(&self, ctx: &Ctx, id: &str, invite: &CloudInvite) -> Row {
    let (declining, invite_id) = (self.answers.declining.clone(), invite.id);
    let decline = ActionButton::new(
      format!("{id}-decline"),
      ButtonKind::Ghost,
      ctx.t("cloud.invites.decline"),
    )
    .on_click(move || declining.set(invites::Asked(Some(invite_id))));
    let accept = ActionButton::new(
      format!("{id}-accept"),
      ButtonKind::Primary,
      ctx.t("cloud.invites.accept"),
    )
    .icon(Icon::Check);
    let accept = if self.answers.connecting() {
      accept.loading()
    } else {
      let (answers, invite, button) = (self.answers.clone(), invite.clone(), format!("{id}-accept"));
      accept.on_click(move || answers.accept(&button, &invite))
    };
    Row::new()
      .flex_shrink(0.0)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(decline)
      .child(accept)
  }
}

/// *3 invites wait for you* with *Review*, which opens Settings › Orchester › Invites.
fn many(ctx: &mut Ctx, count: usize) -> Element {
  let navigation = Navigation::of(ctx);
  Callout::new(
    "all-projects-invites-many",
    CalloutTone::Info,
    ctx.t_args("cloud.notice.many", [("count", count.to_string())]),
  )
  .icon(Icon::Mail)
  .body(ctx.t("cloud.notice.manyBody"))
  .action(
    ActionButton::new(
      "all-projects-invites-review",
      ButtonKind::Secondary,
      ctx.t("cloud.notice.review"),
    )
    .on_click(move || {
      navigation.go(Route::Orchester(OrchesterView {
        server: None,
        panel: OrchesterPanel::Invites,
      }));
    }),
  )
  .into()
}

/// The cloud account, read at `revision`.
fn account(ctx: &mut Ctx, revision: u64) -> Option<orchester_store::StoredCloudAccount> {
  let cloud = Services::of(ctx).cloud;
  ctx
    .future((revision, cloud), |(_, cloud)| async move {
      orchester_workspace::background::run(move || cloud.account())
        .await
        .map_err(drop)?
        .map(super::Account)
        .map_err(|error| tracing::warn!(%error, "the cloud account could not be read"))
    })
    .state()
    .get()
    .data
    .and_then(|account| account.0)
}
