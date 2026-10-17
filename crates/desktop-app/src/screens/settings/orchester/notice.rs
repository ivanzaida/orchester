//! The notice in All projects for waiting invites (`Invite arriving · the notice in All projects` in `jUMAF`): who
//! invited the person to what, with *Decline* (asked in place) and *Accept*, the same answers as Settings ›
//! Orchester › Invites; more than one invite says how many and opens that tab. All projects keeps the notice's reads
//! and answers and puts the notice in its page only while it has something to show, so an empty notice takes no
//! room in the page's spaced column (`S5HDKf`) and an answer under way outlives the notice. An invite that needs
//! two-factor authentication says why under its line and offers the step that fixes it (`qwDH9` cells 2 and 4,
//! [`super::two_factor`]); few recovery codes left after a sign-in with one are said here too.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_api::cloud::{CloudInvite, TwoFactorStatus};
use orchester_core::time::Timestamp;

use super::{
  Invites, Waiting, follow, invites,
  two_factor::{self, Need},
  words::day,
};
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

/// The notice's answers, which the page that shows it keeps.
pub struct InvitesNotice {
  answers: invites::Answers,
}

impl InvitesNotice {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      answers: invites::Answers::new(ctx),
    }
  }

  /// The invites waiting for the cloud account and its two-factor authentication, read again when a server's
  /// connection or the account changes. The page asks for them in every render, before what it asks only sometimes:
  /// lurq keeps a component's futures by the order it asks for them.
  pub fn read(ctx: &mut Ctx) -> Waiting {
    let services = Services::of(ctx);
    let revision = follow(ctx, (&services.servers, &services.cloud));
    let account = account(ctx, revision);
    let invites = super::invites(ctx, revision, &services.cloud, account.as_ref());
    let two_factor = two_factor::read(ctx, revision, &services.cloud, account.as_ref());
    Waiting { invites, two_factor }
  }

  /// The notice for `waiting`: few recovery codes left, what answering an invite came to, then the waiting invite or
  /// how many wait; `None` when there is nothing to show.
  pub fn notice(&self, ctx: &mut Ctx, waiting: &Waiting) -> Option<Element> {
    let mut parts: Vec<Element> = two_factor::recovery_warning(
      ctx,
      "all-projects-recovery",
      (waiting.two_factor.as_ref(), &self.answers),
    )
    .into_iter()
    .collect();
    parts.extend(invites::outcome(ctx, &self.answers, true));
    if let Invites::Listed(listed) = &waiting.invites {
      match listed.as_slice() {
        [] => {}
        [invite] => parts.push(self.one(ctx, invite, waiting.two_factor.as_ref())),
        several => parts.push(many(ctx, several.len())),
      }
    }
    (!parts.is_empty()).then(|| {
      Column::new()
        .id("all-projects-invites")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .with_children(parts)
        .into()
    })
  }

  /// The notice (`All projects notice` `p4WXZT`): a 16 `info` mail icon centred beside who invited the person to
  /// what in 13/600 and, 3 under it, the role, the address and the expiry in 12.5 `text_secondary`, with *Decline*
  /// and *Accept* at the end; `info_muted`, radius 10, 12 over and under and 14 at its sides, no stroke. While it needs
  /// two-factor authentication, why is under its second line and the step that fixes it takes *Accept*'s place; after
  /// a refused accept, *Sign in again to accept* follows it with its buttons.
  fn one(&self, ctx: &mut Ctx, invite: &CloudInvite, status: Option<&TwoFactorStatus>) -> Element {
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
    let need = two_factor::need((invite.id, invite.needs_two_factor), status, &self.answers);
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
      )
      .with_children(two_factor::reason(ctx, &id, need));
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
      .with_children((!declining && need != Need::Refused).then(|| self.buttons(ctx, &id, invite, need)));
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Section.role())
      .child(notice)
      .with_children(self.refused(ctx, &id, invite, need))
      .with_children(note(ctx, &id, need))
      .with_children(self.answers.person.reason(ctx, &format!("{id}-accept")))
      .with_children(self.answers.person.reason(ctx, &format!("{id}-sign-in-again")))
      .with_children(self.answers.person.reason(ctx, &format!("{id}-turn-on")));
    if declining {
      column = column.child(invites::decline_question(ctx, &id, invite, &self.answers));
    }
    column.into()
  }

  /// *Sign in again to accept* after a refused accept, with *Sign in again* and *Not now* (`qwDH9` cell 4).
  fn refused(&self, ctx: &Ctx, id: &str, invite: &CloudInvite, need: Need) -> Vec<Element> {
    if need != Need::Refused {
      return Vec::new();
    }
    vec![
      two_factor::refused(ctx, id),
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(two_factor::sign_in_again(
          ctx,
          id,
          two_factor::invite_to(invite),
          &self.answers,
        ))
        .child(two_factor::not_now(ctx, id, invite, &self.answers))
        .into(),
    ]
  }

  /// *Decline*, asked in place, and *Accept*, or the step two-factor authentication needs in its place.
  fn buttons(&self, ctx: &mut Ctx, id: &str, invite: &CloudInvite, need: Need) -> Row {
    let decline = two_factor::decline(ctx, id, invite, &self.answers);
    let row = Row::new()
      .flex_shrink(0.0)
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(decline);
    match need {
      Need::TurnOn => return row.child(two_factor::turn_on(ctx, id, &self.answers)),
      Need::SignInAgain | Need::Refused => {
        return row.child(two_factor::sign_in_again(
          ctx,
          id,
          two_factor::invite_to(invite),
          &self.answers,
        ));
      }
      Need::Accept => {}
    }
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
    row.child(accept)
  }
}

/// What the step two-factor authentication needs does, under the notice.
fn note(ctx: &Ctx, id: &str, need: Need) -> Option<Element> {
  let key = match need {
    Need::Accept => return None,
    Need::TurnOn => "cloud.notice.twoFactor.turnOnNote",
    Need::SignInAgain | Need::Refused => "cloud.notice.twoFactor.signInNote",
  };
  Some(
    Text::new(&ctx.t(key))
      .id(format!("{id}-two-factor-note"))
      .variant(TypographyStyle::Description)
      .width(Dimension::Pct(100.0))
      .into(),
  )
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
