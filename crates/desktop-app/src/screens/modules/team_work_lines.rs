//! The lines of a team module's *Work and delivery* ([`super::team_work`]): the module's claim (`H1zbi` in 04, `GV6tR`
//! in 04d), its branch with *Change* for an Admin (`t3oYel`) and its question (`04b` `InYp8`, `04c` `E4Evt`), what the
//! last action did not do (`APbft` in 04d, `m8l3MZ` in 04c) and a branch someone else changed first, to reapply or keep
//! (`QcaYU` in 04c); its delivery is in [`super::team_work_delivery`]. Take over and Authorize are a person's: their
//! handlers refuse an MCP client's input, as no MCP tool takes a claim over or authorizes a delivery, and the
//! person-only note says why under the line.

use std::sync::Arc;

use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  core::Signal,
  node::Element,
};
use orchester_api::v2::AuthorityErrorCode;
use orchester_core::time::Timestamp;

use super::team_work::{ModuleAnswer, ModuleAsk, Recorded, WorkControl, WorkProps};
use crate::{
  screens::settings::orchester::{PersonOnly, moment},
  team::{
    act::{Acted, Holds},
    deliver::Asked,
    pages::{TeamClaim, minutes},
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

/// The module's *Take over*, which only a person presses.
pub const TAKE_OVER: &str = "module-team-take-over";
/// *Try again* over the lines, a person's only after a Take over or an Authorize.
pub const AGAIN: &str = "module-team-refused-again";

/// What the lines are drawn from.
pub struct View<'a> {
  pub props: &'a WorkProps,
  pub action: &'a WorkControl,
  /// What this computer recorded of the module's deliveries.
  pub recorded: &'a Recorded,
  /// Change branch's question is open, and the name typed in it.
  pub changing: &'a Signal<bool>,
  pub branch: &'a Signal<String>,
  /// The person kept the branch someone else gave the module first.
  pub kept: &'a Signal<bool>,
  /// Take over and Authorize, which only a person presses.
  pub person: &'a PersonOnly,
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

  /// A strip's secondary button that asks `ask`, under way while an ask is.
  pub(super) fn ask(&self, ctx: &Ctx, button: (&str, &str, Icon), ask: ModuleAsk) -> ActionButton {
    self.ask_as(ctx, ButtonKind::Secondary, button, ask)
  }

  /// A strip's button of `kind` that asks `ask`, under way while an ask is.
  pub(super) fn ask_as(
    &self,
    ctx: &Ctx,
    kind: ButtonKind,
    (id, label, glyph): (&str, &str, Icon),
    ask: ModuleAsk,
  ) -> ActionButton {
    let button = ActionButton::new(id.to_owned(), kind, ctx.t(label)).icon(glyph).small();
    if self.pending() {
      return button.loading();
    }
    let run = self.runs(id);
    button.on_click(move || run(ask.clone()))
  }

  /// What a press of the button `id` does with its ask: Take over and Authorize only on a person's press.
  fn runs(&self, id: &str) -> impl Fn(ModuleAsk) + 'static {
    let (action, person, id) = (self.action.clone(), self.person.clone(), id.to_owned());
    move |ask: ModuleAsk| {
      let person_only = matches!(ask, ModuleAsk::TakeOver | ModuleAsk::Authorize);
      if !person_only || person.allows(&id) {
        action.run(ask);
      }
    }
  }
}

/// Why a press of Take over, Authorize or their Try again did nothing: an MCP client pressed it.
pub fn person_only(ctx: &Ctx, view: &View<'_>, ids: &[&str]) -> Option<Element> {
  ids.iter().find_map(|id| view.person.reason(ctx, id))
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
      (TAKE_OVER, "modules.team.work.takeOver", Icon::Hand),
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
    ModuleAsk::Deliver(_) | ModuleAsk::Authorize | ModuleAsk::Merge => "modules.team.work.refused.deliverTitle",
  };
  let run = view.runs(AGAIN);
  let again = ActionButton::new(AGAIN, ButtonKind::Secondary, ctx.t("common.tryAgain"))
    .icon(Icon::RefreshCw)
    .on_click(move || run(ask.clone()));
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
pub(super) fn with(strip: Strip, action: Option<ActionButton>) -> Strip {
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
    // Only a task edit names a pipeline and only a task's lifecycle is refused so; any other action that met one was
    // refused as invalid.
    Acted::PipelineRefused(_) | Acted::Lifecycle(_) => return Some(refused_words(ctx, AuthorityErrorCode::Invalid)),
    Acted::NeedsVersion(required) => {
      return Some(ctx.t_args(
        "modules.team.work.refused.needsVersion",
        [("version", required.as_str())],
      ));
    }
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
pub(super) fn refused_words(ctx: &Ctx, code: AuthorityErrorCode) -> Arc<str> {
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
