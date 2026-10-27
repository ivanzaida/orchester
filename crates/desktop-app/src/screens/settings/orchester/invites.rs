//! *Invites* (`Invites` `QZ6IM` in `05 · Orchester Cloud`; a waiting invite, declining and accepting in `ZUuMT`): a
//! row per invite — who invited the person to what, when it expires, *Accept* and *Decline* at its end — and under the
//! rows the invites accepted here whose join did not finish ([`super::accepted`]) and what the last answer came to.
//! Accepting connects this computer to the invite's server, trusting exactly the certificate the cloud's registry
//! names, so there is no check code to compare; *Decline* asks in place of the row's actions first. An invite that
//! needs two-factor authentication first says so under the row, with the step that fixes it ([`super::two_factor`]).

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::{
  InviteKind, MemberRole,
  cloud::{CloudInvite, TwoFactorStatus},
};
use orchester_core::time::Timestamp;

pub(super) use super::answers::{Answers, Asked, Outcome};
use super::{
  Invites, Kept, Said, accepted,
  two_factor::{self, Need},
  words::{counted, day, shown_address},
};
use crate::{
  theme::IconSize,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::{Icon, icon},
    notice::NoticeTone,
    setting_notice::{SettingNotice, status_line},
    setting_rows::{SettingGroup, SettingRow, group_note, row_tile},
  },
};

/// The group, as `invites` stand, for an account whose two-factor authentication is `status`.
pub(super) fn waiting(
  ctx: &mut Ctx,
  (invites, kept, status): (&Invites, &Kept, Option<&TwoFactorStatus>),
  answers: &Answers,
) -> Element {
  let mut group = SettingGroup::new("orchester-invites", ctx.t("cloud.invites.group"));
  if invites.waiting() > 0 {
    group = group.meta(counted(
      ctx,
      ("cloud.invites.waiting.one", "cloud.invites.waiting.other"),
      invites.waiting(),
    ));
  }
  let mut parts: Vec<Element> = Vec::new();
  match invites {
    Invites::SignedOut => {}
    Invites::Failed(reason) => parts.push(field_error("orchester-invites-failed", &reason.text(ctx)).into()),
    Invites::Listed(listed) if listed.is_empty() => {
      group = group.row(
        SettingRow::new("orchester-invites-none", ctx.t("cloud.invites.none"))
          .lead(row_tile(icon(Icon::Mail, IconSize::Md, PaletteColor::TextSecondary)))
          .render(ctx),
      );
    }
    Invites::Listed(listed) => {
      for invite in listed.iter() {
        group = group.row(card(ctx, invite, (status, answers)));
      }
    }
  }
  parts.extend(outcome(ctx, answers, false));
  parts.extend(super::ended::found(ctx, answers));
  parts.extend(accepted::kept(ctx, (kept, status), answers));
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .child(group)
    .with_children(parts)
    .child(group_note("orchester-invites-lead", &ctx.t("cloud.invites.lead")))
    .into()
}

/// What the last accept, ask again or decline came to. `waiting`: whether to say that a join waits for approval,
/// where the invite accepted here does not say so itself (the All projects notice).
pub(super) fn outcome(ctx: &mut Ctx, answers: &Answers, waiting: bool) -> Vec<Element> {
  let mut parts = Vec::new();
  if answers.stopped.get() {
    parts.push(
      Callout::new(
        "orchester-invite-stopped",
        CalloutTone::Neutral,
        ctx.t("cloud.invites.stopped.title"),
      )
      .body(ctx.t("cloud.invites.stopped.body"))
      .into(),
    );
  }
  parts.extend(answered(ctx, ("orchester-invite", answers.accept.state()), waiting));
  parts.extend(answered(
    ctx,
    ("orchester-invite-finish", answers.finish.state()),
    waiting,
  ));
  parts.extend(two_factor::failed(ctx, "orchester-invite", answers));
  for (id, state) in [
    ("orchester-invite-decline-failed", answers.decline.state().get()),
    ("orchester-invite-set-aside-failed", answers.set_aside.state().get()),
  ] {
    if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
      parts.push(field_error(id, &reason.text(ctx)).into());
    }
  }
  parts
}

