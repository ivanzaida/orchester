//! The invite landing (`Invite landing` `q7dWu` in `ZUuMT`, its states in `d8oDsO`): after a project invite is
//! accepted and this computer joined its server, which local checkout to link to the project. It is a sub-page of
//! Settings › Orchester Cloud, in the tab's column with its crumb back and its own head (16/600, as `03b · Account`'s).
//! The one checkout linked to it already, or the only one whose `origin` matches the project's hint, opens the join
//! review at once; otherwise the page lists the checkouts with how each compares ([`rows`]), adds a folder Orchester
//! doesn't know yet, or shows the `git clone` to run first (Orchester never clones). *Continue to the join review*,
//! under the checkouts, opens it in the chosen project; nothing is sent until the review is confirmed. *Later* leaves
//! the invite accepted: the server's entry in Settings › Orchester lists the project with *Link a checkout…*. With the
//! project's server out of reach, nobody signed in, or signed in without the code accepting needs, the page asks for
//! the sign-in in place of the checkouts ([`sign_in`]), and goes on by itself once it is made.

mod adding;
mod rows;
mod sign_in;

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
    cloud::SignIn,
    project::landing::{CheckoutRefusal, Landing, Linked},
  },
};
use parking_lot::Mutex;

use super::{Said, answers::Answers, sharing_said, two_factor};
use crate::{
  navigation::Navigation,
  root::follow_sign_in,
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
    skeleton::{self, Skeleton},
    state::{load_error, loading},
  },
};

/// The words for a project on a server this computer can't reach now.
const NO_SERVER: &str = "sharing.join.noServer";

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
  /// *Sign in again*, a person's, while accepting the invite needs a sign-in with a code.
  answers: Answers,
}

impl Component for LandingPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let picker = PathPicker::of(ctx);
    let servers = Services::of(ctx).servers;
    let server = ctx.props::<Props>().target.server;
    let add = ctx.future_action(move |title: Arc<str>| {
      let (picker, servers) = (picker.clone(), servers.clone());
      async move {
        let Some(folder) = picker.pick(PathKind::Folder, title).await else {
          return Ok(Added::Closed);
        };
        Ok(match servers.add_checkout(folder.clone(), server).await {
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
      answers: Answers::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let target = ctx.props::<Props>().target.clone();
    let services = Services::of(ctx);
    // First in every render: lurq keeps a component's futures by the order it asks for them.
    let sign_in = follow_sign_in(ctx, &services.cloud);
    let known_server = known_server(ctx, target.server);
    let factors = two_factor::settled(ctx, 0, &services.cloud, sign_in.account());
    let deps = (
      self.revision.get(),
      sign_in.account(),
      target.clone(),
      (services.servers.clone(), services.programs),
    );
    let state = ctx
      .future(deps, |(_, _, target, (servers, programs))| async move {
        read(&servers, &programs, &target)
          .await
          .map(|landing| Read(Arc::new(landing)))
      })
      .state()
      .get();
    // The project's server is not this computer's to reach: accepting the invite may need a sign-in first.
    if state.error.as_ref().is_some_and(|said| *said == Said::new(NO_SERVER)) {
      let invite = (&target, known_server.as_deref());
      match (&sign_in, factors.map(|read| read.0)) {
        (SignIn::SignedOut { .. }, _) => return sign_in::signed_out(ctx, invite),
        // Whether the sign-in can accept is not read yet: the page waits rather than say the server can't be read.
        (SignIn::SignedIn { .. }, None) => {
          let body = loading("orchester-landing-loading", &ctx.t("sharing.landing.reading")).into();
          return page(ctx, None, &target, body);
        }
        (SignIn::SignedIn { email, .. }, Some(Some(status))) if status.enabled && !status.this_session => {
          return sign_in::again(ctx, invite, (email, &self.answers));
        }
        (SignIn::SignedIn { .. }, Some(_)) => {}
      }
    }
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
      (None, None) => {
        // No loading frame draws the invite's landing: its choices' rows are bones in one outline until read.
        let bones = skeleton::list(ctx, "orchester-landing-rows", &[(160.0, 280.0), (140.0, 240.0)]);
        Skeleton::new("orchester-landing-loading", ctx.t("sharing.landing.reading")).show(ctx, bones)
      }
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

/// The name of the server the target names, as this computer keeps it, read once off the UI thread; `None` until it is
/// read, or when it is not in the list.
fn known_server(ctx: &mut Ctx, server: orchester_core::id::ServerId) -> Option<String> {
  let workspace = Services::of(ctx).workspace;
  ctx
    .future((server, workspace), |(server, workspace)| async move {
      background::run(move || workspace.server_named(server))
        .await
        // Work that did not finish was logged where it ended; the page then names no server, which it can do without.
        .map_err(drop)?
        .map(KnownServer)
        .map_err(|error| tracing::warn!(%error, "the invite's server could not be read"))
    })
    .state()
    .get()
    .data
    .and_then(|known| known.0)
}

/// The name of an invite's server, for a future's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KnownServer(Option<String>);

impl DevtoolsInspectable for KnownServer {}

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
