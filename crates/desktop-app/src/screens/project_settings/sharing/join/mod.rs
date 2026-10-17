//! Joining a project on a team server (`docs/architecture/server.md`, "Two people share one repository"), at
//! `…/settings/sharing/join?server=…&project=…`: the project is read from its server with what linking this checkout
//! would do. A checkout with tasks linked to a project with tasks is reviewed first (`AiCwO`, [`review`]); one of them
//! without tasks links on the first link's page (`KSnOW`, [`super::upload`]). Both pages lead back to Sharing, which
//! after a join says what it did and keeps the keys before joining ([`super::linked`]). Not a member, or no project
//! there, it says so and sends nothing.

mod plan;
mod review;
mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::IdempotencyKey;
use orchester_exec::programs::Programs;
use orchester_workspace::{
  background,
  sharing::{
    RemoteMatch, ServerEntry, Servers, SharingError,
    project::{JoinReview, first_link::LinkKind, link::checkout_origin},
  },
};

use super::upload::{LinkPage, Upload, link_page};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{JoinTarget, ProjectPage, ProjectSettingsTab, Route},
  screens::{
    project_settings::page,
    settings::orchester::{Said, sharing_said},
  },
  services::Services,
  ui::state::{load_error, loading},
};

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  target: JoinTarget,
}

impl DevtoolsInspectable for Props {}

/// What joining the target would do for this checkout.
#[derive(Clone)]
enum Resolved {
  /// Both have tasks: the review.
  Review(Arc<Reviewed>),
  /// One of them has none: the first link's page.
  Link(Arc<Upload>),
}

impl PartialEq for Resolved {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Review(one), Self::Review(two)) => Arc::ptr_eq(one, two),
      (Self::Link(one), Self::Link(two)) => Arc::ptr_eq(one, two),
      _ => false,
    }
  }
}

impl DevtoolsInspectable for Resolved {}

/// A review with the server it is on and how the checkout's `origin` compares with the project's hint.
pub(super) struct Reviewed {
  pub entry: ServerEntry,
  pub review: JoinReview,
  pub remote: RemoteMatch,
}

/// The join of the project `target` names, for `project`.
pub fn screen(ctx: &mut Ctx, project: &ShownProject, target: &JoinTarget) -> Element {
  ctx.mount_keyed::<JoinPage>(
    &format!(
      "project-settings-join-{}-{}-{}",
      project.open.project.id(),
      target.server,
      target.slug()
    ),
    Props {
      project: project.clone(),
      target: target.clone(),
    },
  )
}

struct JoinPage {
  /// Counts *Try again* after the project could not be read.
  retries: Signal<u64>,
  /// The first link's page, once the project resolved to one; put away, the window returns to Sharing.
  page: Signal<LinkPage>,
  /// The first link's page was opened: once it is put away, it is not opened again.
  opened: parking_lot::Mutex<bool>,
}

impl Component for JoinPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let page = ctx.signal(LinkPage::default());
    let props = ctx.props::<Props>().clone();
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    // The link's page put itself away: cancelled or linked.
    ctx.watch(&page, move |page: &LinkPage| {
      if page.0.is_none() {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Settings(ProjectSettingsTab::Sharing),
        ));
      }
    });
    Self {
      retries: ctx.signal(0),
      page,
      opened: parking_lot::Mutex::new(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let deps = (
      self.retries.get(),
      props.target.clone(),
      props.project.open.repo_root.clone(),
      (services.servers.clone(), services.programs),
    );
    let state = ctx
      .future(deps, |(_, target, root, (servers, programs))| async move {
        resolve(&servers, &programs, root, &target).await
      })
      .state()
      .get();
    match (state.data, state.error) {
      (Some(Resolved::Review(reviewed)), _) => review::page(ctx, &props.project, reviewed),
      (Some(Resolved::Link(upload)), _) => self.link(ctx, &props.project, upload),
      (None, Some(reason)) => self.failed(ctx, &reason),
      (None, None) => page::screen(
        loading("sharing-join-loading", &ctx.t("sharing.join.loading")).into(),
        page::scroll(lurq::components::Column::new()),
      ),
    }
  }
}

impl JoinPage {
  /// The first link's page for `upload`, opened once.
  fn link(&self, ctx: &mut Ctx, project: &ShownProject, upload: Arc<Upload>) -> Element {
    let mut opened = self.opened.lock();
    if !*opened {
      *opened = true;
      self.page.set(LinkPage(Some(upload)));
    }
    match self.page.get().0 {
      Some(upload) => link_page(ctx, project, upload, &self.page),
      // Put away: the watch above returns to Sharing.
      None => page::screen(
        loading("sharing-join-loading", &ctx.t("sharing.join.loading")).into(),
        page::scroll(lurq::components::Column::new()),
      ),
    }
  }

  /// Why the project can't be joined from here, with *Try again*.
  fn failed(&self, ctx: &mut Ctx, reason: &Said) -> Element {
    let retries = self.retries.clone();
    let text = reason.text(ctx);
    page::screen(
      load_error(
        ctx,
        "sharing-join-failed",
        (&ctx.t("sharing.join.loadFailed"), &text),
        move || retries.update(|count| *count += 1),
      ),
      page::scroll(lurq::components::Column::new()),
    )
  }
}

/// What joining `target` would do for the checkout at `root`.
async fn resolve(servers: &Servers, programs: &Programs, root: PathBuf, target: &JoinTarget) -> Result<Resolved, Said> {
  let all = servers.clone();
  let entry = background::run(move || all.entries())
    .await
    .map_err(|_| Said::new("cloud.errors.local"))?
    .map_err(|error| sharing_said(&error))?
    .into_iter()
    .find(|entry| entry.stored.id == target.server && entry.in_use())
    .ok_or_else(|| Said::new("sharing.join.noServer"))?;
  let project = servers
    .find_project(&entry, target.slug())
    .await
    .map_err(|error| join_said(&error))?;
  let (reading, origin_root) = (programs.clone(), root.clone());
  let origin = background::run(move || checkout_origin(&reading, &origin_root))
    .await
    .map_err(|_| Said::new("cloud.errors.local"))?;
  let link = Servers::target(entry.clone(), project.clone(), origin.as_ref());
  let plan = servers
    .link_plan(&entry, &root, project)
    .await
    .map_err(|error| join_said(&error))?;
  if let LinkKind::Review { .. } = plan.kind {
    let review = servers
      .join_review_at(&entry, &root, target.slug())
      .await
      .map_err(|error| join_said(&error))?;
    return Ok(Resolved::Review(Arc::new(Reviewed {
      entry,
      review,
      remote: link.remote,
    })));
  }
  let remote = match &link.remote {
    RemoteMatch::Same => link.project.remote_hint.clone(),
    RemoteMatch::Different { checkout, .. } => Some(checkout.clone()),
    RemoteMatch::NoHint | RemoteMatch::NoOrigin => None,
  };
  Ok(Resolved::Link(Arc::new(Upload {
    entry,
    plan,
    remote,
    key: IdempotencyKey::generate(),
  })))
}

/// A server that answers a non-member like no project says neither.
fn join_said(error: &SharingError) -> Said {
  match error {
    SharingError::NoProject => Said::new("sharing.join.noProject"),
    error => sharing_said(error),
  }
}
