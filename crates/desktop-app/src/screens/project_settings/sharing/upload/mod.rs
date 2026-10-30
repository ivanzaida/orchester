//! The first link's page (`KSnOW`, which `N9NFio` reuses for linking a checkout to an empty project): over Project
//! settings with its breadcrumb, *Link Kontur to orchester.kontur.dev/kontur?*, what goes up and what continues on the
//! server ([`parts`]), whether to write `orchester.toml`, what linking sends, and *Link and upload*, which only a
//! person presses. A checkout without tasks takes the project's tasks as they are, on the same page. Nothing is sent
//! before; *Cancel* returns to Sharing with the project still local, and a link that failed says why and offers *Try
//! again*. When the project's tasks changed after the page read them (one was added in another tab or over MCP), the
//! link sends nothing and the page offers *Review again*, which reads what linking does now; a person presses it too.

mod parts;

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
use orchester_core::{id::IdempotencyKey, repository::RepositoryIdentity};
use orchester_workspace::sharing::{
  ServerEntry, Servers, SharingError,
  project::{
    ShareError,
    first_link::{LinkKind, LinkPlan, LinkRequest},
  },
};

use super::super::page;
use crate::{
  project::{Projects, ShownProject},
  screens::settings::orchester::{PersonOnly, Said, sharing_said, shown_address},
  services::Services,
  theme::{ExtraSpacing, TaskFormSpacing},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    callout::{Callout, CalloutTone},
    icon::Icon,
  },
};

/// What the page links: the project on its server as read, and the checkout's `origin` for its remote hint.
pub struct Upload {
  pub entry: ServerEntry,
  pub plan: LinkPlan,
  pub remote: Option<RepositoryIdentity>,
  /// The upload's idempotency key, made when the page opened: a retry links once.
  pub key: IdempotencyKey,
}

/// The page, open over Project settings while it holds what it links.
#[derive(Clone, Default)]
pub struct LinkPage(pub Option<Arc<Upload>>);

impl PartialEq for LinkPage {
  fn eq(&self, other: &Self) -> bool {
    match (&self.0, &other.0) {
      (Some(one), Some(two)) => Arc::ptr_eq(one, two),
      (None, None) => true,
      _ => false,
    }
  }
}

impl DevtoolsInspectable for LinkPage {}

/// *Link and upload*, which only a person presses.
const CONFIRM: &str = "sharing-link-confirm";
/// *Review again*, which only a person presses: it opens the link's page again for a person to confirm.
const REVIEW: &str = "sharing-upload-review-again";
/// *Write orchester.toml into the repository*, which only a person turns on or off.
const DISCOVERY: &str = "sharing-upload-discovery-switch";

/// Why the link did not go ahead.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Failure {
  /// The project's tasks are no longer those the page shows: nothing was sent, and the page is read again.
  Changed,
  Other(Said),
}

impl DevtoolsInspectable for Failure {}

#[derive(Clone)]
struct Props {
  project: ShownProject,
  upload: Arc<Upload>,
  page: Signal<LinkPage>,
}

impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project && Arc::ptr_eq(&self.upload, &other.upload)
  }
}

impl DevtoolsInspectable for Props {}

/// The page over Project settings for `upload`; *Cancel* and a finished link put it away through `page`.
pub fn link_page(ctx: &mut Ctx, project: &ShownProject, upload: Arc<Upload>, page: &Signal<LinkPage>) -> Element {
  // Keyed by the upload too: the page read again after *Review again* starts afresh.
  ctx.mount_keyed::<UploadPage>(
    &format!(
      "project-settings-link-page-{}-{}",
      project.open.project.id(),
      upload.key
    ),
    Props {
      project: project.clone(),
      upload,
      page: page.clone(),
    },
  )
}

struct UploadPage {
  /// Whether to write `orchester.toml` naming the project (on as `KSnOW` draws it).
  write: Signal<bool>,
  link: FutureAction<(), (), Failure>,
  /// *Review again*: what linking does now, once the tasks changed.
  review: FutureAction<(), (), Said>,
  person: PersonOnly,
}

