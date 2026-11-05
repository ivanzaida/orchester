//! The lines of a team module's *Work and delivery* ([`super::team_work`]): the module's claim (`H1zbi` in 04, `GV6tR`
//! in 04d), its branch with *Change* for an Admin (`t3oYel`) and its question (`04b` `InYp8`, `04c` `E4Evt`), its
//! delivery to the base by state (`04f` `Wd4G1`), what the last action did not do (`APbft` in 04d, `m8l3MZ` in 04c) and
//! a branch someone else changed first, to reapply or keep (`QcaYU` in 04c).

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};
use orchester_api::v2::AuthorityErrorCode;
use orchester_core::time::Timestamp;
use orchester_workspace::team_work::DeliveryStatus;

use super::team_work::{ModuleAnswer, ModuleAsk, ShownDelivery, WorkControl, WorkProps};
use crate::{
  screens::settings::orchester::moment,
  team::{
    act::{Acted, Holds},
    deliver::Asked,
    pages::TeamClaim,
    strip::{Strip, StripTone},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    dialog::{self, Dialog, Tone},
    field::{FieldInput, field},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// What the lines are drawn from.
pub struct View<'a> {
  pub props: &'a WorkProps,
  pub action: &'a WorkControl,
  pub delivery: Option<&'a ShownDelivery>,
  /// Change branch's question is open, and the name typed in it.
  pub changing: &'a Signal<bool>,
  pub branch: &'a Signal<String>,
  /// The person kept the branch someone else gave the module first.
  pub kept: &'a Signal<bool>,
}

impl View<'_> {
  fn name(&self) -> String {
    self.props.module.module.details().name().as_str().to_owned()
  }

  /// The last answer, once its ask ended.
  fn answer(&self) -> Option<ModuleAnswer> {
    let state = self.action.state().get();
    if state.is_pending() { None } else { state.data }
  }

  fn pending(&self) -> bool {
    self.action.state().get().is_pending()
  }

  /// A strip's button that asks `ask`, under way while an ask is.
  fn ask(&self, ctx: &Ctx, (id, label, glyph): (&str, &str, Icon), ask: ModuleAsk) -> ActionButton {
    let button = ActionButton::new(id.to_owned(), ButtonKind::Secondary, ctx.t(label))
      .icon(glyph)
      .small();
    if self.pending() {
      return button.loading();
    }
    let action = self.action.clone();
    button.on_click(move || action.run(ask.clone()))
  }
}

/// Whole minutes from `from` to `to`, none below zero.
fn minutes(from: Timestamp, to: Timestamp) -> String {
  ((to.unix_millis() - from.unix_millis()) / 60_000).max(0).to_string()
}

/// A commit as the lines name it: its first seven characters.
fn short(commit: &str) -> String {
  commit.chars().take(7).collect()
}

/// The module's claim: who holds it and since when, stale when nothing renewed it, or nobody; with what the server
/// allows: Claim, Release or Take over.
pub fn claim(ctx: &Ctx, view: &View<'_>) -> Element {
  let module = &view.props.module;
  let (name, tasks) = (view.name(), module.tasks.to_string());
  let id = "module-team-claim";
  let abilities = module.abilities;
  let Some(claim) = &module.claim else {
    let strip = Strip::new(
      id,
      (Icon::CircleDashed, PaletteColor::TextMuted),
      ctx.t_args("modules.team.work.nobodyTitle", [("module", name.as_str())]),
      ctx.t_args("modules.team.work.nobodyDetail", [("tasks", tasks.as_str())]),
    );
    return with(strip, abilities.acquire.then(|| claim_button(ctx, view))).into();
  };
  if claim.stale && !claim.holder.you {
    return stale(ctx, view, claim).into();
  }
  let (picked, renewed) = (moment(ctx, claim.acquired), moment(ctx, claim.renewed));
  let detail = ctx.t_args(
    "modules.team.work.claimDetail",
    [("picked", picked.as_str()), ("tasks", &tasks), ("time", &renewed)],
  );
  let title = match (claim.holder.you, claim.here) {
    (true, true) => ctx.t("modules.team.work.hereTitle"),
    (true, false) => ctx.t("modules.team.work.elsewhereTitle"),
    (false, _) => ctx.t_args("modules.team.work.otherTitle", [("holder", claim.holder.name.as_str())]),
  };
  let (glyph, tone) = if claim.stale {
    ((Icon::ClockAlert, PaletteColor::Warning), StripTone::Warning)
  } else {
    ((Icon::UserRoundCheck, PaletteColor::Success), StripTone::Plain)
  };
  let release = (claim.here && abilities.release).then(|| {
    view.ask(
      ctx,
      ("module-team-release", "modules.team.work.release", Icon::LogOut),
      ModuleAsk::Release,
    )
  });
  with(Strip::new(id, glyph, title, detail).tone(tone), release).into()
}

