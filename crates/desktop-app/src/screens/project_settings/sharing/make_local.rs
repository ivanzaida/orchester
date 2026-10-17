//! *Make local* (`Make local · asks under the row` and `… · Kontur team can't be reached` in `woioY`; the removed
//! state's *Make local…* in `N9NFio`): the row with *Make local…*, which asks in place under the row what happens to
//! the tasks, their organization, the person's claims, others' claims and the membership, read from the workspace
//! ([`MakeLocalPlan`]). While the server can be reached the claims are released there; otherwise they go stale on
//! their own and the button says *Make local anyway*. Only a person confirms it. What it did is said over the local
//! page until the person dismisses it ([`made`]).

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_core::project::TaskKey;
use orchester_workspace::{
  background,
  sharing::project::{ClaimRelease, MadeLocal, MakeLocalPlan},
};

use super::confirm::{QUESTION_KEY, pair, question};
use crate::{
  project::ShownProject,
  screens::{
    project_settings::page::{self, CONTROL_WIDTH},
    settings::orchester::{PersonOnly, Said, moment, sharing_said},
  },
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    form_section::{Label, Place, side_row},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// What *Make local* did, until the person dismisses it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Made(pub Option<MadeLocal>);

impl DevtoolsInspectable for Made {}

/// The plan as read when the question opened.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Plan(MakeLocalPlan);

impl DevtoolsInspectable for Plan {}

/// *Make local*, which only a person confirms: it releases claims on the server.
const CONFIRM: &str = "sharing-make-local-confirm";

#[derive(Clone)]
pub(super) struct Props {
  pub project: ShownProject,
  /// The server's name and the server project's key, as the question names them.
  pub server: String,
  pub key: String,
  pub made: Signal<Made>,
  pub revision: Signal<u64>,
  /// The question is open; the removed notice opens it too.
  pub asking: Signal<bool>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && self.server == other.server && self.key == other.key
  }
}

impl DevtoolsInspectable for Props {}

pub(super) struct MakeLocal {
  make: FutureAction<(), (), Said>,
  person: PersonOnly,
}

impl Component for MakeLocal {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let servers = Services::of(ctx).servers;
    let root = props.project.open.repo_root.clone();
    let make = ctx.future_action(move |(): ()| {
      let (servers, root, made, revision) = (
        servers.clone(),
        root.clone(),
        props.made.clone(),
        props.revision.clone(),
      );
      async move {
        let done = servers.make_local(&root).await.map_err(|error| sharing_said(&error))?;
        made.set(Made(Some(done)));
        revision.update(|count| *count += 1);
        Ok(())
      }
    });
    Self {
      make,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let local = props.project.open.project.name().as_str().to_owned();
    let asking = props.asking.get();
    let mut control = Column::new()
      .width(Dimension::Px(CONTROL_WIDTH))
      .spacing(SpacingSize::Sm);
    if !asking {
      let open = props.asking.clone();
      control = control.child(
        Row::new().child(
          ActionButton::new(
            "sharing-make-local-button",
            ButtonKind::Secondary,
            ctx.t("sharing.linked.makeLocalButton"),
          )
          .icon(Icon::Unplug)
          .on_click(move || open.set(true)),
        ),
      );
    }
    let row = side_row(
      Label::new("sharing-make-local", ctx.t("sharing.linked.makeLocal")).description(ctx.t_args(
        "sharing.linked.makeLocalDescription",
        [("project", local.as_str()), ("key", props.key.as_str())],
      )),
      control,
      Place::Within,
      true,
    );
    let mut column = Column::new()
      .id("sharing-make-local-part")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(row);
    if asking {
      column = column.child(self.question(ctx, &props));
    }
    column
  }
}

impl MakeLocal {
  /// *Make Kontur local on this computer?* once its plan is read.
  fn question(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let (servers, root) = (Services::of(ctx).servers, props.project.open.repo_root.clone());
    let state = ctx
      .future((root, servers, props.revision.get()), |(root, servers, _)| async move {
        background::run(move || servers.make_local_plan(&root))
          .await
          .map_err(|_| Said::new("cloud.errors.local"))?
          .map(Plan)
          .map_err(|error| sharing_said(&error))
      })
      .state()
      .get();
    match (state.data, state.error) {
      (_, Some(reason)) => field_error("sharing-make-local-unread", &reason.text(ctx)).into(),
      (Some(Plan(plan)), None) => self.asked(ctx, props, &plan),
      (None, None) => page::note("sharing-make-local-reading", &ctx.t("sharing.makeLocal.reading")),
    }
  }

  fn asked(&self, ctx: &Ctx, props: &Props, plan: &MakeLocalPlan) -> Element {
    let local = props.project.open.project.name().as_str().to_owned();
    let names = [
      ("local", local.clone()),
      ("server", props.server.clone()),
      ("key", props.key.clone()),
      ("time", moment(ctx, plan.synced_at)),
    ];
    let body = ctx.t_args(
      match (plan.member, plan.releases, plan.claims.is_some()) {
        (false, ..) => "sharing.makeLocal.bodyRemoved",
        (true, true, _) => "sharing.makeLocal.body",
        (true, false, true) => "sharing.makeLocal.bodyUnreachable",
        (true, false, false) => "sharing.makeLocal.bodySignedOut",
      },
      names.clone(),
    );
    let title = ctx.t_args("sharing.makeLocal.title", [("local", local.as_str())]);
    let pairs = pairs(ctx, plan, &names);
    let pending = self.make.state().get().is_pending();
    let cancel = {
      let asking = props.asking.clone();
      ActionButton::new(
        "sharing-make-local-cancel",
        ButtonKind::Secondary,
        ctx.t("sharing.makeLocal.cancel"),
      )
      .on_click(move || asking.set(false))
    };
    let label = ctx.t(if plan.member && !plan.releases {
      "sharing.makeLocal.confirmAnyway"
    } else {
      "sharing.makeLocal.confirm"
    });
    let confirm = ActionButton::new(CONFIRM, ButtonKind::Danger, label).icon(Icon::Unplug);
    let confirm = if pending {
      confirm.loading()
    } else {
      let (make, person) = (self.make.clone(), self.person.clone());
      confirm.on_click(move || {
        if person.allows(CONFIRM) {
          make.run(());
        }
      })
    };
    let mut column = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(question(
        "sharing-make-local-question",
        (&title, &body),
        pairs,
        vec![cancel.into(), confirm.into()],
      ))
      .with_children(self.person.reason(ctx, CONFIRM));
    let state = self.make.state().get();
    if let Some(reason) = state.error.clone().filter(|_| !state.is_pending()) {
      column = column.child(field_error("sharing-make-local-error", &reason.text(ctx)));
    }
    column.into()
  }
}

