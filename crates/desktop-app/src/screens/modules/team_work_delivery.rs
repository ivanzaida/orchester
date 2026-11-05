//! A team module's delivery line in *Work and delivery* ([`super::team_work`]): the newest delivery this computer
//! recorded, by state (`04f` `Wd4G1`), with *Authorize* for whom the server lets authorize it and *Ask again* after one
//! that did not go; or, with none recorded, whose it is to ask for.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_workspace::team_work::DeliveryStatus;

use super::{
  team_work::{ModuleAsk, ShownDelivery},
  team_work_lines::{View, refused_words, with},
};
use crate::{
  team::strip::{Strip, StripTone},
  ui::icon::Icon,
};

/// *Authorize*, which only a person presses: what it authorizes an Admin approves, and no MCP tool authorizes.
pub const AUTHORIZE: &str = "module-team-authorize";

/// A commit as the lines name it: its first seven characters.
fn short(commit: &str) -> String {
  commit.chars().take(7).collect()
}

/// What a delivery's line offers.
enum Offers {
  Nothing,
  Authorize,
  AskAgain,
}

/// The newest delivery's line by its state: its title's and detail's keys, its tone and what it offers; `authorizes`
/// when the server lets the person authorize it.
fn delivery_line(delivery: &ShownDelivery, authorizes: bool) -> (&'static str, &'static str, StripTone, Offers) {
  match &delivery.status {
    DeliveryStatus::Requested => (
      "modules.team.work.requestedTitle",
      "modules.team.work.requestedDetail",
      StripTone::Plain,
      Offers::Nothing,
    ),
    DeliveryStatus::AwaitingAuthorization if authorizes => (
      "modules.team.work.awaitingAdminTitle",
      "modules.team.work.awaitingAdminDetail",
      StripTone::Plain,
      Offers::Authorize,
    ),
    DeliveryStatus::AwaitingAuthorization => (
      "modules.team.work.awaitingTitle",
      "modules.team.work.awaitingDetail",
      StripTone::Plain,
      Offers::Nothing,
    ),
    DeliveryStatus::Authorized { .. } => (
      "modules.team.work.authorizedTitle",
      "modules.team.work.authorizedDetail",
      StripTone::Plain,
      Offers::Nothing,
    ),
    DeliveryStatus::AuthorizedAutomatically => (
      "modules.team.work.automaticTitle",
      "modules.team.work.automaticDetail",
      StripTone::Plain,
      Offers::Nothing,
    ),
    DeliveryStatus::Delivered => (
      "modules.team.work.deliveredTitle",
      "modules.team.work.deliveredDetail",
      StripTone::Plain,
      Offers::Nothing,
    ),
    DeliveryStatus::Failed if delivery.merge_failed => (
      "modules.team.work.mergeFailedTitle",
      "modules.team.work.mergeFailedDetail",
      StripTone::Danger,
      Offers::AskAgain,
    ),
    DeliveryStatus::Failed => (
      "modules.team.work.withdrawnTitle",
      "modules.team.work.withdrawnDetail",
      StripTone::Warning,
      Offers::AskAgain,
    ),
    DeliveryStatus::Refused(_) => (
      "modules.team.work.refusedTitle",
      "modules.team.work.refusedTitle",
      StripTone::Danger,
      Offers::AskAgain,
    ),
  }
}

/// The module's delivery to its base, by the newest one this computer recorded, or by who may ask for it.
pub fn delivery(ctx: &Ctx, view: &View<'_>) -> Element {
  let Some(delivery) = view.delivery else {
    return not_requested(ctx, view).into();
  };
  let (title, detail, tone, offers) = delivery_line(delivery, view.props.module.abilities.authorize);
  let named = [
    ("branch", delivery.branch.to_string()),
    ("source", short(delivery.source_commit.as_str())),
    ("base", delivery.base_branch.to_string()),
    ("baseCommit", short(delivery.base_commit.as_str())),
  ];
  let detail = match &delivery.status {
    DeliveryStatus::Refused(error) => refused_words(ctx, error.code()),
    _ => ctx.t_args(detail, named.clone()),
  };
  let title = ctx.t_args(title, named);
  let action = match offers {
    Offers::Nothing => None,
    Offers::Authorize => Some(view.ask(
      ctx,
      (AUTHORIZE, "modules.team.work.authorize", Icon::Check),
      ModuleAsk::Authorize,
    )),
    Offers::AskAgain => Some(view.ask(
      ctx,
      (
        "module-team-delivery-again",
        "modules.team.work.askAgain",
        Icon::RotateCcw,
      ),
      ModuleAsk::Deliver(None),
    )),
  };
  let glyph = (Icon::GitPullRequest, PaletteColor::TextMuted);
  let strip = Strip::new("module-team-delivery", glyph, title, detail).tone(tone);
  with(strip, action).into()
}

/// No delivery recorded here: the holder on this computer asks for it; anyone else reads whose it is to ask.
fn not_requested(ctx: &Ctx, view: &View<'_>) -> Strip {
  let module = &view.props.module;
  let id = "module-team-delivery";
  let glyph = (Icon::GitPullRequest, PaletteColor::TextMuted);
  let holder = module.claim.as_ref().filter(|claim| !claim.stale);
  match holder {
    Some(claim) if claim.here => {
      let ask = view.ask(
        ctx,
        (
          "module-team-delivery-ask",
          "modules.team.work.ask",
          Icon::GitPullRequest,
        ),
        ModuleAsk::Deliver(None),
      );
      Strip::new(
        id,
        glyph,
        ctx.t("modules.team.work.notRequestedTitle"),
        ctx.t("modules.team.work.yoursToAsk"),
      )
      .action(ask)
    }
    Some(claim) if view.props.standing.member() => Strip::new(
      id,
      glyph,
      ctx.t_args(
        "modules.team.work.theirsTitle",
        [("holder", claim.holder.name.as_str())],
      ),
      ctx.t("modules.team.work.theirsDetail"),
    ),
    Some(claim) => Strip::new(
      id,
      glyph,
      ctx.t("modules.team.work.notRequestedTitle"),
      ctx.t_args("modules.team.work.holderAsks", [("holder", claim.holder.name.as_str())]),
    ),
    None => Strip::new(
      id,
      glyph,
      ctx.t("modules.team.work.notRequestedTitle"),
      ctx.t("modules.team.work.nobodyAsks"),
    ),
  }
}