/// What one accept or ask again came to, its parts' ids starting with `id`.
fn answered(ctx: &mut Ctx, (id, shown): (&str, Signal<FutureState<Outcome, Said>>), waiting: bool) -> Vec<Element> {
  let state = shown.get();
  let mut parts = Vec::new();
  if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
    parts.push(field_error(&format!("{id}-accept-failed"), &reason.text(ctx)).into());
  }
  match state.data.clone().filter(|_| state.is_fulfilled()) {
    Some(Outcome::Joined { server, project, .. }) => {
      let body = match project {
        Some(project) => ctx.t_args("cloud.invites.joined.project", [("project", project)]),
        None => ctx.t("cloud.invites.joined.server"),
      };
      parts.push(
        Callout::new(
          format!("{id}-joined"),
          CalloutTone::Success,
          ctx.t_args("cloud.invites.joined.title", [("server", server.as_str())]),
        )
        .body(body)
        .into(),
      );
    }
    Some(Outcome::Waiting { server }) if waiting => parts.push(
      Callout::new(
        format!("{id}-waiting"),
        CalloutTone::Warning,
        ctx.t("cloud.invites.approval.title"),
      )
      .icon(Icon::Hourglass)
      .body(ctx.t_args("cloud.invites.approval.body", [("server", server.as_str())]))
      .into(),
    ),
    Some(Outcome::Refused { server }) => parts.push(super::ended::refused(ctx, id, &server, move || {
      shown.set(FutureState::idle());
    })),
    // The invite's card says what two-factor authentication needs.
    Some(Outcome::Waiting { .. } | Outcome::TwoFactorOff { .. } | Outcome::NeedsCode { .. }) | None => {}
  }
  parts
}

/// What an invite is to, without the role, for the All projects notice whose second line names it.
pub(super) fn notice_title(ctx: &Ctx, invite: &CloudInvite) -> String {
  match &invite.project {
    Some(project) => ctx
      .t_args(
        "cloud.notice.toProject",
        [
          ("inviter", invite.inviter_display_name.as_str()),
          ("project", project.name.as_str()),
          ("server", invite.server.name.as_str()),
        ],
      )
      .to_string(),
    None => title(ctx, invite),
  }
}

/// What an invite is to, in one sentence.
pub(super) fn title(ctx: &Ctx, invite: &CloudInvite) -> String {
  let (inviter, server) = (invite.inviter_display_name.as_str(), invite.server.name.as_str());
  match (&invite.project, invite.role, invite.kind) {
    (Some(project), Some(role), _) => ctx
      .t_args(
        "cloud.invites.toProject",
        [
          ("inviter", inviter),
          ("project", project.name.as_str()),
          ("server", server),
          ("role", &ctx.t(role_key(role))),
        ],
      )
      .to_string(),
    (_, _, InviteKind::Account) => ctx
      .t_args("cloud.invites.toAccount", [("inviter", inviter), ("server", server)])
      .to_string(),
    _ => ctx
      .t_args("cloud.invites.toServer", [("inviter", inviter), ("server", server)])
      .to_string(),
  }
}

const fn role_key(role: MemberRole) -> &'static str {
  match role {
    MemberRole::Owner => "cloud.invites.role.owner",
    MemberRole::Member => "cloud.invites.role.member",
  }
}

