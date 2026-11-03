//! *Export conversation…* of a run (ORC-327, 09c `O3jhaZ`, 09d `A2YMy`): on the Conversation tab *More actions* gains
//! a group after *Copy link* with *Copy conversation as Markdown*, the messages only, and *Export conversation…*, which
//! opens the export dialog (`ui::chat::export`) on the task's latest run, the whole run with every step: the task's key
//! and title, the run, how many steps ran and how many messages it holds. The Conversation tab hands its timeline here
//! as it builds it ([`RunExport::publish`]); until it did, the group is not offered.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_core::{id::RunId, time::Timestamp};
use orchester_workspace::{
  export::run::{self, RunFacts},
  tasks::timeline::Item,
};
use parking_lot::Mutex;

use super::controls::{Control, Offer};
use crate::{
  screens::task::TaskContext,
  ui::{
    button::ButtonKind,
    chat::export::{Exporting, Source},
    icon::Icon,
    pressable::Press,
  },
};

/// The timeline of the run the Conversation tab shows, as it built it last.
struct Shown {
  run: RunId,
  items: Arc<Vec<Item>>,
}

/// Which run's timeline the tab published; `None` until it did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Published(Option<RunId>);

impl lurq::app::component::DevtoolsInspectable for Published {}

/// The run's export, the page's: the tab publishes what it shows, the header's menu and dialog export it.
#[derive(Clone)]
pub struct RunExport {
  exporting: Exporting,
  shown: Arc<Mutex<Option<Shown>>>,
  /// Which run's timeline the tab published, so the menu offers the group once there is one.
  ready: Signal<Published>,
}

impl RunExport {
  pub fn new(ctx: &mut Ctx) -> Self {
    Self {
      exporting: Exporting::new(ctx),
      shown: Arc::default(),
      ready: ctx.signal(Published::default()),
    }
  }

  /// The tab built `items`, the timeline of `run`.
  pub fn publish(&self, run: RunId, items: &Arc<Vec<Item>>) {
    *self.shown.lock() = Some(Shown {
      run,
      items: items.clone(),
    });
    if self.ready.get_untracked() != Published(Some(run)) {
      self.ready.set(Published(Some(run)));
    }
  }

  /// *Copy conversation as Markdown* and *Export conversation…*, once the tab showed the latest run of `task`.
  pub(super) fn controls(&self, ctx: &Ctx, task: &TaskContext) -> Vec<Control> {
    let Some(run) = task
      .detail
      .run
      .as_ref()
      .filter(|run| self.ready.get() == Published(Some(run.id())))
    else {
      return Vec::new();
    };
    let Some((source, _)) = self.source(ctx, task, run.id()) else {
      return Vec::new();
    };
    let exporting = self.exporting.clone();
    let copy = Press::run(move || exporting.copy_messages(source.clone()));
    let exporting = self.exporting.clone();
    let export = Press::run(move || exporting.show());
    vec![
      Control::new(
        ctx,
        ("task-copy-conversation", "ui.chat.export.copyAll", Icon::Copy),
        ButtonKind::Ghost,
        Offer::Ready(copy),
      ),
      Control::new(
        ctx,
        ("task-export-conversation", "ui.chat.export.menu", Icon::Download),
        ButtonKind::Ghost,
        Offer::Ready(export),
      ),
    ]
  }

  /// The export dialog while it is open.
  pub(super) fn dialog(&self, ctx: &mut Ctx, task: &TaskContext) -> Option<Element> {
    if !self.exporting.open.get() {
      return None;
    }
    let run = task.detail.run.as_ref()?;
    let (source, what) = self.source(ctx, task, run.id())?;
    self.exporting.dialog(ctx, "task-export-dialog", (&source, what))
  }

  /// What the export of `run` takes, and what the dialog says it covers.
  fn source(&self, ctx: &Ctx, task: &TaskContext, run: RunId) -> Option<(Source, Arc<str>)> {
    let items = self
      .shown
      .lock()
      .as_ref()
      .filter(|shown| shown.run == run)?
      .items
      .clone();
    let number = task.detail.run.as_ref()?.run.number().get();
    let facts = RunFacts {
      key: task.detail.task.key().to_string(),
      title: task.detail.task.details().title.as_str().to_owned(),
      number,
    };
    let what = what(ctx, &facts, &items);
    let file_name = facts.file_name();
    let source = Source {
      root: task.project.open.repo_root.clone(),
      file_name,
      hints: (
        ctx.t("task.export.messagesOnlyHint"),
        ctx.t("task.export.withToolsHint"),
      ),
      build: Arc::new(move |include, words, screen| {
        run::markdown(&facts, &items, include, (words, screen), Timestamp::now())
      }),
    };
    Some((source, what))
  }
}

/// *KON-12 · Add retry to webhook sender · run 8 · 5 steps · 63 messages*.
fn what(ctx: &Ctx, facts: &RunFacts, items: &[Item]) -> Arc<str> {
  let span = run::span(items);
  let count = |(one, other): (&str, &str), count: usize| {
    let words = count.to_string();
    ctx.t_args(if count == 1 { one } else { other }, [("count", words.as_str())])
  };
  let steps = count(("task.export.steps.one", "task.export.steps.other"), span.steps);
  let messages = count(
    ("task.export.messages.one", "task.export.messages.other"),
    span.messages,
  );
  let number = facts.number.to_string();
  ctx.t_args(
    "task.export.what",
    [
      ("key", facts.key.as_str()),
      ("title", facts.title.as_str()),
      ("number", number.as_str()),
      ("steps", &*steps),
      ("messages", &*messages),
    ],
  )
}
