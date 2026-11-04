//! Services (`01 · Services` `G1sdS`, `01b · Services · none yet` `CdjWu`, `docs/architecture/agent-testing.md`): the
//! project's services, each with its driver, needs, owned paths and whether its start asks a person; its stacks with
//! the order they start their services in and the slots they take; its test suites with their layer; and the entry to
//! Flaky tests ([`groups`]). With no service yet it says what a service is and offers Import draft and New service.
//! A row, New and Import draft open their pages under the tab, which say they are not built yet ([`not_built`]).
//!
//! What the tab lists is read off the UI thread when it opens and when the project's tasks are saved; services that
//! automation adds while it shows appear when it is opened again.
mod groups;
mod not_built;
mod words;

use lurq::{
  app::{component::Component, ctx::Ctx},
  components::Column,
  core::Signal,
  node::{Element, dimension::Dimension},
};
use orchester_workspace::{agent_testing::listing::ProjectServices, background, messages};

pub(super) use self::not_built::not_built;
use super::{TabProps, page};
use crate::{
  services::Services,
  theme::ExtraSpacing,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

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
