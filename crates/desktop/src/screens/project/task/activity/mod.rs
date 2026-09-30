//! The Activity tab (02.I `SfXU1`): the agent transcript of the latest run, grouped by step
//! execution, with a step filter, the switch for unparsed lines and whether the list follows new
//! entries. While the run is live the list follows the newest entry; scrolling up stops that and
//! offers "Jump to latest". The states without a transcript are `hig8O`.

mod entry;
mod first_edit;
mod footer;
mod mcp;
mod panel;
mod problem;
mod rows;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    events::scroll_event::ScrollEvent,
    theme::{BorderSize, SpacingSize},
  },
  components::{Column, Row, Select, Spacer, VirtualizedList},
  core::Signal,
  layout::{
    Alignment,
    layout_kind::{Justify, ScrollState},
  },
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{connection::AgentProvider, id::StepExecutionId};
use parking_lot::Mutex;

use self::{
  entry::TranscriptRowView,
  footer::{LiveFooter, LiveFooterProps},
  rows::{OpenRows, Toggles, TranscriptRow},
};
use super::tab_state;
use crate::{
  background, messages,
  messages::Message,
  runner::Runners,
  screens::project::connections::status::short_key as provider_short_key,
  tasks::{
    detail::{RunView, TaskDetail, activity_in},
    transcript::{Entry, entries},
  },
  theme::{ExtraColor, ExtraSpacing, gutter_scrollbar},
  ui::{button::ActionButton, field::select_style, icon::Icon, status_label::compact_status_label, switch::Switch},
};

/// Toolbar, step filter and the Jump to latest button (`c1dmZ`, `rYSoT`, `iwe3p`).
const TOOLBAR_HEIGHT: f32 = 56.0;
const FILTER_WIDTH: f32 = 240.0;
const FILTER_HEIGHT: f32 = 32.0;
const JUMP_HEIGHT: f32 = 30.0;
/// How close to the end still counts as at the end.
const FOLLOW_SLACK: f32 = 24.0;

#[derive(Clone, Debug, PartialEq)]
pub struct ActivityProps {
  pub repo_root: PathBuf,
  pub detail: Arc<TaskDetail>,
  /// Changes whenever the runner reports a change: the transcript is read again.
  pub revision: (u64, u64),
  /// An agent is at work on the run: its runner is connected and the run is running.
  pub live: bool,
}

impl lurq::app::component::DevtoolsInspectable for ActivityProps {}

/// The step execution the transcript is narrowed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StepFilter(Option<StepExecutionId>);

impl lurq::app::component::DevtoolsInspectable for StepFilter {}

/// The stored transcript as entries.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Transcript(Arc<Vec<Entry>>);

impl lurq::app::component::DevtoolsInspectable for Transcript {}

pub struct ActivityTab {
  filter: Signal<StepFilter>,
  raw: Signal<bool>,
  following: Signal<bool>,
  retry: Signal<u64>,
  toggles: Toggles,
  scroll: ScrollState,
  /// Rows when the list stopped following, for "N new".
  left_at: Signal<usize>,
  /// Rows of the last render, to notice new ones.
  shown: Arc<Mutex<usize>>,
}

impl Component for ActivityTab {
  type Props = ActivityProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      filter: ctx.signal(StepFilter(None)),
      raw: ctx.signal(false),
      following: ctx.signal(true),
      retry: ctx.signal(0),
      toggles: Toggles(ctx.signal(OpenRows::default())),
      scroll: ScrollState::new(),
      left_at: ctx.signal(0),
      shown: Arc::new(Mutex::new(0)),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ActivityProps>().clone();
    let Some(run) = props.detail.run.as_ref() else {
      return tab_state::empty_activity(ctx, &ctx.t("runs.activity.emptyNoRun"));
    };
    let deps = (props.repo_root.clone(), run.id(), props.revision, self.retry.get());
    let load = ctx.future(deps, |(repo_root, run, ..)| async move {
      background::run(move || activity_in(&repo_root, run))
        .await?
        .map(|activity| Transcript(Arc::new(entries(&activity))))
        .map_err(|error| {
          tracing::error!(%error, "cannot read the run's transcript");
          messages::store_error(&error)
        })
    });
    let state = load.state().get();
    let transcript = match (state.data, state.error) {
      (Some(transcript), _) => transcript,
      (None, Some(error)) => return self.failed(ctx, &error),
      (None, None) => {
        let number = run.run.number().get().to_string();
        return tab_state::state(
          "activity-loading",
          Icon::Loader,
          &ctx.t_args("runs.activity.loadingTitle", [("run", number.as_str())]),
          &ctx.t("runs.activity.loadingDetail"),
          Vec::new(),
        );
      }
    };
    Row::new()
      .id("activity")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .child(self.transcript(ctx, run, &transcript.0, props.live))
      .child(panel::invocation_panel(ctx, run, self.filter.get().0))
      .into()
  }
}

