//! An item answered in place: its task with its latest run and the run's requests are read off the UI thread
//! (`tasks::needs_you::InPlace`), again as the project's runs and waiting requests change, and the card of the
//! run's timeline that answers it is the conversation's own (`screens::task::cards`), shown bare inside the
//! item: the same answers, rules and person gates, sent to the project's runner as the person. While it is
//! read the item says so; when it cannot be read, why; when the request is not on the task's latest run, that
//! it is answered in the conversation.

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  node::Element,
};
use orchester_core::id::{ProjectId, TaskId};
use orchester_workspace::{background, messages, tasks::needs_you::InPlace};

use crate::{
  project::Projects,
  screens::task::{
    cards::{self, Place},
    links, step_label,
  },
  services::Services,
  theme::ExtraTypography,
  ui::{field::field_error, message::message_text, state::loading},
};

/// Which card answers which item.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct InPlaceProps {
  pub(super) id: String,
  pub(super) project: ProjectId,
  pub(super) task: TaskId,
  /// The card's key in its run's timeline.
  pub(super) key: Arc<str>,
  /// "03 Implement", the step its run is on.
  pub(super) step: Option<(usize, Arc<str>)>,
  /// The project's waiting requests and questions, by id: a change reads the run's requests again.
  pub(super) waiting: Arc<[Arc<str>]>,
}

impl DevtoolsInspectable for InPlaceProps {}

/// What was read.
#[derive(Clone, Debug, PartialEq)]
struct Read(Arc<InPlace>);

impl DevtoolsInspectable for Read {}

pub(super) struct InPlaceCard;

impl Component for InPlaceCard {
  type Props = InPlaceProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<InPlaceProps>().clone();
    let projects = Projects::of(ctx);
    let Some(open) = projects.state(props.project).and_then(|state| state.open().cloned()) else {
      return Element::new();
    };
    let feed = projects.feed(props.project);
    let online = feed.runner_online(&open.repo_root);
    let hold = Services::of(ctx).hold;
    let deps = (
      open.repo_root,
      props.task,
      (feed.changes.get(), feed.revision.get()),
      props.waiting.clone(),
    );
    let read = ctx.future(deps, move |(root, task, ..): (PathBuf, TaskId, _, _)| {
      let hold = hold.clone();
      async move {
        hold.released().await;
        background::run(move || InPlace::read(&root, task))
          .await?
          .map(|read| Read(Arc::new(read)))
          .map_err(|error| {
            tracing::error!(%error, "cannot read the request to answer it in place");
            messages::store_error(&error)
          })
      }
    });
    let state = read.state().get();
    let read = match (state.data, state.error) {
      (Some(Read(read)), _) => read,
      (None, Some(error)) => {
        return field_error(&format!("{}-error", props.id), &message_text(ctx, &error)).into();
      }
      (None, None) => return loading(&format!("{}-reading", props.id), &ctx.t("needs_you.reading")).into(),
    };
    let lost = read.detail.lost_its_runner(online);
    let Some(item) = read.card(&props.key, lost) else {
      return lurq::components::Text::new(&ctx.t("needs_you.notHere"))
        .id(format!("{}-not-here", props.id))
        .variant(ExtraTypography::Note.role())
        .into();
    };
    let place = Place {
      project: props.project,
      detail: read.detail.clone(),
      key: props.key.clone(),
      step: props.step.as_ref().map(|(number, name)| step_label(ctx, *number, name)),
      step_label,
      links: links(),
      in_panel: true,
    };
    cards::card(ctx, place, &item)
  }
}
