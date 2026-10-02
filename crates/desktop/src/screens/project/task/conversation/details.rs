//! The Details pane (`gNa91`): the invocation panel of the step the timeline is narrowed to, else of
//! the latest launch, or of the step whose tokens were chosen in its header. Closed by default and
//! remembered for every task (the application database's `ui_state`): the tab's job is to follow and
//! to talk, and the figures are looked up now and then.

use lurq::{
  app::ctx::{Ctx, FutureAction},
  core::Signal,
  node::Element,
};
use orchester_core::id::StepExecutionId;

use super::{super::activity::invocation_panel, StepFilter, nav};
use crate::{
  background,
  messages::{self, Message},
  navigation::Navigation,
  tasks::{detail::RunView, transcript::Entry},
  workspace::Workspace,
};

/// Whether the pane is open and which step it is about.
#[derive(Clone)]
pub(super) struct DetailsPane {
  /// What the person chose in this tab; `None` until they choose, when the remembered state holds.
  pub(super) open: Signal<Option<bool>>,
  /// The step whose tokens were chosen in its header; `None` follows the step filter.
  pub(super) on: Signal<StepFilter>,
  pub(super) save: SaveAction,
}

pub(super) type SaveAction = FutureAction<bool, (), Message>;

fn workspace(ctx: &mut Ctx) -> Workspace {
  ctx
    .use_context::<Navigation>()
    .expect("the tab host provides Navigation")
    .workspace()
    .clone()
}

/// Remembers whether the pane is open; a failure only loses the choice for the next task, so it is
/// logged.
pub(super) fn save_action(ctx: &mut Ctx) -> SaveAction {
  let workspace = workspace(ctx);
  ctx.future_action(move |open: bool| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.save_conversation_details(open))
        .await?
        .map_err(|error| {
          tracing::warn!(%error, "whether the Details pane is open was not remembered");
          messages::store_error(&error)
        })
    }
  })
}

/// Whether the person kept the pane open last time; closed while that is read, or when it cannot
/// be, which the log says.
pub(super) fn remembered(ctx: &mut Ctx) -> bool {
  let workspace = workspace(ctx);
  let remembered = ctx.future((), move |()| {
    let workspace = workspace.clone();
    async move {
      background::run(move || workspace.conversation_details())
        .await?
        .map_err(|error| {
          tracing::warn!(%error, "whether the Details pane is open cannot be read; it is closed");
          messages::store_error(&error)
        })
    }
  });
  remembered.state().get().data.unwrap_or(false)
}

impl DetailsPane {
  /// Whether the pane shows: the person's choice in this tab, else what they chose last.
  pub(super) fn is_open(&self, remembered: bool) -> bool {
    self.open.get().unwrap_or(remembered)
  }

  /// Opens the pane while it is `open` closed, or closes it, and remembers it.
  pub(super) fn toggle(&self, open: bool) -> impl Fn() + Send + Sync + 'static {
    let (chosen, on, save) = (self.open.clone(), self.on.clone(), self.save.clone());
    move || {
      let open = !open;
      chosen.set(Some(open));
      if !open {
        on.set(StepFilter(None));
      }
      save.run(open);
    }
  }

  /// Opens the pane on `execution`, as choosing its tokens in its header does.
  pub(super) fn open_on(&self, execution: StepExecutionId) -> impl Fn() + Send + Sync + 'static {
    let (chosen, on, save) = (self.open.clone(), self.on.clone(), self.save.clone());
    move || {
      chosen.set(Some(true));
      on.set(StepFilter(Some(execution)));
      save.run(true);
    }
  }
}

/// The pane while it is open: about the step its tokens opened it on, else `filter`'s.
pub(super) fn details_pane(
  ctx: &mut Ctx,
  run: &RunView,
  entries: &[Entry],
  (details, open, filter): (&DetailsPane, bool, Option<StepExecutionId>),
) -> Option<Element> {
  if !open {
    return None;
  }
  let on = details.on.get().0.or(filter);
  let nav = nav(ctx);
  Some(invocation_panel(ctx, &nav, run, entries, on))
}