impl Component for UploadPage {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let servers = Services::of(ctx).servers;
    let write = ctx.signal(true);
    let (wanted, root) = (write.clone(), props.project.open.repo_root.clone());
    let link = ctx.future_action(move |(): ()| {
      let (servers, root, upload) = (servers.clone(), root.clone(), props.upload.clone());
      let write_discovery = wanted.get();
      async move {
        let request = LinkRequest {
          remote: upload.remote.clone(),
          key: upload.key,
          write_discovery,
        };
        match servers.link_checkout(&upload.entry, &root, &upload.plan, request).await {
          Ok(shared) => {
            if let Some(Err(error)) = shared.discovery {
              // The link stands; Sharing's discovery row says what the repository's file holds.
              tracing::warn!(%error, "orchester.toml was not written after linking");
            }
            Ok(())
          }
          Err(ShareError::Sharing(SharingError::ProjectChanged)) => Err(Failure::Changed),
          Err(ShareError::Taken { .. }) => Err(Failure::Other(Said::new("sharing.upload.taken"))),
          Err(ShareError::Sharing(error)) => Err(Failure::Other(sharing_said(&error))),
        }
      }
    });
    let props = ctx.props::<Props>().clone();
    let (projects, feed, page) = (Projects::of(ctx), props.project.feed.clone(), props.page.clone());
    let project = props.project.open.project.id();
    ctx.watch(&link.state(), move |state| {
      if state.is_fulfilled() && state.error.is_none() {
        page.set(LinkPage::default());
        // The project has the server project's key now, and the tasks that came down are the server's: both are read
        // again, as after a join (`join::review`).
        projects.retry(project);
        feed.saved();
      }
    });
    let (servers, root) = (Services::of(ctx).servers, props.project.open.repo_root.clone());
    let review = ctx.future_action(move |(): ()| {
      let (servers, root, upload, page) = (servers.clone(), root.clone(), props.upload.clone(), props.page.clone());
      async move { review_again(&servers, &root, &upload, &page).await }
    });
    Self {
      write,
      link,
      review,
      person: PersonOnly::new(ctx),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let header = self.header(ctx, &props);
    let state = self.link.state().get();
    let mut content = Column::new()
      .id("sharing-upload")
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg);
    if let Some(failure) = state.error.clone().filter(|_| !state.is_pending()) {
      content = content.child(self.failed(ctx, &props, &failure));
    }
    let local = ("local", props.project.open.project.name().as_str().to_owned());
    // Once the tasks changed, the button reads what linking does now instead of linking.
    let nothing_yet = if state.error == Some(Failure::Changed) && !state.is_pending() {
      ctx.t_args("sharing.upload.nothingYetChanged", [local])
    } else {
      ctx.t_args(
        "sharing.upload.nothingYet",
        [("action", action_label(ctx, &props.upload.plan).to_string()), local],
      )
    };
    content = content
      .child(parts::what_goes(ctx, &props.project, &props.upload))
      .child(parts::discovery(ctx, &props.upload, (&self.write, &self.person)))
      .child(super::link::sends(ctx))
      .child(page::icon_note("sharing-upload-nothing-yet", Icon::Info, &nothing_yet));
    page::screen(header, page::page_scroll(content))
  }
}

impl UploadPage {
  /// The breadcrumb, *Link Kontur to …?*, what it does, and *Cancel* with *Link and upload* (`Head` `JKMvl`).
  fn header(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let local = props.project.open.project.name().as_str().to_owned();
    let project = &props.upload.plan.project;
    let server = props.upload.entry.stored.name.clone();
    let close = {
      let page = props.page.clone();
      move || page.set(LinkPage::default())
    };
    let crumbs = crumbs(ctx, (&local, &server), close.clone());
    let title = ctx.t_args(
      "sharing.upload.title",
      [("local", local.as_str()), ("address", shown_address(&project.address))],
    );
    let lead = parts::lead(ctx, props);
    let text = Column::new()
      .flex(1.0)
      .spacing(SpacingSize::Xs)
      .child(
        Text::new(&title)
          .id("sharing-upload-title")
          .variant(TypographyStyle::Heading),
      )
      .child(
        Text::new(&lead)
          .id("sharing-upload-lead")
          .variant(TypographyStyle::Description)
          .width(Dimension::Pct(100.0)),
      );
    let pending = self.link.state().get().is_pending();
    let cancel = ActionButton::new(
      "sharing-upload-cancel",
      ButtonKind::Secondary,
      ctx.t("sharing.upload.cancel"),
    );
    let cancel = if pending {
      cancel.disabled()
    } else {
      cancel.on_click(close)
    };
    let actions = Row::new()
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center)
      .child(cancel)
      .child(self.confirm(ctx, &props.upload.plan, pending));
    let column = Column::new()
      .width(Dimension::Px(page::WIDTH))
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
      .with_children(self.person.reason(ctx, CONFIRM))
      .with_children(self.person.reason(ctx, REVIEW));
    Column::new()
      .id("sharing-upload-head")
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

