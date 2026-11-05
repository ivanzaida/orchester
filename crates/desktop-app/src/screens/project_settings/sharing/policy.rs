//! A team project's *Team policy* in Sharing (`07 · Sharing · admin` `MnVch`, `07b · Sharing · member` `FSeEz`,
//! `docs/design/team-authority.md` §7): *Automatic merge*, a switch for whom the server's capabilities let change it
//! and its value with a lock for a member, the Admins named; and *Stale claim after*, read-only, the server's claim
//! staleness threshold as the workspace's metadata gives it to every reader. A change of the automatic merge goes
//! through the dispatcher on the policy's revision; one someone else made first shows what it is now, to change again
//! or keep (`Sharing · Automatic merge` in `05b` `H7MBjm`). While the server refuses this version of Orchester, the
//! page says so first, with the version it needs (`07d` `BDJKM`).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::PaletteColor,
  },
  components::Row,
  core::Signal,
  layout::Alignment,
  node::Element,
};
use orchester_core::sharing::ClaimStalenessThreshold;
use orchester_workspace::actions::{Action, PolicyAction};

use super::super::kit::{Group, SettingRow, value};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{Route, UpdatesShow},
  screens::settings::orchester::PersonOnly,
  services::{Services, updates},
  team::{
    TeamFailure, TeamSource,
    act::{Acted, Acting, Holds},
    lock,
    queries::{TeamPolicy, team_admins, team_policy},
    view,
  },
  theme::{ExtraSpacing, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::{Icon, icon},
    notice::{Notice, NoticeTone},
    toggle::{Toggle, ToggleKind},
  },
};

/// What Sharing reads of a team project for its policy.
#[derive(Clone, PartialEq)]
pub(super) struct TeamPart {
  pub source: Option<TeamSource>,
  pub failure: Option<TeamFailure>,
  pub policy: Option<TeamPolicy>,
  /// Who changes the policy, named.
  pub admins: Option<String>,
}

impl DevtoolsInspectable for TeamPart {}

/// The project's team part, read from its server.
pub(super) fn read(ctx: &mut Ctx, project: &ShownProject) -> TeamPart {
  view::provide_client(ctx);
  let source = match view::source(ctx, project) {
    Ok(source) => source,
    Err(failure) => {
      return TeamPart {
        source: None,
        failure: Some(failure),
        policy: None,
        admins: None,
      };
    }
  };
  let Some(source) = source else {
    return TeamPart {
      source: None,
      failure: None,
      policy: None,
      admins: None,
    };
  };
  let policy = ctx.query(team_policy(source.clone()));
  let admins = ctx.query(team_admins(source.clone())).data();
  TeamPart {
    failure: policy.error().map(|failure| (*failure).clone()),
    policy: policy.data().map(|policy| *policy),
    admins: admins.map(|admins| lock::admin_names(ctx, &admins)),
    source: Some(source),
  }
}

/// The server refuses this version of Orchester (`Notice · This team server needs Orchester 0.4.0` in 07d): the
/// version it needs, and this one it refuses, on the server `server` for the team project `project`.
pub(super) fn refusal(ctx: &mut Ctx, part: &TeamPart, (server, project): (&str, &str)) -> Option<Element> {
  let Some(TeamFailure::Unsupported { required }) = &part.failure else {
    return None;
  };
  let navigation = Navigation::of(ctx);
  let check = ActionButton::new(
    "sharing-version-check",
    ButtonKind::Secondary,
    ctx.t("sharing.policy.checkUpdates"),
  )
  .icon(Icon::RefreshCw)
  .opens(navigation.to(Route::Updates(UpdatesShow::Section)));
  let current = updates::version();
  let notice = Notice::new(
    "sharing-version-refused",
    NoticeTone::Danger,
    ctx.t_args("sharing.policy.versionTitle", [("version", required.as_str())]),
  )
  .body(ctx.t_args(
    "sharing.policy.versionBody",
    [("server", server), ("current", current.as_str()), ("project", project)],
  ))
  .action(check);
  Some(notice.into())
}

/// The threshold as *Stale claim after* says it: whole minutes, or seconds when it is no whole number of minutes.
fn threshold_words(ctx: &Ctx, threshold: ClaimStalenessThreshold) -> std::sync::Arc<str> {
  let seconds = threshold.seconds();
  if seconds.is_multiple_of(60) {
    return ctx.t_args("sharing.policy.minutes", [("minutes", (seconds / 60).to_string())]);
  }
  ctx.t_args("sharing.policy.seconds", [("seconds", seconds.to_string())])
}

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub part: TeamPart,
}

impl DevtoolsInspectable for Props {}

/// The switch of the automatic merge, which only a person turns.
const SWITCH: &str = "sharing-policy-merge-switch";
/// *Turn on again* / *Turn off again* under a conflict, which only a person presses.
const AGAIN: &str = "sharing-policy-conflict-again";

/// *Team policy*, once the policy is read.
pub(super) struct Policy {
  /// The person kept the automatic merge someone else set first; reset by the next change.
  kept: Signal<bool>,
  /// Turning the automatic merge lets later deliveries go without an Admin, so it is a person's (`Guarded::Sharing`);
  /// no MCP tool changes the policy either.
  person: PersonOnly,
}

