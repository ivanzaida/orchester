//! Sharing's confirmations (10.12, 10.14 C–H): *Share … on …?* with what it uploads and claims and
//! the discovery file switch; making someone an owner; making yourself a member; leaving, which the
//! last owner cannot; removing a member, whose claims are released; and *Make local*, which says
//! what happens to the person's claims whether or not the server can be reached. A dialog stays open
//! with what went wrong when the server refused.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::ctx::Ctx,
  components::Column,
  node::{Element, dimension::Dimension},
};
use orchester_api::{Member, MemberRole};
use orchester_core::{id::UserId, store::TaskStore};
use orchester_store::ProjectDb;

use super::{Actions, Facts, MemberWork, Members, SharingDialog, SharingTab, labels};
use crate::{
  background,
  screens::project::servers::frame::{Dialog, button, cancel, dialog},
  sharing::{
    ServerEntry,
    feed::ServerFeed,
    project::{MemberChange, SharePlan, state::ProjectSharing},
  },
  ui::{icon::Icon, message_text, property::property, switch::Switch},
  workspace::OpenProject,
};

type Run = Arc<dyn Fn() + Send + Sync>;

/// The keys of the tasks whose claim each person holds, as the cache has them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Held(Vec<(UserId, String)>);

impl lurq::app::component::DevtoolsInspectable for Held {}

impl Held {
  fn of(&self, holder: Option<UserId>) -> Vec<String> {
    self
      .0
      .iter()
      .filter(|(id, _)| Some(*id) == holder)
      .map(|(_, key)| key.clone())
      .collect()
  }
}

impl SharingTab {
  /// The Share confirmation of a local project, while open.
  pub(super) fn local_dialog(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    facts: &Facts,
    actions: &Actions,
  ) -> Option<Element> {
    let SharingDialog::Share { server, key } = self.dialog.get() else {
      return None;
    };
    let list = ServerFeed::of(ctx).list.get();
    let entry = list.entry(server)?.clone();
    let plan = facts.plan.clone()?;
    let identity = plan.identity.clone();
    let project = open.project.name().as_str();
    let key_text = open.project.key().as_str();
    let write = self.write_discovery.get();
    let body = self.share_body(ctx, open, (&plan, &entry), write);
    let state = actions.share.state().get();
    let share = (!state.is_pending()).then(|| {
      let (action, entry) = (actions.share.clone(), entry.clone());
      Arc::new(move || action.run((entry.clone(), identity.clone(), key, write))) as Run
    });
    let problem = state
      .error
      .clone()
      .filter(|_| state.is_rejected())
      .map(|error| message_text(ctx, &error));
    let confirm = button(
      "share-confirm",
      ctx.t("sharing.share.confirm"),
      Some(Icon::CloudUpload),
      share.clone(),
    )
    .primary()
    .render(ctx);
    Some(self.frame(
      ctx,
      Dialog {
        id: "share-dialog",
        title: ctx.t_args(
          "sharing.share.title",
          [("project", project), ("server", entry.stored.name.as_str())],
        ),
        explanation: Some(ctx.t_args(
          "sharing.share.explanation",
          [("key", key_text), ("server", entry.stored.name.as_str())],
        )),
        body: Some(body.into()),
        problem,
        leading: None,
        actions: vec![cancel(ctx, "share-cancel", self.close()), confirm],
        submit: share,
      },
    ))
  }

