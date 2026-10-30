//! *Download tasks*, the last step of the start page (`docs/design/startup.md` decision 7; `08 · First download`
//! `YN6MO`): each part of a shared project with its state ([`parts`]), *Open Kontur*, which works before the download
//! ends, since the tasks keep arriving in the project's Tasks list, and *Stay here*, which keeps the page and puts away
//! what says the project can open. The download itself is the shared projects' round
//! ([`orchester_workspace::sharing::project`]), so it goes on whichever screen shows; a failure shows in the tasks' row
//! with *Try again*, and what came down stays.
//!
//! A link of a checkout without tasks leads here. A project that is not linked says so, and one no longer listed too.

mod parts;
mod read;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;
use orchester_workspace::sharing::project::{DownloadProgress, DownloadState};

use self::read::{Linked, Standing};
use super::{
  join_team::frame::{Ancestor, StartStep, StepFrame, step_frame},
  settings::orchester::{follow, follow_shared, shown_address},
};
use crate::{
  navigation::Navigation,
  routes::{JoinWay, OrchesterView, ProjectPage, Route, TaskList},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    field::field_error,
    icon::Icon,
  },
};

/// The screen's elements start with it (`screen-first_download`).
const ID: &str = "first_download";

/// The step's screen for `project`.
pub(super) fn screen(ctx: &mut Ctx, project: ProjectId) -> Element {
  ctx.mount_keyed::<FirstDownloadPage>(&format!("first-download-{project}"), Props { project })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Props {
  project: ProjectId,
}

impl DevtoolsInspectable for Props {}

struct FirstDownloadPage {
  /// *Stay here* was pressed.
  stayed: Signal<bool>,
}

impl Component for FirstDownloadPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      stayed: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let Props { project } = *ctx.props::<Props>();
    let services = Services::of(ctx);
    let revision = follow(ctx, (&services.servers, &services.cloud)) + follow_shared(ctx, &services.servers);
    let standing = read::standing(ctx, revision, project);
    let root = standing.as_ref().ok().and_then(|standing| match standing {
      Some(Standing::Linked(linked)) => Some(linked.root.clone()),
      Some(Standing::Local { .. } | Standing::Missing) | None => None,
    });
    let origin = read::origin(ctx, root);
    match standing {
      Ok(Some(Standing::Linked(linked))) => {
        let progress = services.servers.download_progress(&linked.root);
        self.page(ctx, project, &linked, (origin.as_ref(), progress.as_ref()))
      }
      Ok(Some(Standing::Local { name })) => {
        let (title, lead) = (
          ctx.t_args("startup.shared.download.notShared.title", [("project", name.as_str())]),
          ctx.t("startup.shared.download.notShared.lead"),
        );
        let open = open_button(ctx, project, &name);
        let body = Row::new().child(open).into();
        frame(ctx, (Vec::new(), name.clone()), (&title, &lead), body)
      }
      Ok(Some(Standing::Missing)) => {
        let (title, lead) = (
          ctx.t("startup.shared.download.missing.title"),
          ctx.t("startup.shared.download.missing.lead"),
        );
        let crumb = ctx.t("shell.screens.firstDownload").to_string();
        frame(ctx, (Vec::new(), crumb), (&title, &lead), Column::new().into())
      }
      Ok(None) => Column::new().id(format!("screen-{ID}")).into(),
      Err(said) => {
        let (title, reason) = (ctx.t("startup.shared.download.unread"), said.text(ctx));
        let body = field_error(&format!("{ID}-unread"), &reason).into();
        let crumb = ctx.t("shell.screens.firstDownload").to_string();
        frame(ctx, (Vec::new(), crumb), (&title, ""), body)
      }
    }
  }
}

impl FirstDownloadPage {
  /// The step for the linked project: its parts, what says it can open, and the actions.
  fn page(
    &self,
    ctx: &mut Ctx,
    project: ProjectId,
    linked: &Linked,
    (origin, progress): (Option<&read::Origin>, Option<&DownloadProgress>),
  ) -> Element {
    let name = linked.name.as_str();
    let folder = linked.root.display().to_string();
    let downloading = progress.is_some_and(|progress| progress.state == DownloadState::Downloading);
    let staying = self.stayed.get();
    let can_open = (downloading && !staying).then(|| {
      Callout::new(
        format!("{ID}-can-open"),
        CalloutTone::Info,
        ctx.t_args("startup.shared.download.canOpen.title", [("project", name)]),
      )
      .body(ctx.t("startup.shared.download.canOpen.body"))
    });
    let stay = (downloading && !staying).then(|| {
      let stayed = self.stayed.clone();
      ActionButton::new(
        format!("{ID}-stay"),
        ButtonKind::Ghost,
        ctx.t("startup.shared.download.stay"),
      )
      .large()
      .on_click(move || stayed.set(true))
    });
    let actions = Row::new()
      .id(format!("{ID}-actions"))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(open_button(ctx, project, name))
      .with_children(stay);
    let body = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .child(parts::list(ctx, ID, linked, (origin, progress)))
      .with_children(can_open)
      .child(actions);
    let title_key = match progress.map(|progress| &progress.state) {
      Some(DownloadState::Downloading) => "startup.shared.download.title.downloading",
      Some(DownloadState::Done) => "startup.shared.download.title.done",
      Some(DownloadState::Failed { .. }) => "startup.shared.download.title.failed",
      None => "startup.shared.download.title.idle",
    };
    let title = ctx.t_args(title_key, [("project", name)]);
    let address = linked
      .sharing
      .server
      .as_ref()
      .map_or("", |(_, address)| shown_address(address.as_str()));
    let lead = ctx.t_args(
      "startup.shared.download.lead",
      [("project", name), ("folder", folder.as_str()), ("address", address)],
    );
    let ancestors = vec![
      Ancestor {
        label: ctx.t("startup.join.crumb").to_string(),
        route: Route::JoinTeam(JoinWay::default()),
      },
      Ancestor {
        label: linked.sharing.server_name().to_owned(),
        route: Route::Orchester(OrchesterView::server(linked.sharing.link.server)),
      },
    ];
    frame(ctx, (ancestors, name.to_owned()), (&title, &lead), body.into())
  }
}

/// *Open Kontur*, on its Tasks list, where the tasks keep arriving.
fn open_button(ctx: &mut Ctx, project: ProjectId, name: &str) -> ActionButton {
  let tasks = Route::Project(project, ProjectPage::Tasks(TaskList::default()));
  ActionButton::new(
    format!("{ID}-open"),
    ButtonKind::Primary,
    ctx.t_args("startup.shared.download.open", [("project", name)]),
  )
  .icon(Icon::ArrowRight)
  .large()
  .opens(Navigation::of(ctx).to(tasks))
}

/// `body` in the frame of the start page's steps.
fn frame(
  ctx: &mut Ctx,
  (ancestors, crumb): (Vec<Ancestor>, String),
  (title, lead): (&str, &str),
  body: Element,
) -> Element {
  step_frame(
    ctx,
    StepFrame {
      id: ID,
      ancestors,
      crumb,
      step: StartStep::Download,
      title: title.to_owned(),
      lead: lead.to_owned(),
      body,
      note: None,
    },
  )
}
