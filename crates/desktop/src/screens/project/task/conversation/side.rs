//! The tab's layout (`wkUFK` in `kpnmP`): the chat at the left (toolbar, read-back lines, the chat
//! and the composer) and at the right the step rail, its strip (`I35q7D`) or Details in its place
//! (`oryKr`), never two of them; a collapsed rail opened in a narrow window shows over the chat.

use std::sync::Arc;

use lurq::{
  app::ctx::Ctx,
  components::{Column, Row, Stack},
  layout::StackAlignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::StepExecutionId;

use super::{
  super::activity::{DETAILS_WIDTH, LiveFooter, LiveFooterProps},
  ConversationProps, ConversationTab, StepFilter,
  approval_card::AnswersLayout,
  column,
  composer::{ComposerDock, ComposerProps},
  details::details_pane,
  follow::{LeftAt, Reveal},
  rail::{Picker, RAIL_WIDTH, RailProps, rail},
  readback::{self, Controls, readback},
  strip::{STRIP_WIDTH, strip},
  toolbar::{ToolbarState, toolbar},
};
use crate::{
  tasks::{
    conversation::PanelState,
    detail::RunView,
    timeline::{
      Item,
      load::Loaded,
      rail::{RailStep, rail as rail_of, summary},
      view::{self, Choice, Shown},
    },
  },
  theme::PROJECT_SIDEBAR_WIDTH,
};

/// What the panel at the right shows.
pub(super) enum Beside {
  Rail,
  Strip,
  Details,
}

impl Beside {
  const fn width(&self) -> f32 {
    match self {
      Self::Rail => RAIL_WIDTH,
      Self::Strip => STRIP_WIDTH,
      Self::Details => DETAILS_WIDTH,
    }
  }
}

/// The panel at the right, its width, and the rail over the chat while it shows there.
pub(super) struct Side {
  panel: Element,
  width: f32,
  over: Option<Element>,
}

/// Everything one render of the chat and its rail is made of.
struct View<'a> {
  props: &'a ConversationProps,
  run: &'a RunView,
  items: &'a [Item],
  loaded: &'a Loaded,
  /// The rail's steps, and the one picked.
  steps: &'a [RailStep],
  picked: Option<&'a RailStep>,
  choice: Choice,
  shown: Shown,
  /// While a step is picked and a search runs: what the search finds in every step.
  everywhere: Option<Shown>,
  beside: Beside,
  window: f32,
}

impl View<'_> {
  /// What the search finds in every step.
  fn found(&self) -> &Shown {
    self.everywhere.as_ref().unwrap_or(&self.shown)
  }
}

impl ConversationTab {
  pub(super) fn with_items(
    &self,
    ctx: &mut Ctx,
    props: &ConversationProps,
    run: &RunView,
    (items, loaded): (&[Item], &Loaded),
    details_open: bool,
  ) -> Element {
    let steps = rail_of(run, items);
    let filter = self.step.get().0;
    let picked = filter
      .as_ref()
      .and_then(|key| steps.iter().find(|step| step.key == *key && step.started()));
    let choice = Choice {
      show: self.show.get().0,
      step: picked.map(|step| step.executions.clone()),
      query: self.query.get(),
    };
    let toggles = self.toggles.clone();
    let open = |key: &str, default: bool| toggles.is_open(key, default);
    let shown = view::shown(items, &choice, &open);
    // The rail counts the matches of every step, whichever is picked.
    let everywhere = (picked.is_some() && !choice.query.trim().is_empty()).then(|| {
      let all = Choice {
        step: None,
        ..choice.clone()
      };
      view::shown(items, &all, &open)
    });
    let window = ctx.window().logical_width();
    let beside = if details_open {
      Beside::Details
    } else if self.rail.is_collapsed(window) {
      Beside::Strip
    } else {
      Beside::Rail
    };
    let view = View {
      props,
      run,
      items,
      loaded,
      steps: &steps,
      picked,
      choice,
      shown,
      everywhere,
      beside,
      window,
    };
    let main = self.chat(ctx, &view);
    let side = self.side(ctx, &view);
    self.layout(ctx, props, Some(side), main)
  }

  /// The toolbar, the read-back lines, the chat or why it is empty, *Jump to latest* and the working
  /// line.
  fn chat(&self, ctx: &mut Ctx, view: &View<'_>) -> Element {
    let conversation = &view.props.conversation;
    // An agent that waits in its turn for the answer to its question does not work: the run waits for the
    // person (C·15), so there is no working line and the toolbar says so.
    let asking = conversation.state == PanelState::Live && !view.run.waiting_questions.is_empty();
    let live = conversation.state == PanelState::Live && !asking;
    // Read so that a row's request to reveal another renders the chat again.
    self.revealed.get();
    let reveal = self.follow(&view.shown, view.items);
    let working = live.then(|| LiveFooterProps {
      agent: view
        .run
        .current_step()
        .and_then(|step| step.agent.clone())
        .unwrap_or_else(|| "".into()),
      last: view.loaded.entries.last().map(|entry| entry.at),
    });
    // The working line is the chat's last row; with nothing else to show, it follows what says why.
    let (body, end, working): (Element, _, _) = if view.shown.rows.is_empty() {
      // Without rows there is no list; the next one measures its rows again.
      if self.measured.get_untracked() {
        self.measured.set(false);
      }
      let below = working.map(|working| column(ctx.mount_keyed::<LiveFooter>("conversation-working", working)));
      (self.empty(ctx, view), None, below)
    } else {
      // The chat column is the window less the project sidebar and the panel beside the chat.
      let chat = view.window - PROJECT_SIDEBAR_WIDTH - view.beside.width();
      let answers = AnswersLayout::for_chat(chat);
      let list = self.list(ctx, (&view.shown, view.items), reveal, (working, answers));
      (list, Some(self.follow_end(ctx)), None)
    };
    let readback = readback(
      ctx,
      (&view.choice, &view.shown),
      (view.steps, view.picked),
      &Controls {
        step: &self.step,
        query: &self.query,
        show: &self.show,
      },
      live,
    );
    let jump = (!self.following.get() && !view.shown.rows.is_empty()).then(|| self.jump(ctx, view.shown.rows.len()));
    let state = ToolbarState {
      show: self.show.clone(),
      query: self.query.clone(),
      searching: self.searching.clone(),
      raw: self.raw.clone(),
      details: self.details.clone(),
      details_open: matches!(view.beside, Beside::Details),
      beside: view.beside.width(),
      state: if asking {
        PanelState::Waiting
      } else {
        conversation.state
      },
      asks: asking || conversation.open_question().is_some(),
      following: self.following.get(),
    };
    Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .child(toolbar(ctx, &state))
      .with_children(readback)
      .child(body)
      .with_children(end)
      .with_children(jump)
      .with_children(working)
      .into()
  }

