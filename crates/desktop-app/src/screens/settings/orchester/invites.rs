//! *Waiting invites* (`Invites` `s25gc8` in `Cu20b`; accepting and declining in `jUMAF`): the invites accepted here
//! whose join did not finish ([`super::accepted`]), then who invited the person to what, with *Decline* and *Accept*.
//! Accepting connects this computer to the invite's server, trusting exactly the certificate the cloud's registry
//! names, so there is no check code to compare; *Decline* asks in place first.

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_api::{InviteKind, MemberRole, cloud::CloudInvite};
use orchester_core::time::Timestamp;

pub(super) use super::answers::{Answers, Asked, Outcome};
use super::{
  Invites, Kept, Said, accepted,
  parts::{paragraph, section as part, section_title},
  words::{counted, day, shown_address},
};
use crate::{
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchesterText},
  ui::{
    avatar::{initials, person_avatar},
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::{Icon, icon},
    inspector,
  },
};

/// The section, as `invites` stand.
pub(super) fn waiting(ctx: &mut Ctx, (invites, kept): (&Invites, &Kept), answers: &Answers) -> Element {
  let meta = (invites.waiting() > 0).then(|| {
    counted(
      ctx,
      ("cloud.invites.waiting.one", "cloud.invites.waiting.other"),
      invites.waiting(),
    )
  });
  let mut parts: Vec<Element> = vec![paragraph("orchester-invites-lead", &ctx.t("cloud.invites.lead")).into()];
  parts.extend(outcome(ctx, answers, false));
  parts.extend(super::ended::found(ctx, answers));
  parts.extend(accepted::kept(ctx, kept, answers));
  match invites {
    Invites::SignedOut => {}
    Invites::Failed(reason) => parts.push(field_error("orchester-invites-failed", &reason.text(ctx)).into()),
    Invites::Listed(listed) if listed.is_empty() => {
      parts.push(inspector::note("orchester-invites-none", Icon::Mail, &ctx.t("cloud.invites.none")).into());
    }
    Invites::Listed(listed) => {
      for invite in listed.iter() {
        parts.push(card(ctx, invite, answers));
      }
    }
  }
  part(
    "orchester-invites",
    section_title(
      "orchester-invites-title",
      &ctx.t("cloud.invites.title"),
      meta.as_deref(),
    ),
    parts,
  )
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
    Some(Outcome::Waiting { .. }) | None => {}
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

/// An invite (`Invite · Billing` `JgJbd`): the inviter's initials, what it is to, its address and expiry, and the
/// answers; *Decline the invite?* in place under it while asked; *Connecting…* while it is accepted.
fn card(ctx: &mut Ctx, invite: &CloudInvite, answers: &Answers) -> Element {
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
  let text = Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Text::new(&title(ctx, invite))
        .id(format!("{id}-title"))
        .variant(OrchesterText::InviteTitle.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Row::new()
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(
          Text::new(shown_address(&address))
            .variant(OrchesterText::Address.role())
            .nowrap(),
        )
        .child(Text::new(&ctx.t("cloud.factSeparator")).variant(ExtraTypography::Note.role()))
        .child(Text::new(&dates).variant(ExtraTypography::Note.role()).nowrap()),
    );
  let accepting = answers.connecting() && answers.accepting.get().0 == Some(invite.id);
  let declining = answers.declining.get().0 == Some(invite.id);
  let top = Row::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .child(person_avatar(
      &format!("{id}-avatar"),
      &initials(&invite.inviter_display_name),
    ))
    .child(text)
    .with_children((!accepting && !declining).then(|| buttons(ctx, &id, invite, answers)));
  let mut card = Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(
      crate::theme::AccountsSpacing::Edge.role(),
      SpacingSize::Md,
    ))
    .spacing(SpacingSize::Md)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(top);
  if accepting {
    card = card.child(connecting(ctx, &id, invite, answers));
  }
  card = card.with_children(answers.person.reason(ctx, &format!("{id}-accept")));
  if !declining {
    return card.into();
  }
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Section.role())
    .child(card)
    .child(decline_question(ctx, &id, invite, answers))
    .into()
}

/// *Connecting to Kontur team…* with *Cancel* under it (`Accepting` in `jUMAF`).
fn connecting(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> Column {
  let answers = answers.clone();
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Start)
    .child(
      Row::new()
        .id(format!("{id}-connecting"))
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(icon(Icon::LoaderCircle, IconSize::Sm, PaletteColor::TextSecondary))
        .child(
          Text::new(&ctx.t_args("cloud.invites.connecting", [("server", invite.server.name.as_str())]))
            .variant(ExtraTypography::Lead.role())
            .flex(1.0),
        ),
    )
    .child(
      ActionButton::new(format!("{id}-cancel"), ButtonKind::Ghost, ctx.t("cloud.invites.cancel"))
        .on_click(move || answers.stop()),
    )
}

fn buttons(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> Row {
  let (declining, invite_id) = (answers.declining.clone(), invite.id);
  let decline = ActionButton::new(
    format!("{id}-decline"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.decline"),
  )
  .on_click(move || declining.set(Asked(Some(invite_id))));
  let busy = answers.connecting();
  let accept_button = ActionButton::new(
    format!("{id}-accept"),
    ButtonKind::Primary,
    ctx.t("cloud.invites.accept"),
  )
  .icon(Icon::Check);
  let accept_button = if busy {
    accept_button.disabled()
  } else {
    let (answers, invite, button) = (answers.clone(), invite.clone(), format!("{id}-accept"));
    accept_button.on_click(move || answers.accept(&button, &invite))
  };
  Row::new()
    .flex_shrink(0.0)
    .spacing(SpacingSize::Sm)
    .child(decline)
    .child(accept_button)
}

/// *Decline the invite to Kontur?* in place (`Decline · confirmed in place` in `jUMAF`).
pub(super) fn decline_question(ctx: &Ctx, id: &str, invite: &CloudInvite, answers: &Answers) -> Element {
  let target = invite
    .project
    .as_ref()
    .map_or(invite.server.name.as_str(), |project| project.name.as_str());
  let (answers_now, invite_id) = (answers.clone(), invite.id);
  let pending = answers.decline.state().get().is_pending();
  let button = format!("{id}-decline-confirm");
  let confirm = ActionButton::new(button.clone(), ButtonKind::Danger, ctx.t("cloud.invites.decline")).icon(Icon::X);
  let confirm = if pending {
    confirm.loading()
  } else {
    let button = button.clone();
    confirm.on_click(move || answers_now.decline(&button, invite_id))
  };
  let keep = answers.declining.clone();
  Column::new()
    .id(format!("{id}-decline-question"))
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Sm)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::BorderStrong))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Text::new(&ctx.t_args("cloud.invites.declineQuestion", [("target", target)]))
        .variant(ExtraTypography::FormSection.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Text::new(&ctx.t_args(
        "cloud.invites.declineBody",
        [("inviter", invite.inviter_display_name.as_str())],
      ))
      .variant(ExtraTypography::Lead.role())
      .width(Dimension::Pct(100.0)),
    )
    .child(
      Row::new().spacing(SpacingSize::Sm).child(confirm).child(
        ActionButton::new(format!("{id}-keep"), ButtonKind::Secondary, ctx.t("cloud.invites.keep"))
          .on_click(move || keep.set(Asked(None))),
      ),
    )
    .with_children(answers.person.reason(ctx, &button))
    .into()
}
