//! A team module's delivery line in *Work and delivery* ([`super::team_work`]), by state (`04f` `Wd4G1`): the delivery
//! the server holds open for the module (`ModuleRead.pending_delivery`), with who asked for it and when, *Authorize*
//! for whom the server lets authorize one that waits for an Admin (`04e` `GujCo`), and *Ask again* for its holder here
//! once an Admin authorized it, which request takes the merge; how the merge and its report went here, from this
//! computer's records; or, with neither, whose it is to ask for.

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  node::Element,
};
use orchester_api::v2::PendingAuthorization;
use orchester_workspace::team_work::DeliveryStatus;

use super::{
  team_work::{ModuleAsk, ShownDelivery},
  team_work_lines::{View, refused_words, with},
};
use crate::{
  screens::settings::orchester::moment,
  team::{
    pages::TeamDelivery,
    strip::{Strip, StripTone},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

/// *Authorize*, which only a person presses: what it authorizes an Admin approves, and no MCP tool authorizes.
pub const AUTHORIZE: &str = "module-team-authorize";
const ID: &str = "module-team-delivery";

/// A commit as the lines name it: its first seven characters.
fn short(commit: &str) -> String {
  commit.chars().take(7).collect()
}

/// What a delivery's line offers.
#[derive(Clone, Copy)]
enum Offers {
  Nothing,
  Authorize,
  /// Ask anew, after one that did not go.
  AskAgain,
  /// Ask again for the delivery an Admin authorized, which takes the merge.
  Merge,
}

/// A line's title and detail keys, its tone, glyph and what it offers.
type Line = (&'static str, &'static str, StripTone, (Icon, PaletteColor), Offers);

/// The module's delivery to its base: the server's open one, unless this computer merges it or merged it already; else
/// the newest one recorded here; else who may ask for it. A request recorded here that waited for an Admin and that the
/// server no longer holds open ended there, so it is not shown.
pub fn delivery(ctx: &Ctx, view: &View<'_>) -> Element {
  let newest = view.recorded.newest.as_ref();
  match (&view.props.module.pending, newest) {
    (Some(pending), Some(local)) if local.binds(pending) && local.merged_here() => recorded(ctx, view, local),
    (Some(pending), _) => pending_line(ctx, view, pending),
    (None, Some(local)) if local.status != DeliveryStatus::AwaitingAuthorization => recorded(ctx, view, local),
    (None, _) => not_requested(ctx, view).into(),
  }
}

/// The server's open delivery `pending`, by how it is authorized and who looks at it.
fn pending_line(ctx: &Ctx, view: &View<'_>, pending: &TeamDelivery) -> Element {
  let module = &view.props.module;
  let base = view
    .recorded
    .base_of(pending)
    .map_or_else(|| short(pending.base_commit.as_str()), |base| base.to_string());
  let named = [
    ("branch", module.branch.clone()),
    ("source", short(pending.source_commit.as_str())),
    ("base", base),
    ("baseCommit", short(pending.base_commit.as_str())),
    ("time", moment(ctx, pending.asked_at)),
    ("holder", pending.asked_by.name.clone()),
  ];
  let holder_here = module.claim.as_ref().is_some_and(|claim| claim.here);
  let (title, detail, tone, glyph, offers) = pending_kind(pending, module.abilities.authorize, holder_here);
  let strip = Strip::new(ID, glyph, ctx.t_args(title, named.clone()), ctx.t_args(detail, named)).tone(tone);
  with(strip, offered(ctx, view, offers)).into()
}

/// The line of an open delivery: one that waits for an Admin is the person's to authorize when `authorizes`, else it is
/// said as the person's own request or someone else's; one an Admin authorized is the holder's to ask for again here.
fn pending_kind(pending: &TeamDelivery, authorizes: bool, holder_here: bool) -> Line {
  let waits = (Icon::GitPullRequest, PaletteColor::Info);
  let authorized = (Icon::ShieldCheck, PaletteColor::Success);
  match pending.authorization {
    PendingAuthorization::AwaitingAdmin if authorizes => (
      "modules.team.work.awaitingAdminTitle",
      if pending.asked_by.you {
        "modules.team.work.awaitingAdminYourDetail"
      } else {
        "modules.team.work.awaitingAdminDetail"
      },
      StripTone::Warning,
      (Icon::GitPullRequest, PaletteColor::Warning),
      Offers::Authorize,
    ),
    PendingAuthorization::AwaitingAdmin => (
      "modules.team.work.awaitingTitle",
      if pending.asked_by.you {
        "modules.team.work.awaitingDetail"
      } else {
        "modules.team.work.awaitingOtherDetail"
      },
      StripTone::Info,
      waits,
      Offers::Nothing,
    ),
    PendingAuthorization::Admin => (
      "modules.team.work.authorizedTitle",
      "modules.team.work.authorizedDetail",
      StripTone::Success,
      authorized,
      if holder_here { Offers::Merge } else { Offers::Nothing },
    ),
    PendingAuthorization::Automatic => (
      "modules.team.work.automaticTitle",
      "modules.team.work.automaticDetail",
      StripTone::Success,
      authorized,
      Offers::Nothing,
    ),
  }
}

/// The line of a delivery by what this computer recorded of it.
fn recorded_kind(delivery: &ShownDelivery) -> Line {
  let plain = (Icon::GitPullRequest, PaletteColor::TextMuted);
  let authorized = (Icon::ShieldCheck, PaletteColor::Success);
  match &delivery.status {
    DeliveryStatus::Requested | DeliveryStatus::AwaitingAuthorization => (
      "modules.team.work.requestedTitle",
      "modules.team.work.requestedDetail",
      StripTone::Plain,
      plain,
      Offers::Nothing,
    ),
    DeliveryStatus::Authorized { .. } => (
      "modules.team.work.authorizedTitle",
      "modules.team.work.authorizedDetail",
      StripTone::Success,
      authorized,
      Offers::Nothing,
    ),
    DeliveryStatus::AuthorizedAutomatically => (
      "modules.team.work.automaticTitle",
      "modules.team.work.automaticDetail",
      StripTone::Success,
      authorized,
      Offers::Nothing,
    ),
    DeliveryStatus::Delivered => (
      "modules.team.work.deliveredTitle",
      "modules.team.work.deliveredDetail",
      StripTone::Plain,
      plain,
      Offers::Nothing,
    ),
    DeliveryStatus::Failed if delivery.merge_failed => (
      "modules.team.work.mergeFailedTitle",
      "modules.team.work.mergeFailedDetail",
      StripTone::Danger,
      plain,
      Offers::AskAgain,
    ),
    DeliveryStatus::Failed => (
      "modules.team.work.withdrawnTitle",
      "modules.team.work.withdrawnDetail",
      StripTone::Warning,
      plain,
      Offers::AskAgain,
    ),
    DeliveryStatus::Refused(_) => (
      "modules.team.work.refusedTitle",
      "modules.team.work.refusedTitle",
      StripTone::Danger,
      plain,
      Offers::AskAgain,
    ),
  }
}

/// A delivery this computer recorded, by its state.
fn recorded(ctx: &Ctx, view: &View<'_>, delivery: &ShownDelivery) -> Element {
  let (title, detail, tone, glyph, offers) = recorded_kind(delivery);
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
  let strip = Strip::new(ID, glyph, ctx.t_args(title, named), detail).tone(tone);
  with(strip, offered(ctx, view, offers)).into()
}

/// The button a line offers.
fn offered(ctx: &Ctx, view: &View<'_>, offers: Offers) -> Option<ActionButton> {
  let again = (
    "module-team-delivery-again",
    "modules.team.work.askAgain",
    Icon::RotateCcw,
  );
  match offers {
    Offers::Nothing => None,
    Offers::Authorize => Some(view.ask_as(
      ctx,
      ButtonKind::Primary,
      (AUTHORIZE, "modules.team.work.authorize", Icon::ShieldCheck),
      ModuleAsk::Authorize,
    )),
    Offers::AskAgain => Some(view.ask(ctx, again, ModuleAsk::Deliver(None))),
    Offers::Merge => Some(view.ask(ctx, again, ModuleAsk::Merge)),
  }
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
