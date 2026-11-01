//! The join review (`Join KTR · review page` `AiCwO`): over Project settings with its breadcrumb, *Join KTR on Kontur
//! team?*, that a project of this computer's moves into the account whose server it is, the local tasks all ticked to
//! start with what each becomes ([`super::rows`]), how many go up, stay *Not shared* and go up claimed, what similar
//! titles mean, and what joining sends. *Join and upload N*, which only a person presses, uploads the ticked tasks in
//! one import; a join that failed says whether anything reached the server and keeps the ticks for *Try again*, which
//! uploads nothing twice. Once joined, Sharing says what it did.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{BorderSize, PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::{IdempotencyKey, TaskId};
use orchester_workspace::sharing::RemoteMatch;

use super::{
  Reviewed,
  plan::{Plan, Unticked},
  rows,
};
use crate::{
  navigation::Navigation,
  project::{Projects, ShownProject},
  routes::{ProjectPage, ProjectSettingsTab, Route},
  screens::{
    project_settings::{
      page,
      sharing::{link::moves_to_account, linked::joined},
    },
    settings::orchester::{PersonOnly, Said, sharing_said},
  },
  services::Services,
  theme::{ExtraRadius, ExtraSpacing, ExtraTypography, TaskFormSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
    notice::{Notice, NoticeTone},
  },
};

/// The review's width (`Body` `cK2ve` in `AiCwO`).
const WIDTH: f32 = 1000.0;

/// *Join and upload N*, which only a person presses.
const CONFIRM: &str = "sharing-join-confirm";

/// Why a join did not finish, and whether it left tasks on the server (`None`: the connection was lost while they went
/// up, so it may have).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Failure {
  uploaded: Option<bool>,
  reason: Said,
}

impl DevtoolsInspectable for Failure {}

#[derive(Clone)]
struct Props {
  project: ShownProject,
  reviewed: Arc<Reviewed>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && Arc::ptr_eq(&self.reviewed, &other.reviewed)
  }
}

impl DevtoolsInspectable for Props {}

/// The review of `reviewed` for `project`.
pub(super) fn page(ctx: &mut Ctx, project: &ShownProject, reviewed: Arc<Reviewed>) -> Element {
  ctx.mount_keyed::<Review>(
    &format!("sharing-join-review-{}", reviewed.review.project),
    Props {
      project: project.clone(),
      reviewed,
    },
  )
}

struct Review {
  unticked: Signal<Unticked>,
  join: FutureAction<Vec<TaskId>, (), Failure>,
  person: PersonOnly,
}

impl Component for Review {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let services = Services::of(ctx);
    let (servers, workspace, root) = (
      services.servers,
      services.workspace,
      props.project.open.repo_root.clone(),
    );
    let (reviewed, project) = (props.reviewed.clone(), props.project.open.project.id());
    let join = ctx.future_action(move |ticked: Vec<TaskId>| {
      let (servers, workspace, root, reviewed) = (servers.clone(), workspace.clone(), root.clone(), reviewed.clone());
      async move {
        // A new key each time: a join after a lost answer uploads only what the server lacks.
        let request = (ticked, IdempotencyKey::generate());
        match servers.join(&reviewed.entry, &root, &reviewed.review, request).await {
          Ok(done) => {
            let dropped = done.dropped;
            // Joined either way: a note that wasn't kept only leaves Sharing without it, and `keep` logs why.
            if orchester_workspace::background::run(move || joined::keep(&workspace, project, &dropped))
              .await
              .is_err()
            {
              tracing::warn!("what the join did was not kept for Sharing: the work was interrupted");
            }
            Ok(())
          }
          Err(failed) => Err(Failure {
            uploaded: failed.uploaded,
            reason: sharing_said(&failed.error),
          }),
        }
      }
    });
    let (navigation, projects, feed) = (Navigation::of(ctx), Projects::of(ctx), props.project.feed);
    ctx.watch(&join.state(), move |state| {
      if state.is_fulfilled() && state.error.is_none() {
        // The project has the server project's key now, and its tasks are the server's: both are read again.
        projects.retry(project);
        feed.saved();
        navigation.replace(Route::Project(
          project,
          ProjectPage::Settings(ProjectSettingsTab::Sharing),
        ));
      }
    });
    Self {
      unticked: ctx.signal(Unticked::default()),
      join,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let unticked = self.unticked.get();
    let plan = Plan::of(&props.reviewed.review, &unticked);
    let header = self.header(ctx, &props, &plan);
    let key = props.reviewed.review.key.as_str().to_owned();
    let state = self.join.state().get();
    let moves = props
      .reviewed
      .moves_to_account
      .then(|| moves_to_account(ctx, props.project.open.project.name().as_str()));
    let content = Column::new()
      .id("sharing-join")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .with_children(mismatch(ctx, &props))
      .with_children(moves)
      .with_children(
        state
          .error
          .clone()
          .filter(|_| !state.is_pending())
          .map(|failure| failed(ctx, &props, &failure)),
      )
      .child(summary(ctx, &plan, &key))
      .child(rows::table(ctx, &plan.rows, (&key, &self.unticked, &self.person)))
      .child(page::wide_note(
        "sharing-join-note",
        Icon::Info,
        &ctx.t_args("sharing.join.note", [("key", key.as_str())]),
      ))
      .child(super::super::link::join_sends(ctx));
    page::screen(header, page::scroll_wide(content, WIDTH))
  }
}