/// An invite's row (`Dana Koval invited you to Kontur…` `Ec1re` in `ZUuMT`): a mail tile, what it is to, when it
/// expires (its address in the tooltip), and *Accept* and *Decline*; *Decline the invite?* in place of the actions
/// while asked; *Connecting…* while it is accepted. While it needs two-factor authentication, the step that fixes it
/// is under the row in place of its actions.
fn card(ctx: &mut Ctx, invite: &CloudInvite, (status, answers): (Option<&TwoFactorStatus>, &Answers)) -> Element {
  let id = format!("orchester-invite-{}", invite.id);
  let address = invite
    .project
    .as_ref()
    .map_or_else(|| invite.server.address.clone(), |project| project.address.clone());
  // The cloud lists no sending day, only the expiry.
  let dates = ctx.t_args(
    "cloud.invites.expires",
    [("expires", day(ctx, Timestamp::from_unix_millis(invite.expires_at)))],
  );
  let accepting = answers.connecting() && answers.accepting.get().0 == Some(invite.id);
  let declining = answers.declining.get().0 == Some(invite.id);
  let need = two_factor::need((invite.id, invite.needs_two_factor), status, answers);
  let mut row = SettingRow::new(format!("{id}-entry"), title(ctx, invite))
    .lead(row_tile(icon(Icon::Mail, IconSize::Md, PaletteColor::TextSecondary)))
    .hint(dates.to_string())
    .info(shown_address(&address).to_owned());
  if !accepting && !declining && need == Need::Accept {
    row = row.control(buttons(ctx, &id, invite, answers));
  }
  if accepting {
    row = row.below(connecting(ctx, &id, invite, answers));
  }
  if !accepting && !declining {
    row = row.below_all(two_factor::below(ctx, &id, invite, (need, answers)));
  }
  if declining {
    row = row.below(decline_question(ctx, &id, invite, answers));
  }
  row = row.below_all(answers.person.reason(ctx, &format!("{id}-accept")));
  Column::new()
    .id(id)
    .width(Dimension::Pct(100.0))
    .child(row.render(ctx))
    .into()
}

/// *Connecting to Kontur team…* with *Cancel* under it (`Accepting · the invite row while it joins` `at7XI`).
fn connecting(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> Column {
  let answers = answers.clone();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Start)
    .child(status_line(
      &format!("{id}-connecting"),
      Icon::LoaderCircle,
      &ctx.t_args("cloud.invites.connecting", [("server", invite.server.name.as_str())]),
    ))
    .child(
      ActionButton::new(format!("{id}-cancel"), ButtonKind::Ghost, ctx.t("cloud.invites.cancel"))
        .on_click(move || answers.stop()),
    )
}

/// *Accept* and *Decline* at the row's end.
fn buttons(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> Row {
  let decline = two_factor::decline(ctx, id, invite, answers);
  let busy = answers.connecting();
  let accept_button = ActionButton::new(
    format!("{id}-accept"),
    ButtonKind::Primary,
    ctx.t("cloud.invites.accept"),
  );
  let accept_button = if busy {
    accept_button.disabled()
  } else {
    let (answers, invite, button) = (answers.clone(), invite.clone(), format!("{id}-accept"));
    accept_button.on_click(move || answers.accept(&button, &invite))
  };
  Row::new()
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .child(accept_button)
    .child(decline)
}

/// *Decline the invite to Kontur?* in place of the invite's actions (`Decline` `DqgtJ` in `ZUuMT`).
pub(super) fn decline_question(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> Element {
  let target = invite
    .project
    .as_ref()
    .map_or(invite.server.name.as_str(), |project| project.name.as_str());
  let (answers_now, invite_id) = (answers.clone(), invite.id);
  let pending = answers.decline.state().get().is_pending();
  let button = format!("{id}-decline-confirm");
  let confirm = ActionButton::new(button.clone(), ButtonKind::Danger, ctx.t("cloud.invites.decline"));
  let confirm = if pending {
    confirm.loading()
  } else {
    let button = button.clone();
    confirm.on_click(move || answers_now.decline(&button, invite_id))
  };
  let keep = answers.declining.clone();
  let notice = SettingNotice::new(format!("{id}-decline-question"), NoticeTone::Warning)
    .title(ctx.t_args("cloud.invites.declineQuestion", [("target", target)]))
    .body(ctx.t_args(
      "cloud.invites.declineBody",
      [("inviter", invite.inviter_display_name.as_str())],
    ))
    .action(confirm)
    .action(
      ActionButton::new(format!("{id}-keep"), ButtonKind::Ghost, ctx.t("cloud.invites.keep"))
        .on_click(move || keep.set(Asked(None))),
    );
  match answers.person.reason(ctx, &button) {
    Some(reason) => notice.part(reason).into(),
    None => notice.into(),
  }
}
