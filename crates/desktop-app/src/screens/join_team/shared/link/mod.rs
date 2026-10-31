//! *Link a folder* (06 `go2aM`): the folder a person chose or typed is checked as it changes
//! ([`orchester_workspace::sharing::project::folder::check_folder`]), against the project's repository hint. A match
//! goes on with *Link and download tasks*; another `origin` warns and goes on only with *Link anyway* (13b `tDFeg`);
//! not a git repository is an error under the folder row, with nothing to press (13 `sN8Pa`). Linking opens the project
//! on its tasks, where the first download shows, or on the join review when both sides have tasks. Linking changes what
//! this computer sends, so it is a person's: synthetic input is refused, and the page says so.
//!
//! Choice A of `docs/design/startup.md`: Orchester never clones, so the frames' *Clone it here* is the `git clone`
//! command with *Copy* ([`view::clone_panel`]).

mod view;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_api::SharedProject;
use orchester_core::id::IdempotencyKey;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  sharing::{
    ServerEntry, Servers,
    project::{
      ShareError,
      folder::{FolderCheck, FolderLinkError, FolderLinked, check_folder},
      landing::clone_command,
    },
  },
  workspace::Workspace,
};
use parking_lot::Mutex;

use super::{ID, Team, join_crumb, read};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, JoinWay, ProjectPage, Route, TaskList},
  screens::{
    join_team::frame::{Crumb, StartStep, StepFrame, step_frame},
    settings::orchester::{PersonOnly, Said, connect::Shown, sharing_said},
  },
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    state::{load_error, loading},
  },
};

const SUBMIT: &str = "join-team-link-submit";

/// The page's screen for the project `target` names.
pub(in crate::screens::join_team) fn screen(ctx: &mut Ctx, target: JoinTarget) -> Element {
  let key = format!("join-team-link-{}-{}", target.server, target.slug());
  ctx.mount_keyed::<LinkFolder>(&key, Props { target })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Props {
  target: JoinTarget,
}

impl DevtoolsInspectable for Props {}

/// What the folder picker gave; `None` when it was closed without a folder.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Chosen(Option<PathBuf>);

impl DevtoolsInspectable for Chosen {}

/// What the check found for the text in the folder field; nothing for an empty field.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Checked {
  folder: String,
  check: Option<FolderCheck>,
}

impl DevtoolsInspectable for Checked {}

/// What a press of the button at the page's end asks for.
#[derive(Clone)]
struct Ask {
  entry: ServerEntry,
  project: SharedProject,
  target: JoinTarget,
  folder: PathBuf,
  anyway: bool,
  key: IdempotencyKey,
}

/// Where the link leads.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Done(Route);

impl DevtoolsInspectable for Done {}

struct LinkFolder {
  folder: Signal<String>,
  choose: FutureAction<Arc<str>, Chosen, ()>,
  link: FutureAction<Ask, Done, Shown>,
  /// Counts the reads: *Try again*.
  revision: Signal<u64>,
  person: PersonOnly,
  /// The folder of the last link asked for and the upload's key: a retry of the same folder links once.
  key: Arc<Mutex<Option<(String, IdempotencyKey)>>>,
}

impl Component for LinkFolder {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let picker = PathPicker::of(ctx);
    let choose = ctx.future_action(move |title: Arc<str>| {
      let picker = picker.clone();
      async move { Ok::<_, ()>(Chosen(picker.pick(PathKind::Folder, title).await)) }
    });
    let folder = ctx.signal(String::new());
    let typed = folder.clone();
    ctx.watch(&choose.state(), move |state| {
      if state.is_pending() || !state.is_fulfilled() {
        return;
      }
      if let Some(Chosen(Some(path))) = &state.data {
        typed.set(path.display().to_string());
      }
    });
    let services = Services::of(ctx);
    let link = ctx.future_action(move |ask: Ask| {
      let (servers, workspace, programs) = (
        services.servers.clone(),
        services.workspace.clone(),
        services.programs.clone(),
      );
      async move { linked(servers, workspace, programs, ask).await }
    });
    let navigation = Navigation::of(ctx);
    ctx.watch(&link.state(), move |state| {
      if state.is_pending() || !state.is_fulfilled() {
        return;
      }
      if let Some(Done(route)) = &state.data {
        navigation.go(route.clone());
      }
    });
    Self {
      folder,
      choose,
      link,
      revision: ctx.signal(0),
      person: PersonOnly::new(ctx),
      key: Arc::default(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let target = ctx.props::<Props>().target.clone();
    let services = Services::of(ctx);
    let deps = (
      self.revision.get(),
      target.server,
      services.servers.clone(),
      services.cloud.clone(),
    );
    let state = ctx
      .future(deps, |(_, server, servers, cloud)| read(servers, cloud, server))
      .state()
      .get();
    let current = self.folder.get();
    // Asked for in every render, in the same place: lurq keeps a component's futures by the order it asks for them.
    let check = self.check(ctx, (&current, state.data.clone(), &target), services.programs);
    let team = match (state.data, state.error) {
      (Some(team), _) => team,
      (None, Some(reason)) => return self.failed(ctx, &target, &reason),
      (None, None) => {
        let body = loading("join-team-link-reading", &ctx.t("startup.shared.reading")).into();
        return waiting_page(ctx, &target, body);
      }
    };
    let Some(project) = found(&team, &target) else {
      let reason = Shown::Said(Said::new("sharing.join.noProject"));
      return self.failed(ctx, &target, &reason);
    };
    self.page(ctx, (&team, project), (&target, check.as_ref()))
  }
}

/// `target`'s project among `team`'s.
fn found<'a>(team: &'a Team, target: &JoinTarget) -> Option<&'a SharedProject> {
  team
    .projects()
    .iter()
    .map(|listed| &listed.project)
    .find(|project| project.slug == target.slug())
}

