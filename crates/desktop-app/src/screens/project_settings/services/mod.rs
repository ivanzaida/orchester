//! Services (`01 · Services` `G1sdS`, `01b · Services · none yet` `CdjWu`, `docs/architecture/agent-testing.md`): the
//! project's services, each with its driver, needs, owned paths and whether its start asks a person; its stacks with
//! the order they start their services in and the slots they take; its test suites with their layer; and the entry to
//! Flaky tests ([`groups`]). With no service yet it says what a service is and offers Import draft and New service.
//! A service's row and New service open the service's page ([`editor`]), a stack's row and New stack the stack's
//! ([`stack`]), a suite's row and New suite the suite's ([`suite`]); Import draft and Flaky tests say they are not
//! built yet ([`not_built`]). The three editors share their delete modal ([`delete`]) and how they say where they
//! stand ([`lifecycle`]).
//!
//! What the tab lists is read off the UI thread when it opens and when the project's tasks are saved; services that
//! automation adds while it shows appear when it is opened again.
mod delete;
mod editor;
mod groups;
mod lifecycle;
mod not_built;
mod stack;
mod suite;
mod words;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{agent_testing::listing::ProjectServices, background, messages};

use self::not_built::not_built;
use super::{TabProps, page};
use crate::{
  routes::ServicesPage,
  services::Services,
  theme::ExtraSpacing,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

/// The page `shown` under the tab: a service's, a stack's or a suite's editor, a New page, or a page not built yet.
pub(super) fn services_page(ctx: &mut Ctx, props: &TabProps, shown: ServicesPage) -> Element {
  match shown {
    ServicesPage::NewService => editor::editor(ctx, props, None),
    ServicesPage::Service(id) => editor::editor(ctx, props, Some(id)),
    ServicesPage::NewStack => stack::editor(ctx, props, None),
    ServicesPage::Stack(id) => stack::editor(ctx, props, Some(id)),
    ServicesPage::NewSuite => suite::editor(ctx, props, None),
    ServicesPage::Suite(id) => suite::editor(ctx, props, Some(id)),
    other => not_built(ctx, &props.project, other),
  }
}

pub(super) struct ServicesTab {
  /// Counts *Try again*s, each of which reads the list again.
  retry: Signal<u64>,
}

impl Component for ServicesTab {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self { retry: ctx.signal(0) }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let deps = (
      props.project.open.repo_root.clone(),
      props.project.feed.revision.get(),
      self.retry.get(),
    );
    let hold = Services::of(ctx).hold;
    let read = ctx
      .future(deps, move |(root, ..)| {
        let hold = hold.clone();
        async move {
          hold.released().await;
          background::run(move || ProjectServices::load_in(&root))
            .await?
            .map_err(|error| {
              tracing::error!(%error, "cannot read the project's services");
              messages::store_error(&error)
            })
        }
      })
      .state()
      .get();
    // A failed read shows with Try again; what was read before stays under it, so the list is never passed off as
    // current after reading it again failed.
    let failure = read.error.as_ref().filter(|_| !read.is_pending()).map(|error| {
      let retry = self.retry.clone();
      load_error(
        ctx,
        "settings-services-load-error",
        (
          &ctx.t("project_settings.services.loadFailed"),
          &message_text(ctx, error),
        ),
        move || retry.update(|count| *count += 1),
      )
    });
    let failed = failure.is_some();
    let mut shown = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .with_children(failure);
    if let Some(listed) = &read.data {
      // What was read stays while it is read again.
      shown = shown.child(groups::groups(ctx, &props, listed));
    } else if !failed {
      shown = shown.child(loading(
        "settings-services-loading",
        &ctx.t("project_settings.services.reading"),
      ));
    }
    page::scroll(shown)
  }
}