impl Review {
  /// The breadcrumb, *Join KTR on Kontur team*, what joining does, *Cancel* and *Join and upload N*.
  fn header(&self, ctx: &mut Ctx, props: &Props, plan: &Plan<'_>) -> Element {
    let review = &props.reviewed.review;
    let (local, key, server) = (
      props.project.open.project.name().as_str().to_owned(),
      review.key.as_str().to_owned(),
      props.reviewed.entry.stored.name.clone(),
    );
    let navigation = Navigation::of(ctx);
    let project = props.project.open.project.id();
    let sharing = Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::Sharing));
    let settings = Route::Project(project, ProjectPage::Settings(ProjectSettingsTab::default()));
    let crumbs = breadcrumb(
      ctx,
      "sharing-join-crumbs",
      CrumbSeparator::Chevron,
      vec![
        Crumb::here(local).ancestor(),
        Crumb::to(ctx.t("project_settings.title"), navigation.to(settings)),
        Crumb::to(ctx.t("project_settings.sharing"), navigation.to(sharing.clone())),
        Crumb::here(ctx.t_args("sharing.join.crumb", [("key", key.as_str())])),
      ],
    );
    let text = title(ctx, (&key, &server), review.tasks.len());
    let pending = self.join.state().get().is_pending();
    let cancel = ActionButton::new(
      "sharing-join-cancel",
      ButtonKind::Secondary,
      ctx.t("sharing.join.cancel"),
    );
    let cancel = if pending {
      cancel.disabled()
    } else {
      cancel.opens(navigation.to(sharing))
    };
    let actions = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(cancel)
      .child(self.confirm(ctx, plan, pending));
    let column = Column::new()
      .width(Dimension::Px(WIDTH))
      .spacing(SpacingSize::Sm)
      .child(crumbs)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Md)
          .align_items(Alignment::Start)
          .child(text)
          .child(Spacer::new().width(Dimension::Px(0.0)))
          .child(actions),
      )
      .with_children(self.person.reason(ctx, CONFIRM));
    head(column)
  }

  /// *Join and upload 4* (*Join without uploading* when nothing goes up), *Joining…* while it runs, *Try again* after
  /// a failure, with the ticks as they were.
  fn confirm(&self, ctx: &Ctx, plan: &Plan<'_>, pending: bool) -> ActionButton {
    let state = self.join.state().get();
    let count = plan.keys().len();
    let label = if pending {
      ctx.t("sharing.join.joining")
    } else if state.error.is_some() {
      ctx.t("sharing.join.tryAgain")
    } else if count == 0 {
      ctx.t("sharing.join.confirmNone")
    } else {
      ctx.t_args("sharing.join.confirm", [("count", count.to_string())])
    };
    let button = ActionButton::new(CONFIRM, ButtonKind::Primary, label).icon(Icon::LogIn);
    if pending {
      return button.loading();
    }
    let (join, person, ticked) = (self.join.clone(), self.person.clone(), plan.ticked());
    button.on_click(move || {
      if person.allows(CONFIRM) {
        join.run(ticked.clone());
      }
    })
  }
}