  /// *Link and upload 24 tasks* (or *Link* when nothing goes up), *Linking…* while it runs, *Try again* after a
  /// failure, and *Review again* once the project's tasks changed.
  fn confirm(&self, ctx: &Ctx, plan: &LinkPlan, pending: bool) -> ActionButton {
    let state = self.link.state().get();
    if !pending && state.error == Some(Failure::Changed) {
      return self.review_again(ctx);
    }
    let failed = state.error.is_some() && !pending;
    let (label, glyph) = if pending {
      (ctx.t("sharing.upload.linking"), Icon::Upload)
    } else if failed {
      (ctx.t("sharing.upload.tryAgain"), Icon::RefreshCw)
    } else {
      (action_label(ctx, plan), action_icon(plan))
    };
    let button = ActionButton::new(CONFIRM, ButtonKind::Primary, label).icon(glyph);
    if pending {
      return button.loading();
    }
    let (link, person) = (self.link.clone(), self.person.clone());
    button.on_click(move || {
      if person.allows(CONFIRM) {
        link.run(());
      }
    })
  }

  /// *Review again*, which reads what linking does now and opens the page for it; only a person presses it, as
  /// *Link and upload*.
  fn review_again(&self, ctx: &Ctx) -> ActionButton {
    let button =
      ActionButton::new(REVIEW, ButtonKind::Primary, ctx.t("sharing.upload.reviewAgain")).icon(Icon::RefreshCw);
    if self.review.state().get().is_pending() {
      return button.loading();
    }
    let (review, person) = (self.review.clone(), self.person.clone());
    button.on_click(move || {
      if person.allows(REVIEW) {
        review.run(());
      }
    })
  }

  /// Why the link did not go ahead: the tasks changed since the page read them, or what failed.
  fn failed(&self, ctx: &Ctx, props: &Props, failure: &Failure) -> Element {
    let names = [
      ("local", props.project.open.project.name().as_str()),
      ("project", props.upload.plan.project.name.as_str()),
    ];
    let callout = match failure {
      Failure::Changed => {
        let review = self.review.state().get();
        let body = match review.error.clone().filter(|_| !review.is_pending()) {
          Some(reason) => reason.text(ctx),
          None => ctx.t_args("sharing.upload.changedBody", names).to_string(),
        };
        Callout::new(
          "sharing-upload-changed",
          CalloutTone::Warning,
          ctx.t_args("sharing.upload.changed", names),
        )
        .body(body)
      }
      Failure::Other(reason) => Callout::new(
        "sharing-upload-failed",
        CalloutTone::Danger,
        ctx.t_args("sharing.upload.failed", names),
      )
      .body(reason.text(ctx)),
    };
    callout.into()
  }
}

/// What linking does now for `upload`'s project, on a page of its own with a new upload key. A checkout that has
/// tasks now, linked to a project with tasks, is a join, whose review this version doesn't have.
async fn review_again(
  servers: &Servers,
  root: &std::path::Path,
  upload: &Upload,
  page: &Signal<LinkPage>,
) -> Result<(), Said> {
  let plan = servers
    .link_plan(&upload.entry, root, upload.plan.project.clone())
    .await
    .map_err(|error| sharing_said(&error))?;
  if let LinkKind::Review { .. } = plan.kind {
    return Err(Said::new("sharing.upload.taken"));
  }
  page.set(LinkPage(Some(Arc::new(Upload {
    entry: upload.entry.clone(),
    plan,
    remote: upload.remote.clone(),
    key: IdempotencyKey::generate(),
  }))));
  Ok(())
}

/// Kontur › Project settings › Sharing › Link to Kontur team; the settings' crumbs put the page away with `close`.
fn crumbs(ctx: &mut Ctx, (local, server): (&str, &str), close: impl Fn() + Clone + Send + Sync + 'static) -> Row {
  let crumbs = vec![
    Crumb::here(local.to_owned()).ancestor(),
    Crumb::link(ctx.t("project_settings.title"), close.clone()),
    Crumb::link(ctx.t("project_settings.sharing"), close),
    Crumb::here(ctx.t_args("sharing.upload.crumb", [("server", server)])),
  ];
  breadcrumb(ctx, "sharing-upload-crumbs", CrumbSeparator::Chevron, crumbs)
}

/// *Link and upload* shows what goes up; a link that uploads nothing is a plain link.
fn action_icon(plan: &LinkPlan) -> Icon {
  match &plan.kind {
    LinkKind::Upload { tasks, .. } if !tasks.is_empty() => Icon::Upload,
    _ => Icon::Link,
  }
}

/// What the confirming button says.
fn action_label(ctx: &Ctx, plan: &LinkPlan) -> Arc<str> {
  match &plan.kind {
    LinkKind::Upload { tasks, .. } if !tasks.is_empty() => {
      ctx.t_args("sharing.upload.confirm", [("count", tasks.len().to_string())])
    }
    _ => ctx.t("sharing.upload.confirmLink"),
  }
}