impl ActivityTab {
  fn transcript(&self, ctx: &mut Ctx, run: &RunView, entries: &[Entry], live: bool) -> Element {
    let filter = self.filter.get().0;
    let rows = Arc::new(rows::rows(run, entries, filter, self.raw.get(), &self.toggles));
    let following = self.following.get();
    self.follow(rows.len(), following);
    let list: Element = if rows.is_empty() {
      let key = if live {
        "runs.activity.emptyStarting"
      } else {
        "runs.activity.emptyNone"
      };
      tab_state::empty_activity(ctx, &ctx.t(key))
    } else {
      self.list(ctx, rows.clone())
    };
    let jump = (!following && !rows.is_empty()).then(|| self.jump(ctx, rows.len()));
    let footer = live.then(|| {
      let last = entries.last().map(|entry| entry.at);
      let agent = run
        .current_step()
        .and_then(|step| step.agent.clone())
        .unwrap_or_else(|| "".into());
      ctx.mount_keyed::<LiveFooter>("activity-footer", LiveFooterProps { agent, last })
    });
    Column::new()
      .flex(1.0)
      .height(Dimension::Pct(100.0))
      .padding_horizontal(SpacingSize::Section)
      .child(self.toolbar(ctx, run, live, following))
      .child(list)
      .with_children(jump)
      .with_children(footer)
      .into()
  }

  /// Keeps the newest row in view while following.
  fn follow(&self, rows: usize, following: bool) {
    let mut shown = self.shown.lock();
    if following && rows > *shown {
      self.scroll.scroll_to_bottom_pending();
    }
    *shown = rows;
  }

  fn list(&self, ctx: &mut Ctx, rows: Arc<Vec<TranscriptRow>>) -> Element {
    let (scroll, following, left_at, count) = (
      self.scroll.clone(),
      self.following.clone(),
      self.left_at.clone(),
      rows.len(),
    );
    let scrollbar = gutter_scrollbar();
    VirtualizedList::new_shared(ctx, rows)
      .list_key("activity-rows")
      .width(Dimension::Pct(100.0))
      .flex(1.0)
      .with_scroll_state(self.scroll.clone())
      .scrollbar(scrollbar)
      .on_scroll(move |event: ScrollEvent| {
        // The handler runs before the wheel moves the list, which moves by the opposite of the delta.
        let end = (scroll.content_height() - scroll.viewport_height()).max(0.0);
        let at_end = scroll.scroll_y() - event.delta_y >= end - FOLLOW_SLACK;
        if at_end != following.get_untracked() {
          if !at_end {
            left_at.set(count);
          }
          following.set(at_end);
        }
      })
      .mount_keyed::<TranscriptRowView, _, _, _>(|row| row.key.to_string(), Clone::clone)
  }

  /// "Jump to latest · N new" (`iwe3p`).
  fn jump(&self, ctx: &Ctx, rows: usize) -> Element {
    let new = rows.saturating_sub(self.left_at.get());
    let label = if new == 0 {
      ctx.t("runs.activity.jump")
    } else {
      ctx.t_args("runs.activity.jumpNew", [("count", new.to_string().as_str())])
    };
    let (scroll, following) = (self.scroll.clone(), self.following.clone());
    let button = ActionButton::new(label)
      .icon(Icon::ArrowDown)
      .id("activity-jump")
      .height(Dimension::Px(JUMP_HEIGHT))
      .on_click(move || {
        scroll.scroll_to_bottom_pending();
        following.set(true);
      })
      .render(ctx);
    Row::new()
      .width(Dimension::Pct(100.0))
      .padding_vertical(SpacingSize::Sm)
      .justify(Justify::Center)
      .child(button)
      .into()
  }