/// *Join KTR on Kontur team* over what joining does with the `count` local tasks.
fn title(ctx: &Ctx, (key, server): (&str, &str), count: usize) -> Column {
  let lead_key = if count == 1 {
    "sharing.join.description.one"
  } else {
    "sharing.join.description.other"
  };
  Column::new()
    .flex(1.0)
    .spacing(SpacingSize::Xs)
    .child(
      Text::new(&ctx.t_args("sharing.join.title", [("key", key), ("server", server)]))
        .id("sharing-join-title")
        .variant(TypographyStyle::Heading),
    )
    .child(
      Text::new(&ctx.t_args(lead_key, [("key", key.to_owned()), ("count", count.to_string())]))
        .id("sharing-join-lead")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
}

/// The page's head around `column`, a line under it (`Head` in `AiCwO`).
fn head(column: Column) -> Element {
  Column::new()
    .id("sharing-join-head")
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

/// *4 of 6 ticked* with what that comes to (`Summary` `G9uMd`).
fn summary(ctx: &Ctx, plan: &Plan<'_>, key: &str) -> Element {
  let (total, keys) = (plan.rows.len(), plan.keys());
  let separator = ctx.t("sharing.join.summary.separator");
  let uploaded = match keys.as_slice() {
    [] => ctx.t("sharing.join.summary.uploadedNone"),
    [one] => ctx.t_args("sharing.join.summary.uploaded.one", [("key", one.to_string())]),
    [first, .., last] => ctx.t_args(
      "sharing.join.summary.uploaded.other",
      [
        ("count", keys.len().to_string()),
        ("first", first.to_string()),
        ("last", last.to_string()),
      ],
    ),
  };
  let mut parts = vec![uploaded.to_string()];
  let (kept, claimed) = (plan.kept(), plan.claimed());
  if kept > 0 {
    parts.push(counted(
      ctx,
      ("sharing.join.summary.kept.one", "sharing.join.summary.kept.other"),
      kept,
    ));
  }
  if claimed > 0 {
    parts.push(counted(
      ctx,
      ("sharing.join.summary.claimed.one", "sharing.join.summary.claimed.other"),
      claimed,
    ));
  }
  Row::new()
    .id("sharing-join-summary")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Inset.role(), SpacingSize::Md))
    .spacing(SpacingSize::Md)
    .align_items(Alignment::Center)
    .background(PaletteColor::SurfaceRaised)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(ExtraRadius::Tile.role())
    .child(
      Text::new(&ctx.t_args(
        "sharing.join.ticked",
        [
          ("ticked", keys.len().to_string()),
          ("total", total.to_string()),
          ("key", key.to_owned()),
        ],
      ))
      .id("sharing-join-ticked")
      .variant(ExtraTypography::Strong.role())
      .nowrap(),
    )
    .child(
      Text::new(&parts.join(&separator))
        .id("sharing-join-summary-detail")
        .variant(TypographyStyle::Description)
        .flex(1.0),
    )
    .into()
}

/// `one` or `other` with `count`.
fn counted(ctx: &Ctx, (one, other): (&str, &str), count: usize) -> String {
  let key = if count == 1 { one } else { other };
  ctx.t_args(key, [("count", count.to_string())]).to_string()
}

/// The checkout's `origin` differs from the project's hint: the person chose to go on, and the page keeps saying so.
fn mismatch(ctx: &Ctx, props: &Props) -> Option<Element> {
  let RemoteMatch::Different { checkout, hint } = &props.reviewed.remote else {
    return None;
  };
  Some(
    Callout::new(
      "sharing-join-mismatch",
      CalloutTone::Warning,
      ctx.t_args("sharing.link.mismatch.title", [("origin", checkout.as_str())]),
    )
    .icon(Icon::TriangleAlert)
    .body(ctx.t_args(
      "sharing.link.mismatch.body",
      [
        ("project", props.reviewed.review.key.as_str()),
        ("server", props.reviewed.entry.stored.name.as_str()),
        ("hint", hint.as_str()),
      ],
    ))
    .into(),
  )
}

/// *Couldn't join KTR*: whether anything reached the server, that the ticks are kept, and why.
fn failed(ctx: &Ctx, props: &Props, failure: &Failure) -> Element {
  let names = [
    ("key", props.reviewed.review.key.as_str().to_owned()),
    ("project", props.project.open.project.name().as_str().to_owned()),
    ("server", props.reviewed.entry.stored.name.clone()),
  ];
  let body = match failure.uploaded {
    Some(false) => "sharing.join.failedNothing",
    Some(true) => "sharing.join.failedUploaded",
    None => "sharing.join.failedUnknown",
  };
  Notice::new(
    "sharing-join-failed",
    NoticeTone::Danger,
    ctx.t_args("sharing.join.failed", names.clone()),
  )
  .icon(Icon::CircleAlert)
  .body(ctx.t_args(body, names))
  .part(
    Text::new(&failure.reason.text(ctx))
      .id("sharing-join-failed-reason")
      .variant(ExtraTypography::Note.role()),
  )
  .into()
}