/// The page before its project is read, or when it can't be: its slug is all there is to call it.
fn waiting_page(ctx: &mut Ctx, target: &JoinTarget, body: Element) -> Element {
  let frame = StepFrame {
    id: ID,
    ancestors: vec![join_crumb(ctx)],
    crumb: target.slug().to_owned(),
    step: StartStep::Folder,
    title: ctx.t("startup.shared.link.titlePending").to_string(),
    lead: None,
    body,
    note: None,
  };
  step_frame(ctx, frame)
}

impl LinkFolder {
  /// `reason` why the project can't be read, with *Try again*.
  fn failed(&self, ctx: &mut Ctx, target: &JoinTarget, reason: &Shown) -> Element {
    let (revision, text) = (self.revision.clone(), reason.text(ctx));
    let body = load_error(
      ctx,
      "join-team-link-failed",
      (&ctx.t("startup.shared.readFailed"), &text),
      move || revision.update(|count| *count += 1),
    );
    waiting_page(ctx, target, body)
  }

  /// What the check found for `current`, the text in the field; `None` until it is known, so a result for an earlier
  /// text is never shown for this one.
  fn check(
    &self,
    ctx: &mut Ctx,
    (current, team, target): (&str, Option<Team>, &JoinTarget),
    programs: Programs,
  ) -> Option<FolderCheck> {
    let deps = (current.to_owned(), team, target.clone(), programs);
    let checked = ctx
      .future(deps, |(folder, team, target, programs)| async move {
        Ok::<_, ()>(checked(programs, team, &target, folder).await)
      })
      .state()
      .get()
      .data;
    checked
      .filter(|checked| checked.folder == current)
      .and_then(|checked| checked.check)
  }

  /// The page of `project` on `team` (`Inner` `vG08w`).
  fn page(
    &self,
    ctx: &mut Ctx,
    (team, project): (&Team, &SharedProject),
    (target, check): (&JoinTarget, Option<&FolderCheck>),
  ) -> Element {
    let command = project.remote_hint.as_ref().and_then(clone_command);
    let mut parts = vec![
      view::project_line(ctx, project),
      view::existing_option(ctx, (&self.folder, &self.choose), project, (check, command.is_some())),
    ];
    parts.extend(command.as_ref().map(|command| view::clone_panel(ctx, project, command)));
    parts.push(self.actions(ctx, (team, project), (target, check)));
    let name = team.entry().stored.name.clone();
    let lead = if command.is_some() {
      "startup.shared.link.lead"
    } else {
      "startup.shared.link.leadNoClone"
    };
    let frame = StepFrame {
      id: ID,
      ancestors: vec![
        join_crumb(ctx),
        Crumb {
          name: "team",
          label: name,
          route: Route::JoinTeam(JoinWay::Projects(target.server)),
        },
      ],
      crumb: project.name.clone(),
      step: StartStep::Folder,
      title: ctx
        .t_args("startup.shared.link.title", [("project", project.name.as_str())])
        .to_string(),
      lead: Some(ctx.t(lead).to_string()),
      body: Column::new()
        .id("join-team-link")
        .width(Dimension::Pct(100.0))
        .spacing(SpacingSize::Xl)
        .with_children(parts)
        .into(),
      note: None,
    };
    step_frame(ctx, frame)
  }