  /// What sharing uploads, claims and names, and the discovery file switch (10.12).
  fn share_body(
    &self,
    ctx: &Ctx,
    open: &OpenProject,
    (plan, entry): (&SharePlan, &ServerEntry),
    write: bool,
  ) -> Column {
    let project = open.project.name().as_str();
    let tasks = plan.tasks.iter().map(ToString::to_string).collect::<Vec<_>>();
    let uploaded = match (tasks.first(), tasks.last()) {
      (Some(first), Some(last)) => ctx.t_args(
        if tasks.len() == 1 {
          "sharing.share.tasksValue.one"
        } else {
          "sharing.share.tasksValue.other"
        },
        [
          ("count", tasks.len().to_string().as_str()),
          ("first", first.as_str()),
          ("last", last.as_str()),
        ],
      ),
      _ => ctx.t("sharing.share.tasksNone"),
    };
    let claimed = plan.claimed.iter().map(ToString::to_string).collect::<Vec<_>>();
    let claimed = if claimed.is_empty() {
      ctx.t("sharing.share.claimedNone")
    } else {
      ctx.t_args(
        "sharing.share.claimedValue",
        [("keys", &*labels::keys_or(ctx, &claimed, ""))],
      )
    };
    let flip = self.write_discovery.clone();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Md)
      .child(property(&ctx.t("sharing.share.tasks"), &uploaded))
      .child(property(
        &ctx.t("sharing.share.newTasks"),
        &ctx.t_args("sharing.share.newTasksValue", [("key", plan.next.to_string().as_str())]),
      ))
      .child(property(&ctx.t("sharing.share.claimed"), &claimed))
      .child(property(
        &ctx.t("sharing.share.project"),
        &ctx.t_args(
          "sharing.share.projectValue",
          [
            ("project", project),
            ("key", open.project.key().as_str()),
            ("identity", plan.identity.as_str()),
          ],
        ),
      ))
      .child(
        Switch::new("share-discovery", ctx.t("sharing.share.discovery"), write)
          .note(ctx.t_args(
            "sharing.share.discoveryDetail",
            [("address", entry.address.as_str()), ("project", project)],
          ))
          .render(move || flip.update(|on| *on = !*on)),
      )
  }

  /// A shared project's open confirmation.
  pub(super) fn shared_dialog(
    &self,
    ctx: &mut Ctx,
    open: &OpenProject,
    sharing: &ProjectSharing,
    facts: &Facts,
    actions: &Actions,
  ) -> Option<Element> {
    let shown = self.dialog.get();
    if shown == SharingDialog::Closed || matches!(shown, SharingDialog::Share { .. }) {
      return None;
    }
    let members = self.load_members_for_dialog(ctx, sharing);
    let held = self.held(ctx, &open.repo_root);
    let dialog = match shown {
      SharingDialog::MakeOwner(member) => self.make_owner(ctx, sharing, &member, actions),
      SharingDialog::MakeSelfMember => self.make_self_member(ctx, sharing, members.as_ref(), actions),
      SharingDialog::Remove(member) => self.remove(ctx, sharing, &member, (&held, actions)),
      SharingDialog::Leave => self.leave(ctx, sharing, members.as_ref(), actions),
      SharingDialog::MakeLocal => self.make_local(ctx, (open, sharing, facts), (&held, actions)),
      SharingDialog::Closed | SharingDialog::Share { .. } => return None,
    };
    Some(self.frame(ctx, dialog))
  }

  fn frame(&self, ctx: &mut Ctx, shown: Dialog) -> Element {
    dialog(ctx, &self.frame, shown)
  }

  fn close(&self) -> Run {
    let dialog = self.dialog.clone();
    Arc::new(move || dialog.set(SharingDialog::Closed))
  }

  /// Runs `work` unless a change is under way; the action bar's button.
  fn member_run(&self, actions: &Actions, work: MemberWork) -> Option<Run> {
    let action = actions.member.clone();
    (!actions.member.state().get().is_pending()).then(|| Arc::new(move || action.run(work.clone())) as Run)
  }

  fn member_problem(&self, ctx: &Ctx, actions: &Actions) -> Option<Arc<str>> {
    let state = actions.member.state().get();
    state
      .error
      .clone()
      .filter(|_| state.is_rejected())
      .map(|error| message_text(ctx, &error))
  }

  fn make_owner(&self, ctx: &Ctx, sharing: &ProjectSharing, member: &Member, actions: &Actions) -> Dialog {
    let key = sharing.link.key.as_str();
    let first = first_name(&member.user.display_name);
    let run = self.member_run(
      actions,
      MemberWork::Change(MemberChange::Role(member.user.id, MemberRole::Owner)),
    );
    Dialog {
      id: "make-owner-dialog",
      title: ctx.t_args(
        "sharing.roles.makeOwner.title",
        [("name", member.user.display_name.as_str()), ("key", key)],
      ),
      explanation: Some(ctx.t_args(
        "sharing.roles.makeOwner.explanation",
        [("first", first), ("server", sharing.server_name())],
      )),
      body: None,
      problem: self.member_problem(ctx, actions),
      leading: None,
      actions: vec![
        cancel(ctx, "make-owner-cancel", self.close()),
        button(
          "make-owner-confirm",
          ctx.t("sharing.roles.makeOwner.confirm"),
          Some(Icon::Crown),
          run.clone(),
        )
        .primary()
        .render(ctx),
      ],
      submit: run,
    }
  }

  fn make_self_member(
    &self,
    ctx: &Ctx,
    sharing: &ProjectSharing,
    members: Option<&Members>,
    actions: &Actions,
  ) -> Dialog {
    let run = sharing
      .me
      .and_then(|me| self.member_run(actions, MemberWork::Change(MemberChange::Role(me, MemberRole::Member))));
    // Until the members are read, the explanation names no other owner.
    let explanation = match members.map(|members| other_owners(Some(members), sharing.me)) {
      Some(others) => match others.first() {
        Some(owner) => ctx.t_args("sharing.roles.makeSelfMember.explanation", [("owner", owner.as_str())]),
        None => ctx.t("sharing.roles.makeSelfMember.lastOwner"),
      },
      None => ctx.t("sharing.roles.makeSelfMember.explanationAnyOwner"),
    };
    Dialog {
      id: "make-self-member-dialog",
      title: ctx.t_args(
        "sharing.roles.makeSelfMember.title",
        [("key", sharing.link.key.as_str())],
      ),
      explanation: Some(explanation),
      body: None,
      problem: self.member_problem(ctx, actions),
      leading: None,
      actions: vec![
        cancel(ctx, "make-self-member-cancel", self.close()),
        button(
          "make-self-member-confirm",
          ctx.t("sharing.roles.makeSelfMember.confirm"),
          Some(Icon::User),
          run.clone(),
        )
        .primary()
        .render(ctx),
      ],
      submit: run,
    }
  }

  /// *Leave*: the last owner is refused here, with the way out (10.14 E).
  fn leave(&self, ctx: &Ctx, sharing: &ProjectSharing, members: Option<&Members>, actions: &Actions) -> Dialog {
    let key = sharing.link.key.as_str();
    // Until the members are read the server decides: it refuses the last owner (`409 last_owner`).
    let last_owner = sharing.link.role == Some(MemberRole::Owner)
      && members.is_some_and(|_| other_owners(members, sharing.me).is_empty());
    let run = sharing
      .me
      .filter(|_| !last_owner)
      .and_then(|me| self.member_run(actions, MemberWork::Change(MemberChange::Remove(me))));
    let body = last_owner.then(|| {
      let mut body = Column::new()
        .width(Dimension::Pct(100.0))
        .spacing(lurq::app::theme::SpacingSize::Md)
        .child(crate::ui::field::wrapping_field_error(
          &ctx.t_args("sharing.leave.lastOwner", [("key", key)]),
        ));
      if let Some(member) =
        members.and_then(|members| members.0.iter().find(|member| member.role == MemberRole::Member))
      {
        let (dialog, shown) = (self.dialog.clone(), member.clone());
        body = body.child(
          button(
            "leave-make-owner",
            ctx.t_args("sharing.leave.makeOwner", [("name", member.user.display_name.as_str())]),
            Some(Icon::Crown),
            Some(Arc::new(move || dialog.set(SharingDialog::MakeOwner(shown.clone())))),
          )
          .render(ctx),
        );
      }
      body.into()
    });
    Dialog {
      id: "leave-dialog",
      title: ctx.t_args("sharing.leave.title", [("key", key)]),
      explanation: (!last_owner).then(|| ctx.t_args("sharing.leave.explanation", [("key", key)])),
      body,
      problem: self.member_problem(ctx, actions),
      leading: None,
      actions: vec![
        cancel(ctx, "leave-cancel", self.close()),
        button(
          "leave-confirm",
          ctx.t("sharing.members.leave"),
          Some(Icon::LogOut),
          run.clone(),
        )
        .primary()
        .render(ctx),
      ],
      submit: run,
    }
  }

  fn remove(&self, ctx: &Ctx, sharing: &ProjectSharing, member: &Member, (held, actions): (&Held, &Actions)) -> Dialog {
    let key = sharing.link.key.as_str();
    let claims = held.of(Some(member.user.id));
    let claims = if claims.is_empty() {
      ctx.t("sharing.remove.claimsNone")
    } else {
      ctx.t_args(
        "sharing.remove.claimsValue",
        [("keys", &*labels::keys_or(ctx, &claims, ""))],
      )
    };
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(lurq::app::theme::SpacingSize::Md)
      .child(property(&ctx.t("sharing.remove.claims"), &claims))
      .child(property(
        &ctx.t("sharing.remove.runs"),
        &ctx.t("sharing.remove.runsValue"),
      ));
    let run = self.member_run(actions, MemberWork::Change(MemberChange::Remove(member.user.id)));
    Dialog {
      id: "remove-member-dialog",
      title: ctx.t_args(
        "sharing.remove.title",
        [("name", member.user.display_name.as_str()), ("key", key)],
      ),
      explanation: Some(ctx.t_args(
        "sharing.remove.explanation",
        [("first", first_name(&member.user.display_name)), ("key", key)],
      )),
      body: Some(body.into()),
      problem: self.member_problem(ctx, actions),
      leading: None,
      actions: vec![
        cancel(ctx, "remove-member-cancel", self.close()),
        button(
          "remove-member-confirm",
          ctx.t("sharing.members.remove"),
          Some(Icon::UserMinus),
          run.clone(),
        )
        .primary()
        .render(ctx),
      ],
      submit: run,
    }
  }

  /// *Make local* (10.14 G), and while the server cannot be reached (H).
  fn make_local(
    &self,
    ctx: &Ctx,
    (open, sharing, facts): (&OpenProject, &ProjectSharing, &Facts),
    (held, actions): (&Held, &Actions),
  ) -> Dialog {
    let reachable = !sharing.access().read_only();
    let (project, key, server) = (
      open.project.name().as_str(),
      sharing.link.key.as_str(),
      sharing.server_name(),
    );
    let synced = labels::time(sharing.link.last_synced_at);
    let body = make_local_body(ctx, (open, sharing, facts), &held.of(sharing.me), reachable);
    let action = actions.make_local.clone();
    let state = actions.make_local.state().get();
    let run = (!state.is_pending()).then(|| Arc::new(move || action.run(())) as Run);
    let explanation = if reachable {
      ctx.t_args(
        "sharing.makeLocal.explanation",
        [("project", project), ("server", server), ("key", key)],
      )
    } else {
      ctx.t_args(
        "sharing.makeLocal.explanationOffline",
        [("server", server), ("time", synced.as_str())],
      )
    };
    let confirm = if reachable {
      "sharing.makeLocal.confirm"
    } else {
      "sharing.makeLocal.confirmAnyway"
    };
    Dialog {
      id: "make-local-dialog",
      title: ctx.t_args("sharing.makeLocal.title", [("project", project)]),
      explanation: Some(explanation),
      body: Some(body.into()),
      problem: state
        .error
        .clone()
        .filter(|_| state.is_rejected())
        .map(|error| message_text(ctx, &error)),
      leading: None,
      actions: vec![
        cancel(ctx, "make-local-cancel", self.close()),
        button(
          "make-local-confirm",
          ctx.t(confirm),
          Some(Icon::HardDriveDownload),
          run.clone(),
        )
        .primary()
        .render(ctx),
      ],
      submit: run,
    }
  }

  /// Who holds which task, read from the cache when a dialog that names claims opens.
  fn held(&self, ctx: &mut Ctx, root: &std::path::Path) -> Held {
    let load = ctx.future(
      (root.to_path_buf(), self.revision.get()),
      |(root, _): (PathBuf, u64)| async move {
        background::run(move || read_held(&root))
          .await?
          .map_err(|error| crate::messages::store_error(&error))
      },
    );
    load.state().get().data.unwrap_or_default()
  }

  fn load_members_for_dialog(&self, ctx: &mut Ctx, sharing: &ProjectSharing) -> Option<Members> {
    self.load_members(ctx, sharing).and_then(Result::ok)
  }
}

