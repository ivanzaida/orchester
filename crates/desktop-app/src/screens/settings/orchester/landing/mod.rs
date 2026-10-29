//! The invite landing (`Invite landing` `q7dWu` in `ZUuMT`, its states in `d8oDsO`): after a project invite is
//! accepted and this computer joined its server, which local checkout to link to the project. It is a sub-page of
//! Settings › Orchester Cloud, in the tab's column with its crumb back and its own head (16/600, as `03b · Account`'s).
//! The one checkout linked to it already, or the only one whose `origin` matches the project's hint, opens the join
//! review at once; otherwise the page lists the checkouts with how each compares ([`rows`]), adds a folder Orchester
//! doesn't know yet, or shows the `git clone` to run first (Orchester never clones). *Continue to the join review*,
//! under the checkouts, opens it in the chosen project; nothing is sent until the review is confirmed. *Later* leaves
//! the invite accepted: the server's entry in Settings › Orchester lists the project with *Link a checkout…*.

mod adding;
mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  core::Signal,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  sharing::{
    Servers,
    project::landing::{CheckoutRefusal, Landing, Linked},
  },
};
use parking_lot::Mutex;

use super::{Said, sharing_said};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, OrchesterView, ProjectPage, ProjectSettingsTab, Route},
  screens::settings::frame,
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    notice::{Notice, NoticeTone},
    state::{load_error, loading},
  },
};

#[derive(Clone, PartialEq)]
struct Props {
  target: JoinTarget,
}

impl DevtoolsInspectable for Props {}

/// The project and its checkouts as read.
#[derive(Clone)]
struct Read(Arc<Landing>);

impl PartialEq for Read {
  fn eq(&self, other: &Self) -> bool {
    Arc::ptr_eq(&self.0, &other.0)
  }
}

impl DevtoolsInspectable for Read {}

/// The checkout the person picked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Picked(pub Option<ProjectId>);

impl DevtoolsInspectable for Picked {}

/// What *Add a checkout folder…* came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Added {
  /// The picker was closed without a folder.
  Closed,
  Project(ProjectId),
  /// The folder isn't a git checkout.
  NotGit(PathBuf),
  Failed(PathBuf),
}

impl DevtoolsInspectable for Added {}

/// The invite landing for `target`, as the Orchester Cloud tab's column.
pub fn tab(ctx: &mut Ctx, target: &JoinTarget) -> Element {
  ctx.mount_keyed::<LandingPage>(
    &format!("orchester-landing-{}-{}", target.server, target.slug()),
    Props { target: target.clone() },
  )
}

struct LandingPage {
  picked: Signal<Picked>,
  add: FutureAction<Arc<str>, Added, Said>,
  /// Counts the reads: *Try again*, and a folder added.
  revision: Signal<u64>,
  /// The landing opened the review at once; it does so once.
  skipped: Mutex<bool>,
  /// *Later* was pressed: the page says where the invite waits.
  later: Signal<bool>,
}

impl Component for LandingPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let picker = PathPicker::of(ctx);
    let servers = Services::of(ctx).servers;
    let add = ctx.future_action(move |title: Arc<str>| {
      let (picker, servers) = (picker.clone(), servers.clone());
      async move {
        let Some(folder) = picker.pick(PathKind::Folder, title).await else {
          return Ok(Added::Closed);
        };
        Ok(match servers.add_checkout(folder.clone()).await {
          Ok(open) => Added::Project(open.project.id()),
          Err(CheckoutRefusal::NotGit) => Added::NotGit(folder),
          Err(error) => {
            tracing::warn!(%error, folder = %folder.display(), "a checkout folder could not be added");
            Added::Failed(folder)
          }
        })
      }
    });
    let (picked, revision) = (ctx.signal(Picked::default()), ctx.signal(0));
    let (choose, again) = (picked.clone(), revision.clone());
    ctx.watch(&add.state(), move |state| {
      if let Some(Added::Project(project)) = state.data.as_ref().filter(|_| state.is_fulfilled()) {
        choose.set(Picked(Some(*project)));
        again.update(|count| *count += 1);
      }
    });
    Self {
      picked,
      add,
      revision,
      skipped: Mutex::new(false),
      later: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let target = ctx.props::<Props>().target.clone();
    let services = Services::of(ctx);
    let deps = (
      self.revision.get(),
      target.clone(),
      (services.servers.clone(), services.programs),
    );
    let state = ctx
      .future(deps, |(_, target, (servers, programs))| async move {
        read(&servers, &programs, &target)
          .await
          .map(|landing| Read(Arc::new(landing)))
      })
      .state()
      .get();
    let body = match (state.data, state.error) {
      (Some(Read(landing)), _) => {
        if self.open_at_once(ctx, &landing, &target) {
          loading("orchester-landing-opening", &ctx.t("sharing.landing.opening")).into()
        } else {
          let body = if self.later.get() {
            self.waits(ctx, &landing)
          } else {
            let actions = self.actions(ctx, &landing, &target);
            rows::body(ctx, &landing, (&self.picked, &self.add), actions)
          };
          return page(ctx, Some(&landing), &target, body);
        }
      }
      (None, Some(reason)) => {
        let (revision, text) = (self.revision.clone(), reason.text(ctx));
        load_error(
          ctx,
          "orchester-landing-failed",
          (&ctx.t("sharing.landing.loadFailed"), &text),
          move || revision.update(|count| *count += 1),
        )
      }
      (None, None) => loading("orchester-landing-loading", &ctx.t("sharing.landing.reading")).into(),
    };
    page(ctx, None, &target, body)
  }
}