fn claim_button(ctx: &Ctx, view: &View<'_>) -> ActionButton {
  view.ask(
    ctx,
    ("module-team-take", "modules.team.work.claim", Icon::Hand),
    ModuleAsk::Claim,
  )
}

/// Someone else's stale module claim, with Take over while the server allows it.
fn stale(ctx: &Ctx, view: &View<'_>, claim: &TeamClaim) -> Strip {
  let module = &view.props.module;
  let holder = claim.holder.name.as_str();
  let (age, threshold) = (
    minutes(claim.renewed, Timestamp::now()),
    minutes(claim.renewed, claim.stale_at),
  );
  let detail = ctx.t_args(
    "modules.team.work.staleDetail",
    [
      ("age", age.as_str()),
      ("minutes", &threshold),
      ("tasks", &module.tasks.to_string()),
    ],
  );
  let strip = Strip::new(
    "module-team-claim",
    (Icon::ClockAlert, PaletteColor::Warning),
    ctx.t_args("modules.team.work.staleTitle", [("holder", holder)]),
    detail,
  )
  .tone(StripTone::Warning);
  let take_over = module.abilities.take_over.then(|| {
    view.ask(
      ctx,
      ("module-team-take-over", "modules.team.work.takeOver", Icon::Hand),
      ModuleAsk::TakeOver,
    )
  });
  with(strip, take_over)
}

/// The module's branch, with *Change* for an Admin.
pub fn branch(ctx: &Ctx, view: &View<'_>) -> Element {
  let module = &view.props.module;
  let title = ctx.t_args("modules.team.work.branchTitle", [("branch", module.branch.as_str())]);
  let key = if module.abilities.branch {
    "modules.team.work.branchDetail"
  } else {
    "modules.team.work.branchDetailMember"
  };
  let strip = Strip::new(
    "module-team-branch",
    (Icon::GitBranch, PaletteColor::TextSecondary),
    title,
    ctx.t(key),
  );
  let change = module.abilities.branch.then(|| {
    let (open, typed, current) = (view.changing.clone(), view.branch.clone(), module.branch.clone());
    ActionButton::new(
      "module-team-branch-change",
      ButtonKind::Ghost,
      ctx.t("modules.team.work.change"),
    )
    .small()
    .on_click(move || {
      typed.set(current.clone());
      open.set(true);
    })
  });
  with(strip, change).into()
}

/// What a delivery's line offers.
enum Offers {
  Nothing,
  Authorize,
  AskAgain,
}

