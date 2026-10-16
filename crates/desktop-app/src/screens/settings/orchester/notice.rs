//! The notice in All projects for waiting invites (`Invite arriving · the notice in All projects` in `jUMAF`): who
//! invited the person to what, with *Decline* (asked in place) and *Accept*, the same answers as Settings ›
//! Orchester › Invites; more than one invite says how many and opens that tab.

use lurq::{
  app::{component::Component, ctx::Ctx, theme::SpacingSize},
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_api::cloud::CloudInvite;
use orchester_core::time::Timestamp;

use super::{Invites, follow, invites, words::day};
use crate::{
  navigation::Navigation,
  routes::{OrchesterPanel, OrchesterView, Route},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
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
    column = column.with_children(invites::outcome(ctx, &self.answers));
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
    let (declining, invite_id) = (self.answers.declining.clone(), invite.id);
    let decline = ActionButton::new(
      format!("{id}-decline"),
      ButtonKind::Ghost,
      ctx.t("cloud.invites.decline"),
    )
    .on_click(move || declining.set(invites::Asked(Some(invite_id))));
    let (accept, accepting) = (self.answers.accept.clone(), self.answers.accepting.clone());
    let busy = self.answers.accept.state().get().is_pending();
    let accept_button = ActionButton::new(
      format!("{id}-accept"),
      ButtonKind::Primary,
      ctx.t("cloud.invites.accept"),
    )
    .icon(Icon::Check);
    let accept_button = if busy {
      accept_button.loading()
    } else {
      accept_button.on_click(move || {
        accepting.set(invites::Asked(Some(invite_id)));
        accept.run(invite_id);
      })
    };
    let notice = Notice::new(id.clone(), NoticeTone::Info, invites::title(ctx, invite))
      .icon(Icon::Mail)
      .body(body)
      .action(decline)
      .action(accept_button);
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(notice);
    if self.answers.declining.get().0 == Some(invite.id) {
      column = column.child(invites::decline_question(ctx, &id, invite, &self.answers));
    }
    column.into()
  }
}

/// *3 invites wait for you* with *Review*, which opens Settings › Orchester › Invites.
fn many(ctx: &mut Ctx, count: usize) -> Element {
  let navigation = Navigation::of(ctx);
  Notice::new(
    "all-projects-invites-many",
    NoticeTone::Info,
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