fn read_held(root: &std::path::Path) -> Result<Held, orchester_store::StoreError> {
  let db = ProjectDb::open(root)?;
  let mut held = Vec::new();
  for task in db.tasks()? {
    if let Some(claim) = db.shared_task_info(task.id())?.and_then(|info| info.claim) {
      held.push((claim.holder.id, task.key().to_string()));
    }
  }
  Ok(Held(held))
}

/// What Make local does to the tasks, their organization, the claims and the membership.
fn make_local_body(
  ctx: &Ctx,
  (open, sharing, facts): (&OpenProject, &ProjectSharing, &Facts),
  mine: &[String],
  reachable: bool,
) -> Column {
  let (project, key) = (open.project.name().as_str(), sharing.link.key.as_str());
  let synced = labels::time(sharing.link.last_synced_at);
  let count = facts.tasks.to_string();
  let tasks = if reachable {
    ctx.t_args("sharing.makeLocal.tasksValue", [("count", count.as_str())])
  } else {
    ctx.t_args(
      "sharing.makeLocal.tasksOffline",
      [("count", count.as_str()), ("time", synced.as_str())],
    )
  };
  let rows = [
    ("sharing.makeLocal.tasks", tasks),
    (
      "sharing.makeLocal.organization",
      ctx.t_args(
        "sharing.makeLocal.organizationValue",
        [("key", key), ("project", project)],
      ),
    ),
    (
      "sharing.makeLocal.claims",
      claims_text(ctx, mine, reachable, sharing.server_name()),
    ),
    ("sharing.makeLocal.others", ctx.t("sharing.makeLocal.othersValue")),
    (
      "sharing.makeLocal.membership",
      ctx.t_args("sharing.makeLocal.membershipValue", [("key", key)]),
    ),
  ];
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(lurq::app::theme::SpacingSize::Md)
    .with_children(rows.iter().map(|(label, value)| property(&ctx.t(label), value)))
}

fn claims_text(ctx: &Ctx, mine: &[String], reachable: bool, server: &str) -> Arc<str> {
  if mine.is_empty() {
    return ctx.t("sharing.makeLocal.claimsNone");
  }
  let keys = labels::keys_or(ctx, mine, "");
  if reachable {
    ctx.t_args(
      "sharing.makeLocal.claimsReleased",
      [("keys", &*keys), ("server", server)],
    )
  } else {
    ctx.t_args("sharing.makeLocal.claimsStale", [("keys", &*keys)])
  }
}

/// The owners other than the person, by name.
fn other_owners(members: Option<&Members>, me: Option<UserId>) -> Vec<String> {
  members
    .map(|members| {
      members
        .0
        .iter()
        .filter(|member| member.role == MemberRole::Owner && Some(member.user.id) != me)
        .map(|member| member.user.display_name.clone())
        .collect()
    })
    .unwrap_or_default()
}

/// The first word of a display name, as the dialogs name a person a second time.
fn first_name(name: &str) -> &str {
  name.split_whitespace().next().unwrap_or(name)
}