/// The newest delivery's line by its state: its title's and detail's keys, its tone and what it offers.
fn delivery_line(delivery: &ShownDelivery, admin: bool) -> (&'static str, &'static str, StripTone, Offers) {
  match &delivery.status {
    DeliveryStatus::Requested => (
      "modules.team.work.requestedTitle",
      "modules.team.work.requestedDetail",
      StripTone::Plain,
      Offers::Nothing,
    ),
    DeliveryStatus::AwaitingAuthorization if admin => (
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
  let (title, detail, tone, offers) = delivery_line(delivery, view.props.module.abilities.branch);
  let detail = match &delivery.status {
    DeliveryStatus::Refused(error) => refused_words(ctx, error.code()),
    _ => ctx.t_args(
      detail,
      [
        ("branch", delivery.branch.to_string()),
        ("source", short(delivery.source_commit.as_str())),
        ("base", delivery.base_branch.to_string()),
        ("baseCommit", short(delivery.base_commit.as_str())),
      ],
    ),
  };
  let action = match offers {
    Offers::Nothing => None,
    Offers::Authorize => Some(view.ask(
      ctx,
      ("module-team-authorize", "modules.team.work.authorize", Icon::Check),
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
  let strip = Strip::new("module-team-delivery", glyph, ctx.t(title), detail).tone(tone);
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

/// What the last ask did not do, over the lines, with Try again; a branch someone else changed first is said by
/// [`conflict`] instead, and a branch name refused in the question under its field.
pub fn refusal(ctx: &Ctx, view: &View<'_>) -> Option<Element> {
  let answer = view.answer()?;
  let name = view.name();
  let (ask, body) = match &answer {
    ModuleAnswer::Acted(ModuleAsk::Branch(_), Acted::Changed(_) | Acted::Refused(AuthorityErrorCode::Invalid)) => {
      return None;
    }
    ModuleAnswer::Acted(ask @ ModuleAsk::Branch(_), Acted::Refused(AuthorityErrorCode::TransitionDenied)) => {
      let branch = view.props.module.branch.as_str();
      let body = ctx.t_args(
        "modules.team.work.refused.branchRunning",
        [("module", name.as_str()), ("branch", branch)],
      );
      (ask.clone(), body)
    }
    ModuleAnswer::Acted(ask @ ModuleAsk::TakeOver, Acted::Refused(AuthorityErrorCode::TransitionDenied)) => {
      let holder = view
        .props
        .module
        .claim
        .as_ref()
        .map_or("", |claim| claim.holder.name.as_str());
      let body = ctx.t_args(
        "modules.team.work.refused.takeOverRunning",
        [("module", name.as_str()), ("holder", holder)],
      );
      (ask.clone(), body)
    }
    ModuleAnswer::Acted(_, Acted::Done) | ModuleAnswer::Asked(_, Asked::Recorded) => return None,
    ModuleAnswer::Acted(ask, acted) | ModuleAnswer::Asked(ask, Asked::Acted(acted)) => {
      (ask.clone(), acted_words(ctx, acted)?)
    }
    ModuleAnswer::Asked(ask, asked) => (ask.clone(), asked_words(ctx, asked)),
  };
  let title = match &ask {
    ModuleAsk::Claim => "modules.team.work.refused.claimTitle",
    ModuleAsk::TakeOver => "modules.team.work.refused.takeOverTitle",
    ModuleAsk::Release => "modules.team.work.refused.releaseTitle",
    ModuleAsk::Branch(_) => "modules.team.work.refused.branchTitle",
    ModuleAsk::Deliver(_) | ModuleAsk::Authorize => "modules.team.work.refused.deliverTitle",
  };
  let action = view.action.clone();
  let again = ActionButton::new(
    "module-team-refused-again",
    ButtonKind::Secondary,
    ctx.t("common.tryAgain"),
  )
  .icon(Icon::RefreshCw)
  .on_click(move || action.run(ask.clone()));
  let notice = Notice::new(
    "module-team-refused",
    NoticeTone::Danger,
    ctx.t_args(title, [("module", name.as_str())]),
  )
  .body(body)
  .action(again);
  Some(notice.into())
}

/// The branch someone else gave the module first (`QcaYU` in 04c): what it is now, to reapply the person's or keep it.
pub fn conflict(ctx: &Ctx, view: &View<'_>) -> Option<Element> {
  let ModuleAnswer::Acted(ModuleAsk::Branch(asked), Acted::Changed(holds)) = view.answer()? else {
    return None;
  };
  if view.kept.get() {
    return None;
  }
  let (current, by) = match holds {
    Some(Holds::Branch { branch, by }) => (branch, by),
    _ => (view.props.module.branch.clone(), String::new()),
  };
  let title = if by.is_empty() {
    ctx.t("modules.team.work.conflictTitleUnnamed")
  } else {
    ctx.t_args("modules.team.work.conflictTitle", [("by", by.as_str())])
  };
  let body = ctx.t_args(
    "modules.team.work.conflictDetail",
    [("current", current.as_str()), ("asked", asked.as_str())],
  );
  let (action, kept) = (view.action.clone(), view.kept.clone());
  let reapply = ActionButton::new(
    "module-team-conflict-reapply",
    ButtonKind::Secondary,
    ctx.t_args("modules.team.work.reapply", [("branch", asked.as_str())]),
  )
  .on_click(move || action.run(ModuleAsk::Branch(asked.clone())));
  let keep = ActionButton::new(
    "module-team-conflict-keep",
    ButtonKind::Ghost,
    ctx.t_args("modules.team.work.keep", [("branch", current.as_str())]),
  )
  .on_click(move || kept.set(true));
  let callout = Callout::new("module-team-conflict", CalloutTone::Warning, title)
    .icon(Icon::TriangleAlert)
    .body(body)
    .action(reapply)
    .action(keep);
  Some(callout.into())
}

/// Change branch's question (`Dialog` `r3zwsM` in 04b), while it is open: the new name, with the server's refusal of a
/// taken one under it (`oMd3y` in 04c).
pub fn branch_dialog(ctx: &mut Ctx, view: &View<'_>) -> Option<Element> {
  if !view.changing.get() {
    return None;
  }
  let module = &view.props.module;
  let taken = matches!(
    view.answer(),
    Some(ModuleAnswer::Acted(
      ModuleAsk::Branch(_),
      Acted::Refused(AuthorityErrorCode::Invalid)
    ))
  );
  let mut input = FieldInput::new(
    "module-team-branch-field",
    ctx.t("modules.team.work.branchField"),
    view.branch.clone(),
  )
  .mono();
  if taken {
    input = input.error(ctx.t("modules.team.work.branchTaken"));
  }
  let now = ctx.t_args("modules.team.work.branchNow", [("branch", module.branch.as_str())]);
  let input = input.render(ctx);
  let control = field("module-team-branch-input", None, input, Some(&now));
  let lines = dialog::lines(
    "module-team-branch-lines",
    [
      dialog::line(
        "module-team-branch-merges",
        Icon::GitMerge,
        &ctx.t("modules.team.work.branchMerges"),
      ),
      dialog::line(
        "module-team-branch-locked",
        Icon::Lock,
        &ctx.t("modules.team.work.branchLocked"),
      ),
      dialog::line(
        "module-team-branch-unique",
        Icon::GitBranch,
        &ctx.t("modules.team.work.branchUnique"),
      ),
    ],
  );
  let (action, typed) = (view.action.clone(), view.branch.clone());
  let confirm = ActionButton::new(
    "module-team-branch-confirm",
    ButtonKind::Primary,
    ctx.t("modules.team.work.changeConfirm"),
  );
  let confirm = if view.pending() {
    confirm.loading()
  } else {
    confirm.on_click(move || action.run(ModuleAsk::Branch(typed.get_untracked())))
  };
  let title = ctx.t_args("modules.team.work.branchDialogTitle", [("module", view.name())]);
  let dialog = Dialog::new(
    "module-team-branch-dialog",
    Icon::GitBranch,
    title,
    ctx.t("modules.team.work.cancel"),
  )
  .tone(Tone::Neutral)
  .part(control)
  .part(lines)
  .action(confirm);
  Some(dialog.render(ctx, view.changing))
}

/// `strip`, with `action` at its end when there is one.
fn with(strip: Strip, action: Option<ActionButton>) -> Strip {
  match action {
    Some(action) => strip.action(action),
    None => strip,
  }
}

/// Why an action was not taken; `None` when it was.
fn acted_words(ctx: &Ctx, acted: &Acted) -> Option<Arc<str>> {
  let key = match acted {
    Acted::Done => return None,
    Acted::Refused(code) => return Some(refused_words(ctx, *code)),
    Acted::Changed(_) => "modules.team.work.refused.changed",
    Acted::Unavailable => "modules.team.work.refused.unavailable",
    Acted::SignedOut => "modules.team.work.refused.signedOut",
    Acted::Failed => "modules.team.work.refused.failed",
  };
  Some(ctx.t(key))
}

/// Why a delivery was not asked for.
fn asked_words(ctx: &Ctx, asked: &Asked) -> Arc<str> {
  match asked {
    Asked::WorkRunning => ctx.t("modules.team.work.refused.workRunning"),
    Asked::NoBase => ctx.t("modules.team.work.refused.noBase"),
    Asked::BranchNotFound(branch) => {
      ctx.t_args("modules.team.work.refused.branchMissing", [("branch", branch.as_str())])
    }
    Asked::Recorded | Asked::Acted(_) | Asked::Failed => ctx.t("modules.team.work.refused.failed"),
  }
}

/// A refusal's code as the module's lines say it.
fn refused_words(ctx: &Ctx, code: AuthorityErrorCode) -> Arc<str> {
  let key = match code {
    AuthorityErrorCode::ClaimConflict | AuthorityErrorCode::ModuleClaimConflict => "modules.team.work.refused.conflict",
    AuthorityErrorCode::ClaimLost | AuthorityErrorCode::SessionMismatch => "modules.team.work.refused.lost",
    AuthorityErrorCode::Invalid => "modules.team.work.refused.busy",
    AuthorityErrorCode::TransitionDenied => "modules.team.work.refused.denied",
    AuthorityErrorCode::DecisionRequired => "modules.team.work.refused.decision",
    AuthorityErrorCode::Forbidden => "modules.team.work.refused.forbidden",
    AuthorityErrorCode::DeliveryNotAuthorized => "modules.team.work.refused.notAuthorized",
    AuthorityErrorCode::Unavailable | AuthorityErrorCode::RateLimited | AuthorityErrorCode::Unauthenticated => {
      "modules.team.work.refused.unavailable"
    }
    _ => "modules.team.work.refused.other",
  };
  ctx.t(key)
}