impl LandingPage {
  /// Opens the review in the checkout `landing` names as the one, or the Sharing of the one linked to the project
  /// already, once: the page itself is skipped.
  fn open_at_once(&self, ctx: &mut Ctx, landing: &Landing, target: &JoinTarget) -> bool {
    let Some(checkout) = landing.direct() else {
      return false;
    };
    let mut skipped = self.skipped.lock();
    if !*skipped {
      *skipped = true;
      // A checkout linked to the project already shows its Sharing; nothing is joined again.
      let page = if checkout.linked == Some(Linked::This) {
        ProjectPage::Settings(ProjectSettingsTab::Sharing)
      } else {
        ProjectPage::Join(target.clone())
      };
      Navigation::of(ctx).replace(Route::Project(checkout.id, page));
    }
    true
  }

  /// *Later* and *Continue to the join review*, at the end of the row under the checkouts (`Actions` `JZzjA`).
  fn actions(&self, ctx: &mut Ctx, landing: &Landing, target: &JoinTarget) -> [Element; 2] {
    let later = self.later.clone();
    let later = ActionButton::new(
      "orchester-landing-later",
      ButtonKind::Ghost,
      ctx.t("sharing.landing.later"),
    )
    .on_click(move || later.set(true));
    [later.into(), self.continue_button(ctx, landing, target).into()]
  }

  /// *Kontur waits in Settings › Orchester* (`Later` in `d8oDsO`): the invite stays accepted and nothing is linked;
  /// *Link a checkout…* comes back to the list, *Open Settings › Orchester* opens the server's entry, which lists the
  /// project with *Link a checkout…*.
  fn waits(&self, ctx: &mut Ctx, landing: &Landing) -> Element {
    let names = [
      ("project", landing.project.name.as_str()),
      ("server", landing.entry.stored.name.as_str()),
    ];
    let back = self.later.clone();
    let navigation = Navigation::of(ctx);
    let entry = Route::Orchester(OrchesterView::server(landing.entry.stored.id));
    Notice::new(
      "orchester-landing-waits",
      NoticeTone::Info,
      ctx.t_args("sharing.landing.waits.title", names),
    )
    .icon(Icon::CircleHelp)
    .body(ctx.t_args("sharing.landing.waits.body", names))
    .stacked()
    .action(
      ActionButton::new(
        "orchester-landing-waits-link",
        ButtonKind::Primary,
        ctx.t("sharing.landing.waits.link"),
      )
      .icon(Icon::Link)
      .on_click(move || back.set(false)),
    )
    .action(
      ActionButton::new(
        "orchester-landing-waits-open",
        ButtonKind::Secondary,
        ctx.t("sharing.landing.waits.open"),
      )
      .icon(Icon::ArrowUpRight)
      .opens(navigation.to(entry)),
    )
    .into()
  }

  /// *Continue to the join review*, once a checkout that can be linked is picked; it sends nothing.
  fn continue_button(&self, ctx: &mut Ctx, landing: &Landing, target: &JoinTarget) -> ActionButton {
    let button = ActionButton::new(
      "orchester-landing-continue",
      ButtonKind::Primary,
      ctx.t("sharing.landing.continue"),
    )
    .icon(Icon::ArrowRight);
    let picked = self
      .picked
      .get()
      .0
      .or_else(|| rows::first_match(landing))
      .filter(|picked| {
        landing
          .checkouts
          .iter()
          .any(|checkout| checkout.id == *picked && checkout.choosable())
      });
    match picked {
      Some(project) => {
        let navigation = Navigation::of(ctx);
        button.opens(navigation.to(Route::Project(project, ProjectPage::Join(target.clone()))))
      }
      None => button.unavailable(ctx.t("sharing.landing.pickFirst")),
    }
  }
}

/// The crumb back to Orchester Cloud, *Join Kontur on Kontur team* with who this computer is there once the project is
/// read, and `body` under them, in the tab's scrolling column.
fn page(ctx: &mut Ctx, landing: Option<&Landing>, target: &JoinTarget, body: Element) -> Element {
  let navigation = Navigation::of(ctx);
  let project = landing.map_or_else(|| target.slug().to_owned(), |landing| landing.project.name.clone());
  let (title, lead) = match landing {
    Some(landing) => (
      ctx.t_args(
        "sharing.landing.title",
        [
          ("project", project.as_str()),
          ("server", landing.entry.stored.name.as_str()),
        ],
      ),
      rows::lead(ctx, landing),
    ),
    None => (
      ctx.t_args("sharing.landing.titlePending", [("project", project.as_str())]),
      None,
    ),
  };
  let lead = lead.map(|lead| frame::lead("orchester-landing-lead", &lead).into());
  let pane = frame::pane("orchester-landing-page")
    .child(frame::crumb(
      "orchester-landing-crumb",
      &ctx.t("settings.tabs.orchester"),
      navigation.to(Route::Orchester(OrchesterView::default())),
    ))
    .child(frame::head("orchester-landing-head", None, (&title, lead), Vec::new()))
    .child(body);
  frame::column("orchester-landing-scroll", pane)
}

/// The project the target names, with this computer's checkouts.
async fn read(
  servers: &Servers,
  programs: &orchester_exec::programs::Programs,
  target: &JoinTarget,
) -> Result<Landing, Said> {
  let all = servers.clone();
  let entry = background::run(move || all.entries())
    .await
    .map_err(|_| Said::new("cloud.errors.local"))?
    .map_err(|error| sharing_said(&error))?
    .into_iter()
    .find(|entry| entry.stored.id == target.server && entry.in_use())
    .ok_or_else(|| Said::new("sharing.join.noServer"))?;
  servers
    .landing(&entry, target.slug(), programs)
    .await
    .map_err(|error| match error {
      orchester_workspace::sharing::SharingError::NoProject => Said::new("sharing.join.noProject"),
      error => sharing_said(&error),
    })
}