  /// Step filter, Show raw lines and the live state (`c1dmZ`).
  fn toolbar(&self, ctx: &Ctx, run: &RunView, live: bool, following: bool) -> Element {
    let raw = self.raw.clone();
    let on = raw.get();
    let state = match (live, following) {
      (true, true) => compact_status_label(Icon::Radio, &ctx.t("runs.activity.following")),
      (true, false) => compact_status_label(Icon::Pause, &ctx.t("runs.activity.notFollowing")),
      (false, _) => compact_status_label(Icon::Check, &ctx.t("runs.activity.ended")),
    };
    Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(TOOLBAR_HEIGHT))
      .spacing(ExtraSpacing::Stack.role())
      .align_items(Alignment::Center)
      .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role()))
      .child(self.filter_select(ctx, run))
      .child(Switch::new("activity-raw", ctx.t("runs.activity.showRaw"), on).render(move || raw.set(!on)))
      .child(Spacer::new().flex(1.0))
      .child(state)
      .into()
  }

  fn filter_select(&self, ctx: &Ctx, run: &RunView) -> Element {
    let executions: Vec<StepExecutionId> = run
      .run
      .attempts()
      .iter()
      .flat_map(|attempt| attempt.executions())
      .map(|execution| execution.id())
      .filter(|execution| run.invocations_of(*execution).next().is_some())
      .collect();
    let key = if executions.len() == 1 {
      "runs.activity.allSteps.one"
    } else {
      "runs.activity.allSteps.other"
    };
    let all = ctx.t_args(key, [("count", executions.len().to_string().as_str())]);
    let options = std::iter::once((StepFilter(None), all)).chain(executions.iter().filter_map(|execution| {
      let step = run.step_of(*execution)?;
      let number = format!("{:02}", step.number);
      let args = [
        ("number", number.as_str()),
        ("step", &*step.name),
        ("agent", step.agent.as_deref().unwrap_or_default()),
      ];
      // A step a failure route re-executed has an execution per attempt, which the attempt tells apart.
      let label = match run
        .run
        .attempts()
        .iter()
        .find(|attempt| attempt.execution(*execution).is_some())
      {
        Some(attempt) if run.run.attempts().len() > 1 => {
          let attempt = attempt.number().to_string();
          let [number, step, agent] = args;
          ctx.t_args(
            "runs.activity.stepOptionAttempt",
            [number, step, agent, ("attempt", attempt.as_str())],
          )
        }
        _ => ctx.t_args("runs.activity.stepOption", args),
      };
      Some((StepFilter(Some(*execution)), label))
    }));
    Element::from(
      Select::new(self.filter.clone())
        .options(options)
        .width(Dimension::Px(FILTER_WIDTH))
        .height(Dimension::Px(FILTER_HEIGHT))
        .style(select_style(ctx, FILTER_HEIGHT)),
    )
    .id("activity-step-filter")
  }

  /// Couldn't load activity: the run is unaffected; the runner log and a retry are offered.
  fn failed(&self, ctx: &mut Ctx, error: &Message) -> Element {
    tracing::debug!(?error, "the transcript could not be read");
    let runners = ctx
      .use_context::<Runners>()
      .expect("the root provides Runners before mounting screens");
    let retry = self.retry.clone();
    let actions = vec![
      ActionButton::new(ctx.t("runner.connection.openLog"))
        .icon(Icon::FileText)
        .id("activity-open-log")
        .on_click(move || runners.open_log())
        .render(ctx),
      ActionButton::new(ctx.t("runs.activity.tryAgain"))
        .icon(Icon::RefreshCw)
        .id("activity-retry")
        .on_click(move || retry.update(|attempt| *attempt += 1))
        .render(ctx),
    ];
    tab_state::state(
      "activity-failed",
      Icon::CloudOff,
      &ctx.t("runs.activity.failedTitle"),
      &ctx.t("runs.activity.failedDetail"),
      actions,
    )
  }
}

/// The CLI and model an agent runs on, as the design writes it ("Codex / Sol"); the model alone
/// once the profile's connection is gone.
pub(super) fn engine(ctx: &Ctx, provider: Option<AgentProvider>, model: &str) -> Arc<str> {
  match provider {
    Some(provider) => ctx.t_args(
      "runs.activity.engine",
      [("provider", &*ctx.t(provider_short_key(provider))), ("model", model)],
    ),
    None => model.into(),
  }
}
