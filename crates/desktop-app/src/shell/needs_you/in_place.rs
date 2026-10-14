//! An item answered in place: its task with its latest run and the run's requests are read off the UI thread
//! (`tasks::needs_you::InPlace`), again as the project's runs and waiting requests change, and the card of the
//! run's timeline that answers it is the conversation's own (`screens::task::cards`), shown bare inside the
//! item: the same answers, rules and person gates, sent to the project's runner by whoever gave them. While it is
//! read the item says so; when it cannot be read, why, also when reading it again failed: then nothing answers
//! it until a reading succeeds, so no answer goes from what may no longer hold; when the request is not on the
//! task's latest run, that it is answered in the conversation. *Open conversation* follows, unless the card carries it
//! in its own answers row. Once the item no longer waits, a note takes the card's place and says how it went (`note`).

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
  },
  components::{Column, Text},
  node::{Element, dimension::Dimension},
};
use orchester_core::id::{ProjectId, TaskId};
use orchester_workspace::{
  background,
  messages::{self, Message},
  tasks::{
    attention::NeedItem,
    needs_you::{InPlace, Settled},
  },
};
use parking_lot::Mutex;

use super::{item::open_conversation, note::note};
use crate::{
  project::Projects,
  screens::task::{
    cards::{self, Decisions, Place, Press},
    links, step_label,
  },
  services::Services,
  theme::{ExtraSpacing, ExtraTypography},
  ui::{field::field_error, message::message_text, state::loading},
};

/// Which card answers which item.
#[derive(Clone)]
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
  /// The item, once it no longer waits: its note takes the card's place.
  pub(super) gone: Option<NeedItem>,
  /// Opens the card in its task's conversation.
  pub(super) conversation: Press,
}

/// What opens the conversation is the same for one item, so it does not count.
impl PartialEq for InPlaceProps {
  fn eq(&self, other: &Self) -> bool {
    self.id == other.id
      && self.project == other.project
      && self.task == other.task
      && self.key == other.key
      && self.step == other.step
      && self.waiting == other.waiting
      && self.gone == other.gone
  }
}

impl DevtoolsInspectable for InPlaceProps {}

/// What was read.
#[derive(Clone, Debug, PartialEq)]
struct Read(Arc<InPlace>);

impl DevtoolsInspectable for Read {}

pub(super) struct InPlaceCard {
  /// Why the last reading that ended failed, until one succeeds; a reading under way keeps it.
  failed: Mutex<Option<Message>>,
}

impl Component for InPlaceCard {
  type Props = InPlaceProps;

  fn create(_ctx: &mut Ctx) -> Self {
    Self {
      failed: Mutex::new(None),
    }
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
    if let Some(item) = &props.gone {
      // Until a read says how it went, it says only that it no longer waits.
      let settled = state.data.as_ref().map_or(Settled::Unknown, |Read(read)| {
        read.settled(&props.key, read.detail.lost_its_runner(online))
      });
      return note(ctx, &props.id, (item, &settled), props.conversation.clone());
    }
    // A failed reading, also of what was read before, locks answering until a reading succeeds: what was
    // read may no longer hold.
    let failed = {
      let mut failed = self.failed.lock();
      if state.is_fulfilled() {
        *failed = None;
      } else if let Some(error) = &state.error {
        *failed = Some(error.clone());
      }
      failed.clone()
    };
    if let Some(error) = failed {
      let error = field_error(&format!("{}-error", props.id), &message_text(ctx, &error));
      return with_conversation(ctx, &props, error.into());
    }
    let Some(Read(read)) = state.data else {
      let reading = loading(&format!("{}-reading", props.id), &ctx.t("needs_you.reading"));
      return with_conversation(ctx, &props, reading.into());
    };
    let lost = read.detail.lost_its_runner(online);
    // The panel expands one item at a time, so its decisions follow that item's run, as a tab's follow its own.
    ctx
      .use_context::<Decisions>()
      .expect("the Needs you panel provides its decisions before its cards")
      .follow_runner(lost);
    let Some(item) = read.card(&props.key, lost) else {
      let not_here = Text::new(&ctx.t("needs_you.notHere"))
        .id(format!("{}-not-here", props.id))
        .variant(ExtraTypography::Note.role());
      return with_conversation(ctx, &props, not_here.into());
    };
    let hosts = cards::hosts_conversation(&item, lost);
    let place = Place {
      project: props.project,
      detail: read.detail.clone(),
      key: props.key.clone(),
      step: props.step.as_ref().map(|(number, name)| step_label(ctx, *number, name)),
      step_label,
      links: links(),
      in_panel: true,
      conversation: hosts.then(|| props.conversation.clone()),
    };
    let card = cards::card(ctx, place, &item);
    if hosts {
      return card;
    }
    with_conversation(ctx, &props, card)
  }
}

/// `content`, then *Open conversation*.
fn with_conversation(ctx: &Ctx, props: &InPlaceProps, content: Element) -> Element {
  Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(content)
    .child(open_conversation(ctx, &props.id, props.conversation.clone()))
    .into()
}
