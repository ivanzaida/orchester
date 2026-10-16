//! The invites accepted here whose join did not finish (N1 security review 1, M2), first in *Waiting invites*. One that
//! waits for approval (`Waiting for approval · on the joining computer` `Bl08B` in `jUMAF`) says who approves it, that
//! nothing was uploaded and that it finishes on its own, with *Cancel joining*; while Settings › Orchester shows, it is
//! asked again every two minutes ([`check`]). One whose join failed says so as `Join stopped` (`DheFE`, the nearest
//! frame) does, with *Try again*, which only a person presses, and *Later*.

use std::time::Duration;

use lurq::{
  app::{
    component::DevtoolsInspectable,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_store::AcceptedInvite;
use orchester_workspace::sharing::cloud::Cloud;

use super::{Kept, answers::Answers};
use crate::{
  theme::{AccountsSpacing, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::{Icon, icon},
  },
};

/// How often a join that waits for approval is asked again: each ask takes an identity token, and the cloud gives a
/// computer 60 an hour.
pub(super) const CHECK_EVERY: Duration = Duration::from_secs(120);

/// The kept invites the person did not put off with *Later*.
pub(super) fn kept(ctx: &Ctx, kept: &Kept, answers: &Answers) -> Vec<Element> {
  let later = answers.later.get();
  kept
    .0
    .iter()
    .filter(|invite| !later.0.contains(&invite.invite))
    .map(|invite| {
      if invite.approval.is_some() {
        waiting(ctx, invite, answers)
      } else {
        stopped(ctx, invite, answers)
      }
    })
    .collect()
}

/// *Waiting for approval*: who approves, that nothing was uploaded, and *Cancel joining*.
fn waiting(ctx: &Ctx, invite: &AcceptedInvite, answers: &Answers) -> Element {
  let id = format!("orchester-accepted-{}", invite.invite);
  let server = invite.server_name.as_str();
  let (set_aside, kept) = (answers.set_aside.clone(), invite.invite);
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(AccountsSpacing::Edge.role(), SpacingSize::Md))
    .spacing(ExtraSpacing::Section.role())
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Text::new(&ctx.t("cloud.invites.approval.title"))
        .id(format!("{id}-title"))
        .variant(ExtraTypography::FormSection.role()),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Start)
        .child(icon(Icon::LoaderCircle, IconSize::Lg, PaletteColor::TextSecondary))
        .child(
          Text::new(&ctx.t_args("cloud.invites.approval.ask", [("server", server)]))
            .id(format!("{id}-ask"))
            .variant(ExtraTypography::Lead.role())
            .flex(1.0),
        ),
    )
    .child(
      Text::new(&ctx.t("cloud.invites.approval.later"))
        .variant(ExtraTypography::Lead.role())
        .color(PaletteColor::TextMuted)
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Row::new().child(
        ActionButton::new(
          format!("{id}-cancel"),
          ButtonKind::Ghost,
          ctx.t("cloud.invites.approval.cancel"),
        )
        .on_click(move || set_aside.run(kept)),
      ),
    )
    .into()
}

/// *Joining Kontur team didn't finish*, with *Try again* and *Later*.
fn stopped(ctx: &Ctx, invite: &AcceptedInvite, answers: &Answers) -> Element {
  let id = format!("orchester-accepted-{}", invite.invite);
  let server = invite.server_name.as_str();
  let again = format!("{id}-try-again");
  let try_again = ActionButton::new(
    again.clone(),
    ButtonKind::Primary,
    ctx.t("cloud.invites.joinStopped.tryAgain"),
  )
  .icon(Icon::RefreshCw);
  let try_again = if answers.finish.state().get().is_pending() {
    try_again.loading()
  } else {
    let (answers, kept, again) = (answers.clone(), invite.invite, again.clone());
    try_again.on_click(move || answers.try_again(&again, kept))
  };
  let (later, kept) = (answers.later.clone(), invite.invite);
  let put_off = ActionButton::new(
    format!("{id}-later"),
    ButtonKind::Ghost,
    ctx.t("cloud.invites.joinStopped.later"),
  )
  .on_click(move || later.update(|later| later.0.push(kept)));
  Column::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Section.role())
    .child(
      Callout::new(
        format!("{id}-stopped"),
        CalloutTone::Warning,
        ctx.t_args("cloud.invites.joinStopped.title", [("server", server)]),
      )
      .icon(Icon::WifiOff)
      .body(ctx.t_args("cloud.invites.joinStopped.body", [("server", server)])),
    )
    .child(Row::new().spacing(SpacingSize::Sm).child(try_again).child(put_off))
    .with_children(answers.person.reason(ctx, &again))
    .into()
}

/// What [`check`] came to; nothing to show, the kept invites are read again when a join finishes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Checked;

impl DevtoolsInspectable for Checked {}

/// Asks again, at `tick`, for every invite accepted here that waits for approval: approved, it joins.
pub(super) fn check(ctx: &mut Ctx, tick: u64, cloud: &Cloud) {
  ctx.future((tick, cloud.clone()), |(_, cloud)| async move {
    let kept = cloud
      .accepted_invites()
      .await
      .map_err(|error| tracing::warn!(%error, "the invites accepted here could not be read"))?;
    for invite in kept.into_iter().filter(|invite| invite.approval.is_some()) {
      if let Err(error) = cloud.finish_accept(invite.invite).await {
        tracing::info!(%error, "a join that waits for approval did not finish yet");
      }
    }
    Ok::<_, ()>(Checked)
  });
}
