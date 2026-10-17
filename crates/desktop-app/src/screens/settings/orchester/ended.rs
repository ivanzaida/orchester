//! How a join came out once it no longer waited: approved (`Approved · the joining computer goes on` `P0wvD` in
//! `jUMAF`), with *Continue* to the server, or refused (`Refused` `S5mMY`), with *Back to Settings › Orchester*. The
//! check finds them while Settings › Orchester shows ([`super::accepted::check`]); a refusal also answers an accept
//! or *Try again*. Each stays until the person moves on. The server's approval does not say which computer approved
//! it, so the title names the server where the design names the approving computer.

use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ServerId;

use super::answers::{Answers, Outcome};
use crate::{
  navigation::Navigation,
  routes::{OrchesterView, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
  },
};

/// The approvals and refusals the check found.
pub(super) fn found(ctx: &mut Ctx, answers: &Answers) -> Vec<Element> {
  answers
    .ended
    .get()
    .0
    .iter()
    .filter_map(|(invite, outcome)| {
      let id = format!("orchester-ended-{invite}");
      let (ended, invite) = (answers.ended.clone(), *invite);
      let done = move || ended.update(|ended| ended.0.retain(|(kept, _)| *kept != invite));
      match outcome {
        Outcome::Joined {
          id: joined,
          server,
          project,
          ..
        } => Some(approved(ctx, &id, (*joined, server, project.as_deref()), done)),
        Outcome::Refused { server } => Some(refused(ctx, &id, server, done)),
        Outcome::Waiting { .. } | Outcome::TwoFactorOff { .. } | Outcome::NeedsCode { .. } => None,
      }
    })
    .collect()
}

/// *Approved*: joined, what is next, and *Continue* to the server's page.
fn approved(
  ctx: &mut Ctx,
  id: &str,
  (server, name, project): (ServerId, &str, Option<&str>),
  done: impl Fn() + Send + Sync + 'static,
) -> Element {
  let body = match project {
    Some(project) => ctx.t_args(
      "cloud.invites.approved.project",
      [("server", name), ("project", project)],
    ),
    None => ctx.t_args("cloud.invites.approved.server", [("server", name)]),
  };
  let navigation = Navigation::of(ctx);
  let go_on = ActionButton::new(
    format!("{id}-continue"),
    ButtonKind::Primary,
    ctx.t("cloud.invites.approved.continue"),
  )
  .icon(Icon::ArrowRight)
  .on_click(move || {
    done();
    navigation.go(Route::Orchester(OrchesterView::server(server)));
  });
  outcome(
    id,
    Callout::new(
      format!("{id}-approved"),
      CalloutTone::Success,
      ctx.t_args("cloud.invites.approved.title", [("server", name)]),
    )
    .icon(Icon::CircleCheck)
    .body(body),
    go_on,
  )
}

/// *Kontur team refused this computer*: nothing was uploaded, whom to ask, and *Back to Settings › Orchester*, which
/// puts it away there.
pub(super) fn refused(ctx: &mut Ctx, id: &str, name: &str, done: impl Fn() + Send + Sync + 'static) -> Element {
  let navigation = Navigation::of(ctx);
  let back = ActionButton::new(
    format!("{id}-back"),
    ButtonKind::Secondary,
    ctx.t("cloud.invites.refused.back"),
  )
  .on_click(move || {
    done();
    navigation.go(Route::Orchester(OrchesterView::default()));
  });
  outcome(
    id,
    Callout::new(
      format!("{id}-refused"),
      CalloutTone::Danger,
      ctx.t_args("cloud.invites.refused.title", [("server", name)]),
    )
    .icon(Icon::ShieldX)
    .body(ctx.t_args("cloud.invites.refused.body", [("server", name)])),
    back,
  )
}

/// The callout with its button under it, 14 apart, as the scenes draw them.
fn outcome(id: &str, callout: Callout, button: ActionButton) -> Element {
  Column::new()
    .id(id.to_owned())
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Section.role())
    .child(callout)
    .child(Row::new().spacing(SpacingSize::Sm).child(button))
    .into()
}
