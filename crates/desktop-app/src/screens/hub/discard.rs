//! Discard draft, asked in place of the draft bar (`Discard draft v5?` in `06c`): the draft's changes are lost and
//! the latest published version is the item again. The draft goes only while it is still the one the editor showed
//! (`orchester_workspace::removal::discard_in`); otherwise it says why and nothing is lost.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
  },
  node::Element,
};
use orchester_core::resource::VersionNumber;
use orchester_workspace::{background, messages::Message, tasks::outside::Outside};
use parking_lot::Mutex;

use super::question::{Question, Weight};
use crate::{
  project::ShownProject,
  services::Services,
  ui::{icon::Icon, message::message_text},
};

type Handler = Arc<dyn Fn() + Send + Sync>;
/// Discards the draft the editor showed, off the UI thread; the version the item is again.
pub(crate) type Discard = Arc<dyn Fn() -> Result<VersionNumber, Message> + Send + Sync>;

#[derive(Clone)]
pub(crate) struct DiscardProps {
  pub project: ShownProject,
  /// The number the draft would publish as, and the latest published version's.
  pub versions: (u32, u32),
  pub discard: Discard,
  /// What the screens that show the item read again.
  pub change: Outside,
  pub on_keep: Handler,
  pub on_discarded: Handler,
}

impl PartialEq for DiscardProps {
  /// The editor makes `discard` anew for the draft it shows now, so a draft saved while the question asks is the one
  /// it discards.
  fn eq(&self, other: &Self) -> bool {
    (self.versions, self.change, &self.project) == (other.versions, other.change, &other.project)
      && Arc::ptr_eq(&self.discard, &other.discard)
  }
}

impl DevtoolsInspectable for DiscardProps {}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Discarded(VersionNumber);

impl DevtoolsInspectable for Discarded {}

/// The question, mounted while it asks.
pub(crate) fn question(ctx: &mut Ctx, props: DiscardProps) -> Element {
  ctx.mount_keyed::<DiscardQuestion>("hub-discard", props)
}

pub(crate) struct DiscardQuestion {
  work: FutureAction<(), Discarded, Message>,
  /// The discard of the draft the editor shows now, which the confirm runs.
  latest: Arc<Mutex<Discard>>,
}

impl Component for DiscardQuestion {
  type Props = DiscardProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<DiscardProps>().clone();
    let latest = Arc::new(Mutex::new(props.discard.clone()));
    let current = latest.clone();
    let work = ctx.future_action(move |()| {
      let discard = current.lock().clone();
      async move { background::run(move || discard().map(Discarded)).await? }
    });
    let (changes, project, change, done) = (
      Services::of(ctx).changes,
      props.project.open.project.id(),
      props.change,
      props.on_discarded.clone(),
    );
    ctx.watch(&work.state(), move |state| {
      if state.is_fulfilled() && state.data.is_some() {
        changes.record(project, change);
        done();
      }
    });
    Self { work, latest }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<DiscardProps>().clone();
    // An autosave while the question asks gives the editor a new draft; the confirm discards that one.
    *self.latest.lock() = props.discard.clone();
    let (draft, published) = props.versions;
    let state = self.work.state().get();
    let failure = state
      .error
      .clone()
      .filter(|_| state.is_rejected())
      .map(|error| message_text(ctx, &error));
    let (work, keep) = (self.work.clone(), props.on_keep.clone());
    Element::from(
      Question::new(
        "hub-discard",
        Weight::Loses,
        (
          ctx.t_args("hub.discard.title", [("version", draft.to_string())]),
          ctx.t_args("hub.discard.sentence", [("version", published.to_string())]),
        ),
        ((Icon::Undo2, ctx.t("hub.discard.confirm")), ctx.t("hub.discard.keep")),
      )
      .failure(failure)
      .on_confirm(state.is_pending(), move || work.run(()))
      .on_keep(move || keep()),
    )
  }
}
