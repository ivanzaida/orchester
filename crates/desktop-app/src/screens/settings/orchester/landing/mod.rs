//! The invite landing (`Join · invite accepted · choose the checkout` `xeFRB`, its states in `d8oDsO`): after a project
//! invite is accepted and this computer joined its server, which local checkout to link to the project. The one
//! checkout linked to it already, or the only one whose `origin` matches the project's hint, opens the join review at
//! once; otherwise the page lists the checkouts with how each compares ([`rows`]), adds a folder Orchester doesn't know
//! yet, or shows the `git clone` to run first (Orchester never clones). *Continue to the join review* opens it in the
//! chosen project; nothing is sent until the review is confirmed. *Later* leaves the invite accepted: the server's
//! entry in Settings › Orchester lists the project with *Link a checkout…*.

mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  sharing::{
    Servers,
    project::landing::{CheckoutRefusal, Landing},
  },
};
use parking_lot::Mutex;

use super::{Said, sharing_said};
use crate::{
  navigation::Navigation,
  routes::{JoinTarget, OrchesterView, ProjectPage, Route, Screen},
  services::{
    Services,
    path_picker::{PathKind, PathPicker},
  },
  theme::{ExtraSpacing, TaskFormSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    icon::Icon,
    state::{load_error, loading},
  },
};

/// The page's header (`Head` in `xeFRB`) and its body (`Body`), which is narrower.
const HEAD_WIDTH: f32 = 1000.0;
const BODY_WIDTH: f32 = 720.0;

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

/// The invite landing for `target`.
pub fn screen(ctx: &mut Ctx, target: &JoinTarget) -> Element {
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
        read(&servers, &programs, &target).await.map(|landing| Read(Arc::new(landing)))
      })
      .state()
      .get();
    let body = match (state.data, state.error) {
      (Some(Read(landing)), _) => {
        if self.open_at_once(ctx, &landing, &target) {
          loading("orchester-landing-opening", &ctx.t("sharing.landing.opening")).into()
        } else {
          let header = self.header(ctx, Some(&landing), &target);
          return page(header, rows::body(ctx, &landing, (&self.picked, &self.add)));
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
    let header = self.header(ctx, None, &target);
    page(header, body)
  }
}

impl LandingPage {
  /// Opens the review in the checkout `landing` names as the one, once: the page itself is skipped.
  fn open_at_once(&self, ctx: &mut Ctx, landing: &Landing, target: &JoinTarget) -> bool {
    let Some(checkout) = landing.direct() else {
      return false;
    };
    let mut skipped = self.skipped.lock();
    if !*skipped {
      *skipped = true;
      Navigation::of(ctx).replace(Route::Project(checkout.id, ProjectPage::Join(target.clone())));
    }
    true
  }

  /// The breadcrumb, *Join Kontur on Kontur team*, what happens next, *Later* and *Continue to the join review*.
  fn header(&self, ctx: &mut Ctx, landing: Option<&Landing>, target: &JoinTarget) -> Element {
    let navigation = Navigation::of(ctx);
    let server = target.server;
    let to_orchester = {
      let navigation = navigation.clone();
      move || navigation.go(Route::Orchester(OrchesterView::server(server)))
    };
    let project = landing.map_or_else(|| target.slug().to_owned(), |landing| landing.project.name.clone());
    let crumbs = breadcrumb(
      ctx,
      "orchester-landing-crumbs",
      CrumbSeparator::Chevron,
      vec![
        Crumb::link(ctx.t(Screen::Settings.title_key()), move || navigation.go(Route::Settings)),
        Crumb::link(ctx.t("settings.tabs.orchester"), to_orchester.clone()),
        Crumb::here(ctx.t_args("sharing.landing.crumb", [("project", project.as_str())])),
      ],
    );
    let (title, lead) = match landing {
      Some(landing) => (
        ctx.t_args(
          "sharing.landing.title",
          [("project", project.as_str()), ("server", landing.entry.stored.name.as_str())],
        ),
        rows::lead(ctx, landing),
      ),
      None => (ctx.t_args("sharing.landing.titlePending", [("project", project.as_str())]), None),
    };
    let later = ActionButton::new("orchester-landing-later", ButtonKind::Ghost, ctx.t("sharing.landing.later"))
      .on_click(to_orchester);
    let go_on = self.continue_button(ctx, landing, target);
    let text = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&title)
          .id("orchester-landing-title")
          .variant(TypographyStyle::Heading),
      )
      .with_children(lead.map(|lead| {
        Text::new(&lead)
          .id("orchester-landing-lead")
          .variant(TypographyStyle::Description)
          .width(Dimension::Pct(100.0))
      }));
    let column = Column::new()
      .width(Dimension::Px(HEAD_WIDTH))
      .spacing(SpacingSize::Sm)
      .child(crumbs)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .align_items(Alignment::Center)
          .child(text)
          .child(Spacer::new().width(Dimension::Px(0.0)))
          .child(later)
          .child(go_on),
      );
    Column::new()
      .id("orchester-landing-head")
      .width(Dimension::Pct(100.0))
      .align_items(Alignment::Center)
      .padding(
        Padding::new()
          .top(TaskFormSpacing::PageTop.role())
          .left(SpacingSize::Xl)
          .right(SpacingSize::Xl)
          .bottom(ExtraSpacing::Section.role()),
      )
      .border_bottom(Border::inside(BorderSize::Sm, PaletteColor::Border))
      .child(column)
      .into()
  }

  /// *Continue to the join review*, once a checkout that can be linked is picked; it sends nothing.
  fn continue_button(&self, ctx: &mut Ctx, landing: Option<&Landing>, target: &JoinTarget) -> ActionButton {
    let button = ActionButton::new(
      "orchester-landing-continue",
      ButtonKind::Primary,
      ctx.t("sharing.landing.continue"),
    )
    .icon(Icon::ArrowRight);
    let picked = self.picked.get().0.filter(|picked| {
      landing.is_some_and(|landing| {
        landing
          .checkouts
          .iter()
          .any(|checkout| checkout.id == *picked && checkout.choosable())
      })
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

/// The header over the scrolled body.
fn page(header: Element, body: Element) -> Element {
  Column::new()
    .id(format!("screen-{}", Screen::Settings.name()))
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(header)
    .child(
      ScrollVertical::new(
        Column::new()
          .width(Dimension::Pct(100.0))
          .align_items(Alignment::Center)
          .padding(Padding::symmetric(SpacingSize::Xl, SpacingSize::Xl))
          .child(
            Column::new()
              .width(Dimension::Px(HEAD_WIDTH))
              .child(Column::new().width(Dimension::Px(BODY_WIDTH)).child(body)),
          ),
      )
      .id("orchester-landing-scroll")
      .flex(1.0),
    )
    .into()
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