impl Component for Policy {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      kept: ctx.signal(false),
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let part = ctx.props::<Props>().part.clone();
    let action = self.action(ctx, part.source.clone());
    let Some(policy) = part.policy else {
      return Group::new("project-settings-sharing-policy", ctx.t("sharing.policy.title"));
    };
    let merge = self.merge_row(ctx, (&part, policy), &action);
    let threshold = threshold_words(ctx, policy.stale_after);
    let stale = SettingRow::new("sharing-policy-stale", ctx.t("sharing.policy.stale"))
      .hint(ctx.t("sharing.policy.staleHint"))
      .control(locked("sharing-policy-stale-value", &threshold))
      .render(ctx);
    Group::new("project-settings-sharing-policy", ctx.t("sharing.policy.title"))
      .row(merge)
      .row(stale)
  }
}

impl Policy {
  /// The change of the automatic merge, on the policy as the page shows it; made on every render.
  fn action(&self, ctx: &mut Ctx, source: Option<TeamSource>) -> FutureAction<PolicyAction, Acted, ()> {
    let acting = Acting::of(&Services::of(ctx));
    let kept = self.kept.clone();
    ctx.future_action(move |change: PolicyAction| {
      let (acting, source) = (acting.clone(), source.clone());
      kept.set(false);
      async move {
        let Some(source) = source else {
          return Ok(Acted::Failed);
        };
        Ok(acting.act(&source, Action::Policy(change)).await.0)
      }
    })
  }

  /// *Automatic merge*: a switch for whom the server lets change it, its value with a lock for a member; what someone
  /// else set first under it.
  fn merge_row(
    &self,
    ctx: &mut Ctx,
    (part, policy): (&TeamPart, TeamPolicy),
    action: &FutureAction<PolicyAction, Acted, ()>,
  ) -> Element {
    let on = policy.policy.automatic_merge;
    let row = SettingRow::new("sharing-policy-merge", ctx.t("sharing.policy.merge"));
    if !policy.changes {
      let key = if on {
        "sharing.policy.mergeMemberOn"
      } else {
        "sharing.policy.mergeMemberOff"
      };
      let admins = part.admins.clone().unwrap_or_default();
      let words = ctx.t(if on { "sharing.policy.on" } else { "sharing.policy.off" });
      return row
        .hint(ctx.t_args(key, [("admins", admins)]))
        .control(locked("sharing-policy-merge-value", &words))
        .render(ctx);
    }
    let state = action.state().get();
    let read = policy.policy.revision;
    let (change, person) = (action.clone(), self.person.clone());
    let switch = Toggle::new(SWITCH, ToggleKind::Switch, on, ctx.t("sharing.policy.merge"))
      .hide_label()
      .on_change(move |wanted| {
        // One change at a time: a second on the same revision would come back as someone else's.
        if person.allows(SWITCH) {
          change.run_if_idle(PolicyAction {
            read,
            automatic_merge: wanted,
          });
        }
      });
    let conflict = (!state.is_pending() && !self.kept.get())
      .then(|| state.data.clone())
      .flatten()
      .and_then(|acted| conflict(ctx, (action, &self.kept, &self.person), policy, &acted));
    let refused = [SWITCH, AGAIN].into_iter().find_map(|id| self.person.reason(ctx, id));
    row
      .hint(ctx.t("sharing.policy.mergeHint"))
      .control(switch)
      .under_all(conflict)
      .under_all(refused)
      .render(ctx)
  }
}

/// The automatic merge someone else set first (`o3iheS` in 05b): what it is now, to change again or keep. A change
/// again names the revision the server answered the conflict with, so it goes through before the page reads the policy
/// again.
fn conflict(
  ctx: &Ctx,
  (action, kept, person): (&FutureAction<PolicyAction, Acted, ()>, &Signal<bool>, &PersonOnly),
  policy: TeamPolicy,
  acted: &Acted,
) -> Option<Element> {
  let Acted::Changed(holds) = acted else {
    return None;
  };
  let current = match holds {
    Some(Holds::AutomaticMerge(current)) => *current,
    _ => policy.policy,
  };
  let now = current.automatic_merge;
  let (body, again, keep) = if now {
    (
      "sharing.policy.conflictOn",
      "sharing.policy.turnOffAgain",
      "sharing.policy.keepOn",
    )
  } else {
    (
      "sharing.policy.conflictOff",
      "sharing.policy.turnOnAgain",
      "sharing.policy.keepOff",
    )
  };
  let (change, read, person) = (action.clone(), current.revision, person.clone());
  let again = ActionButton::new(AGAIN, ButtonKind::Secondary, ctx.t(again)).on_click(move || {
    if person.allows(AGAIN) {
      change.run_if_idle(PolicyAction {
        read,
        automatic_merge: !now,
      });
    }
  });
  let kept = kept.clone();
  let keep =
    ActionButton::new("sharing-policy-conflict-keep", ButtonKind::Ghost, ctx.t(keep)).on_click(move || kept.set(true));
  Some(
    Callout::new(
      "sharing-policy-conflict",
      CalloutTone::Warning,
      ctx.t("sharing.policy.conflictTitle"),
    )
    .icon(Icon::TriangleAlert)
    .body(ctx.t(body))
    .action(again)
    .action(keep)
    .into(),
  )
}

/// A read-only value after a lock (`uZoMY` in 07).
fn locked(id: &str, words: &str) -> Row {
  Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(icon(Icon::Lock, IconSize::Base, PaletteColor::TextMuted))
    .child(value(id, words))
}
