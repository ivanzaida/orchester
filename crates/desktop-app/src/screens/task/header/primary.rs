//! The header's one primary action (`Primary action` `xxoqb` in `j8DakG`), the one the task's state calls for: Share
//! task for a task kept *Not shared*; what the run waits for when a card waits (Approve and Answer open the
//! Conversation at its card; Review opens the Summary at its decision); for a run that waits to be merged, the merge's
//! own action with its key cap, on every tab, as the Summary's top card has it (ORC-731, `crate::screens::task::merge`:
//! *Merge*, or once a merge did not land *Try again* or the attempt page's action); else the run control that moves the
//! run on (Run, Start run, Resume run, Continue run); none when nothing applies. Any other card's action is left out
//! on the tab that shows the card, which holds it (`Decision cards` `CqVv3`).

use orchester_core::run::RunStatus;

use super::controls::{Control, Offer};
use crate::{
  navigation::{
    Navigation,
    pointed::{Pointed, PointedCard},
    press::OpenIn,
  },
  routes::{Route, TaskTab, TaskView},
  screens::task::{
    PageState, TaskContext,
    merge::{self, Merge},
    summary::Waiting,
  },
  ui::{button::ButtonKind, icon::Icon, pressable::Press},
};

/// What a waiting card is, for the action that opens it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Card {
  /// A request of the run's agent waits for the person's approval, in the Conversation.
  Approval,
  /// The run's agent asks the person, in the Conversation.
  Question,
  /// A decision on the Summary.
  Decision(Waiting),
}

impl Card {
  /// The card that waits for the person on the task's page, if one does.
  fn waiting(task: &TaskContext) -> Option<Self> {
    if task.detail.not_shared() {
      return None;
    }
    if task.waiting_approval().is_some() {
      return Some(Self::Approval);
    }
    if let Some(decision) = crate::screens::task::summary::waiting(task) {
      return Some(Self::Decision(decision));
    }
    let run = task.detail.run.as_ref().filter(|_| !task.lost())?;
    let asks = *run.run.status() == RunStatus::Running
      && (!run.run.awaiting_answer().is_empty() || !run.waiting_questions.is_empty());
    asks.then_some(Self::Question)
  }

  /// The tab the card is on.
  const fn tab(self) -> TaskTab {
    match self {
      Self::Approval | Self::Question => TaskTab::Conversation,
      Self::Decision(_) => TaskTab::Summary,
    }
  }

  /// The action's id, words and glyph.
  const fn look(self) -> (&'static str, &'static str, Icon) {
    match self {
      Self::Approval => ("task-primary-approve", "task.primary.approve", Icon::Check),
      Self::Question => ("task-primary-answer", "task.primary.answer", Icon::MessageSquare),
      Self::Decision(Waiting::Ready) => ("task-primary-merge", "task.primary.merge", Icon::GitMerge),
      Self::Decision(_) => ("task-primary-review", "task.primary.review", Icon::Eye),
    }
  }

  /// The Summary's card the action brings into view and rings, where it has one.
  const fn pointed(self) -> Option<PointedCard> {
    match self {
      Self::Decision(Waiting::Plan) => Some(PointedCard::Plan),
      Self::Decision(Waiting::Limit) => Some(PointedCard::RaiseLimit),
      Self::Decision(Waiting::Retries) => Some(PointedCard::Retries),
      _ => None,
    }
  }
}

/// The primary action of `task` on `view`; `moves`, the run control that moves the run on, when nothing waits.
pub fn primary(
  ctx: &mut lurq::app::ctx::Ctx,
  task: &TaskContext,
  view: &TaskView,
  moves: Option<Control>,
) -> Option<Control> {
  if task.detail.not_shared() {
    return Some(share(ctx));
  }
  let card = Card::waiting(task);
  // The pull request's CI calls for its own action once the run ended with one, in place of a merge (ORC-335).
  if matches!(card, None | Some(Card::Decision(Waiting::Ready)))
    && let Some(control) = crate::screens::task::ci::header::primary(ctx, task)
  {
    return Some(control);
  }
  // The merge's action, with its keys, on every tab, the Summary's top card too (01i `k9GBy`, 01j `k4C0Es`).
  if card == Some(Card::Decision(Waiting::Ready))
    && let Some(merge) = Merge::of(task)
    && let Some(action) = merge::action(ctx, task, merge, false)
  {
    return Some(Control {
      id: "task-primary-merge".into(),
      ..action.control
    });
  }
  let Some(card) = card else {
    return moves;
  };
  // On its own tab the card is in sight, and holds the action.
  if view.tab() == card.tab() && view.sub_name().is_none() {
    return None;
  }
  let navigation = Navigation::of(ctx);
  let project = task.project.open.project.id();
  let route = Route::Project(project, task.page(TaskView::of(card.tab())));
  let press = match card.pointed() {
    Some(pointed) => {
      let pointed = Pointed {
        project,
        task: task.key.clone(),
        card: pointed,
      };
      Press::run(move || navigation.open_to_card(&route, OpenIn::ActiveTab, pointed.clone()))
    }
    None => Press::Open(navigation.to(route)),
  };
  Some(Control::new(ctx, card.look(), ButtonKind::Primary, Offer::Ready(press)))
}

/// *Share task* of a task kept *Not shared*, which asks in place over the Summary (`TfVtK`); pressed while it asks.
fn share(ctx: &mut lurq::app::ctx::Ctx) -> Control {
  let share = PageState::of(ctx).share;
  let offer = if share.get() {
    Offer::Disabled
  } else {
    Offer::Ready(Press::run(move || share.set(true)))
  };
  Control::new(
    ctx,
    ("task-share", "task.notShared.share", Icon::CloudUpload),
    ButtonKind::Primary,
    offer,
  )
}