  /// The button that links and *Back* (`Actions` `WXPxa`), and under them why the last press did nothing.
  fn actions(
    &self,
    ctx: &mut Ctx,
    (team, project): (&Team, &SharedProject),
    (target, check): (&JoinTarget, Option<&FolderCheck>),
  ) -> Element {
    let anyway = matches!(check, Some(FolderCheck::Differs { .. }));
    let label = if anyway {
      "startup.shared.link.anyway"
    } else {
      "startup.shared.link.submit"
    };
    let state = self.link.state().get();
    let button = if state.is_pending() {
      ActionButton::new(SUBMIT, ButtonKind::Primary, ctx.t("startup.shared.link.linking")).loading()
    } else {
      let button = ActionButton::new(SUBMIT, ButtonKind::Primary, ctx.t(label)).icon(Icon::ArrowRight);
      if check.is_some_and(|check| !check.refuses()) {
        let ask = (team.entry().clone(), project.clone(), target.clone(), anyway);
        button.on_click(self.asker(ask))
      } else {
        button.disabled()
      }
    };
    let back = ActionButton::new("join-team-link-back", ButtonKind::Ghost, ctx.t("startup.shared.back"))
      .large()
      .roomy()
      .opens(Navigation::of(ctx).to(Route::JoinTeam(JoinWay::Projects(target.server))));
    let failed = state
      .error
      .as_ref()
      .filter(|_| state.is_rejected())
      .map(|reason| field_error("join-team-link-refused", &reason.text(ctx)));
    Column::new()
      .id("join-team-link-actions")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(
        Row::new()
          .spacing(SpacingSize::Sm)
          .align_items(Alignment::Center)
          .child(button.large())
          .child(back),
      )
      .with_children(self.person.reason(ctx, SUBMIT))
      .with_children(failed)
      .into()
  }

  /// Links the folder in the field to the project, a person's press only.
  fn asker(
    &self,
    (entry, project, target, anyway): (ServerEntry, SharedProject, JoinTarget, bool),
  ) -> impl Fn() + Send + Sync + 'static {
    let (run, person, folder, kept) = (
      self.link.clone(),
      self.person.clone(),
      self.folder.clone(),
      self.key.clone(),
    );
    move || {
      if !person.allows(SUBMIT) {
        return;
      }
      let text = folder.get_untracked();
      let mut kept = kept.lock();
      let key = match kept.as_ref() {
        Some((same, key)) if *same == text => *key,
        _ => {
          let key = IdempotencyKey::generate();
          *kept = Some((text.clone(), key));
          key
        }
      };
      run.run_if_idle(Ask {
        entry: entry.clone(),
        project: project.clone(),
        target: target.clone(),
        folder: typed_path(&text),
        anyway,
        key,
      });
    }
  }
}

/// The folder the field's text names, without the spaces a paste brings around it.
fn typed_path(text: &str) -> PathBuf {
  PathBuf::from(text.trim())
}

/// What `folder` is as a checkout of the project `target` names on `team`. A project database that can't be read says
/// the folder can't be read; the reason is logged.
async fn checked(programs: Programs, team: Option<Team>, target: &JoinTarget, folder: String) -> Checked {
  let project = team.as_ref().and_then(|team| found(team, target)).cloned();
  let (Some(project), false) = (project, folder.trim().is_empty()) else {
    return Checked { folder, check: None };
  };
  let (root, server) = (typed_path(&folder), target.server);
  let check = background::run(move || check_folder(&programs, (server, &project), &root)).await;
  let check = match check {
    Ok(Ok(check)) => check,
    Ok(Err(error)) => {
      tracing::warn!(%error, "a chosen folder's project could not be read");
      FolderCheck::Unreadable
    }
    // The work did not finish; `background::run` logged why.
    Err(_) => FolderCheck::Unreadable,
  };
  Checked {
    folder,
    check: Some(check),
  }
}

/// Links as `ask` says, and where that leads: the project's tasks, where the first download shows, or its join review.
async fn linked(servers: Servers, workspace: Workspace, programs: Programs, ask: Ask) -> Result<Done, Shown> {
  let Ask {
    entry,
    project,
    target,
    folder,
    anyway,
    key,
  } = ask;
  let linked = servers
    .link_folder(&entry, &project, &folder, &programs, (anyway, key))
    .await;
  match linked {
    Ok(FolderLinked::Open(open)) => Ok(Done(Route::Project(
      open.project.id(),
      ProjectPage::Tasks(TaskList::default()),
    ))),
    Ok(FolderLinked::Review(_)) => {
      let open = background::run(move || workspace.open(&folder))
        .await
        .map_err(Shown::Message)?
        .map_err(|error| {
          tracing::warn!(%error, "a folder just linked could not be opened");
          Shown::Said(Said::new("sharing.join.noProject"))
        })?;
      Ok(Done(Route::Project(open.project.id(), ProjectPage::Join(target))))
    }
    Err(FolderLinkError::Refused(check)) => Err(Shown::Message(check.message(&project))),
    Err(FolderLinkError::Share(ShareError::Taken { .. })) => Err(Shown::Said(Said::new("sharing.upload.taken"))),
    Err(FolderLinkError::Share(ShareError::Sharing(error))) => {
      tracing::warn!(%error, "a folder could not be linked to a shared project");
      Err(Shown::Said(sharing_said(&error)))
    }
  }
}