/// What happens to each thing (`Pair · Tasks` … `Pair · Membership` in `woioY`).
fn pairs(ctx: &Ctx, plan: &MakeLocalPlan, names: &[(&'static str, String); 4]) -> Vec<Row> {
  let line = |id: &str, key: &str, value: String| pair(id, (&ctx.t(key), QUESTION_KEY), &value);
  let count = ("count", plan.tasks.to_string());
  let tasks = if plan.releases {
    ctx.t_args("sharing.makeLocal.tasks", [count])
  } else {
    let [_, _, _, time] = names.clone();
    ctx.t_args("sharing.makeLocal.tasksAsSynced", [count, time])
  };
  let [local, server, key, _] = names.clone();
  let mut rows = vec![
    line(
      "sharing-make-local-tasks",
      "sharing.makeLocal.tasksKey",
      tasks.to_string(),
    ),
    line(
      "sharing-make-local-organization",
      "sharing.makeLocal.organizationKey",
      ctx
        .t_args("sharing.makeLocal.organization", [key.clone(), local])
        .to_string(),
    ),
    line(
      "sharing-make-local-claims",
      "sharing.makeLocal.claimsKey",
      claims(ctx, plan, server),
    ),
    line(
      "sharing-make-local-others",
      "sharing.makeLocal.othersKey",
      ctx.t("sharing.makeLocal.others").to_string(),
    ),
  ];
  if plan.not_shared > 0 {
    rows.push(line(
      "sharing-make-local-not-shared",
      "sharing.makeLocal.notSharedKey",
      ctx
        .t_args("sharing.makeLocal.notShared", [("count", plan.not_shared.to_string())])
        .to_string(),
    ));
  }
  let membership = if plan.member {
    ctx.t_args("sharing.makeLocal.member", [key])
  } else {
    ctx.t_args("sharing.makeLocal.notMember", [key])
  };
  rows.push(line(
    "sharing-make-local-membership",
    "sharing.makeLocal.membershipKey",
    membership.to_string(),
  ));
  rows
}

/// The person's claims and what becomes of them; unknown once this computer no longer knows who the person is there.
fn claims(ctx: &Ctx, plan: &MakeLocalPlan, server: (&'static str, String)) -> String {
  let Some(claims) = &plan.claims else {
    return ctx.t("sharing.makeLocal.claimsUnknown").to_string();
  };
  if claims.is_empty() {
    return ctx.t("sharing.makeLocal.noClaims").to_string();
  }
  let keys = ("keys", keys(ctx, claims));
  if plan.releases {
    ctx.t_args("sharing.makeLocal.claimsReleased", [keys, server])
  } else {
    ctx.t_args("sharing.makeLocal.claimsStale", [keys])
  }
  .to_string()
}

fn keys(ctx: &Ctx, keys: &[TaskKey]) -> String {
  let separator = ctx.t("sharing.upload.separator");
  keys
    .iter()
    .map(ToString::to_string)
    .collect::<Vec<_>>()
    .join(&separator)
}

/// *Kontur is local on this computer*, with the tasks it kept, the claims it could not release after all and the tasks
/// that returned from *Not shared*, and *Dismiss* at its end: a success notice, as the one after joining (`YdKuq`).
pub(super) fn made(ctx: &Ctx, made: &Signal<Made>, local: &str) -> Option<Element> {
  let done = made.get().0?;
  let mut body = ctx
    .t_args("sharing.makeLocal.made.body", [("count", done.tasks.to_string())])
    .to_string();
  if let ClaimRelease::Stale { tasks } = &done.claims {
    body = ctx
      .t_args(
        "sharing.makeLocal.made.stale",
        [("body", body), ("keys", keys(ctx, tasks))],
      )
      .to_string();
  }
  if !done.returned.is_empty() {
    let separator = ctx.t("sharing.upload.separator");
    let moved: Vec<String> = done
      .returned
      .iter()
      .map(|task| {
        ctx
          .t_args(
            "sharing.makeLocal.made.moved",
            [("before", task.before.to_string()), ("after", task.after.to_string())],
          )
          .to_string()
      })
      .collect();
    body = ctx
      .t_args(
        "sharing.makeLocal.made.returned",
        [("body", body), ("moved", moved.join(&separator))],
      )
      .to_string();
  }
  let dismiss = made.clone();
  Some(
    Notice::new(
      "sharing-made-local",
      NoticeTone::Success,
      ctx.t_args("sharing.makeLocal.made.title", [("local", local)]),
    )
    .body(body)
    .action(
      ActionButton::new(
        "sharing-made-local-dismiss",
        ButtonKind::Secondary,
        ctx.t("sharing.makeLocal.made.dismiss"),
      )
      .on_click(move || dismiss.set(Made::default())),
    )
    .into(),
  )
}