  /// The rail, its strip or Details; the rail over the chat while the strip is opened in a narrow
  /// window.
  fn side(&self, ctx: &mut Ctx, view: &View<'_>) -> Side {
    let width = view.beside.width();
    if let Beside::Details = view.beside {
      let picked = view.picked.and_then(|step| step.executions.last().copied());
      let panel = details_pane(
        ctx,
        (view.run, &view.props.repo_root),
        &view.loaded.entries,
        (&self.details, picked),
      );
      return Side {
        panel,
        width,
        over: None,
      };
    }
    let searching = !view.choice.query.trim().is_empty();
    let picker = self.picker();
    let props = RailProps {
      steps: view.steps,
      summary: summary(view.run, view.steps),
      matches: searching.then(|| match_counts(view.steps, view.found())),
      filter: &self.step,
      toggle: &self.rail,
      picker: &picker,
    };
    if let Beside::Rail = view.beside {
      return Side {
        panel: rail(ctx, &props),
        width,
        over: None,
      };
    }
    Side {
      panel: strip(ctx, view.steps, (&self.step, &self.rail, &picker), view.window),
      width,
      over: self.rail.over.get().then(|| {
        Column::new()
          .id("conversation-rail-over")
          .height(Dimension::Pct(100.0))
          .child(rail(ctx, &props))
          .into()
      }),
    }
  }

  /// Picks a step in the rail or its strip: the chat narrows to it and opens at its start (`bdCsu`);
  /// all steps again go to the end and follow it. Over the chat, the rail closes.
  fn picker(&self) -> Picker {
    let (filter, over, left_at) = (self.step.clone(), self.rail.over.clone(), self.left_at.clone());
    let (scroll, following, reveal) = (self.scroll.clone(), self.following.clone(), self.reveal.clone());
    Arc::new(move |pick: StepFilter| {
      let all = pick.0.is_none();
      Reveal::let_go(&reveal);
      filter.set(pick);
      over.set(false);
      *left_at.lock() = LeftAt(None);
      if all {
        scroll.scroll_to_bottom_pending();
      } else {
        scroll.scroll_to_top_pending();
      }
      following.set(all);
    })
  }

  /// The tab: the chat above the composer, the panel beside it, and the rail over them while it
  /// shows there.
  pub(super) fn layout(&self, ctx: &mut Ctx, props: &ConversationProps, side: Option<Side>, main: Element) -> Element {
    let (panel, width, over) = side.map_or((None, 0.0, None), |side| (Some(side.panel), side.width, side.over));
    let composer = ctx.mount_keyed::<ComposerDock>(
      "conversation-composer-dock",
      ComposerProps {
        repo_root: props.repo_root.clone(),
        detail: props.detail.clone(),
        conversation: props.conversation.clone(),
        draft: self.draft.clone(),
        focus: props.focus.clone(),
        searching: self.searching.clone(),
        reload: props.reload.clone(),
        sent: self.sent_handler(),
        beside: width,
      },
    );
    let chat = Column::new()
      .id("conversation-main")
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .height(Dimension::Pct(100.0))
      .child(Column::new().width(Dimension::Pct(100.0)).flex(1.0).child(main))
      .child(composer);
    let body = Row::new()
      .id("conversation")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(chat)
      .with_children(panel);
    let Some(over) = over else {
      return body.into();
    };
    Stack::new()
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .stack_align(StackAlignment::TopEnd)
      .child(body.height(Dimension::Pct(100.0)))
      .child(over)
      .into()
  }

  /// Nothing to show: the run has not started, or a search found nothing (`FBwIV`, `JPBHA`).
  fn empty(&self, ctx: &mut Ctx, view: &View<'_>) -> Element {
    if view.choice.query.trim().is_empty() {
      return super::tab_state::not_started(ctx, Some(view.run));
    }
    readback::nothing_found(
      ctx,
      view.run,
      (&view.choice, view.picked, Some(view.found())),
      (&self.step, &self.query),
    )
  }
}

/// The matches of the whole run, and of each step of `steps`, from what a search shows.
fn match_counts(steps: &[RailStep], shown: &Shown) -> (usize, Vec<usize>) {
  let of = |execution: &StepExecutionId| {
    shown
      .step_matches
      .iter()
      .filter(|(matched, _)| matched == execution)
      .map(|(_, count)| count)
      .sum::<usize>()
  };
  let counts = steps.iter().map(|step| step.executions.iter().map(of).sum()).collect();
  (shown.matches, counts)
}
