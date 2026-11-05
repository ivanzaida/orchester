//! The chat under the header (`Chat` `ECQg8` and `Composer area` `C7Eppg` in `01 Conversation` `wRSL4`): a new
//! conversation's start, or the conversation — its turns in a 760 column, centred, 40 in from the chat's sides, over
//! the composer's area (8 over the composer, 16 under it, 40 at its sides) — or why it cannot be shown. A send that
//! failed shows over the composer, and so do an *Edit* or *Remove* of a queued message that changed nothing and what
//! keeps the chat from reading what it runs on. Until the Orchestrator is set up, the composer itself says so (`02 Not
//! set up` `Wd5CL`) and the conversation stays readable. While the Orchestrator answers, the composer sends as the
//! person chooses ([`super::send_menu`]), and the messages that wait for the answer show after it.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::{Ctx, FutureState},
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ChatId;
use orchester_workspace::{
  messages::Message,
  orchestrator::{chat::ChatView, queue::WhileAnswering, send::SendError, settings::Readiness},
};

use super::{
  View,
  composer::{ComposerBox, Sending, composer},
  data::{Read, Reads, Shown},
  env::env,
  pending::{Pending, all_turns},
  runs_on::{RunsOnPicker, RunsOnProps},
  send_menu::Ways,
  start::start,
  timeline::{TurnRow, list},
  turns::{Arrival, row_of},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  services::Services,
  theme::{
    DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText,
    beside_chat_lane, chat_scrollbar,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    mention::{self, MenuProps},
    message::message_text,
    notice::{Notice, NoticeTone},
    slash::SkillReach,
    state::load_error,
    todo_list::{PinnedPlan, PinnedProps},
  },
};

/// The conversation's column (`Column` `Hoare`).
pub(super) const COLUMN_WIDTH: f32 = 760.0;

/// The chat for what `reads` found.
pub(super) fn main(ctx: &mut Ctx, view: &View, (project, reads): (&ShownProject, &Reads)) -> Element {
  let readiness = reads.setup.ready().map(|setup| setup.readiness());
  let sending = view.send.state().get();
  let thinking = matches!(&reads.shown, Shown::Chat(shown) if shown.thinking());
  let state = if matches!(&reads.shown, Shown::Chat(shown) if shown.compacting()) {
    // Nothing is sent while the session is compacted (`06b` `j7eSQO`).
    Sending::Held(ctx.t("orchestrator.context.sendWaits"))
  } else {
    sending_state(ctx, view, (project, reads.target), (readiness.as_ref(), thinking))
  };
  let problem = sending
    .error
    .as_ref()
    .map(|error| send_problem(ctx, view, project, reads.target, error));
  let unread = match &reads.setup {
    Read::Failed(error) => Some(setup_failed(ctx, project, error)),
    _ => None,
  };
  // The message being sent; a new conversation's first, sent, while that conversation is read for the first time.
  let pending = sending
    .is_pending()
    .then(|| view.outgoing.get())
    .flatten()
    .or_else(|| view.started_read(reads))
    .map(Pending::of);
  let foot = Foot {
    state: &state,
    readiness: readiness.as_ref(),
    reads,
  };
  match &reads.shown {
    Shown::New if pending.is_none() => {
      let boxed = foot.composer(ctx, view, (project, true), ctx.t("orchestrator.composer.placeholder"));
      start(ctx, view, (boxed, unread.or(problem)))
    }
    Shown::New => conversation(
      ctx,
      view,
      (project, None, pending.as_ref()),
      (&foot, [problem, None, unread]),
    ),
    Shown::Chat(shown) => {
      let queued = queued_problem(ctx, view)
        .or_else(|| super::compact::problem(ctx, view))
        .or_else(|| super::rewind::problem(ctx, view));
      conversation(
        ctx,
        view,
        (project, Some(&**shown), pending.as_ref()),
        (&foot, [problem, queued, unread]),
      )
    }
    // The conversation the first message started goes on showing it while it is read, in the same list: no bones
    // between the message sent and the message kept.
    Shown::Loading
      if pending
        .as_ref()
        .is_some_and(|pending| Some(pending.chat) == reads.target) =>
    {
      conversation(
        ctx,
        view,
        (project, None, pending.as_ref()),
        (&foot, [problem, None, unread]),
      )
    }
    Shown::Loading => loading(ctx, view, project, reads),
    Shown::Failed(error) => {
      let runners = Services::of(ctx).runners;
      let root = project.open.repo_root.clone();
      frame(load_error(
        ctx,
        "orchestrator-load-error",
        (&ctx.t("orchestrator.loadFailed"), &message_text(ctx, error)),
        move || runners.chat_changed(&root),
      ))
    }
    Shown::Missing => frame(missing(ctx, project)),
  }
}

/// What the composer is built from: what sending does, where the Orchestrator stands, and what was read.
struct Foot<'a> {
  state: &'a Sending,
  readiness: Option<&'a Readiness>,
  reads: &'a Reads,
}

impl Foot<'_> {
  /// Whether the Orchestrator is not set up yet: what it runs on was read and it cannot answer on it.
  fn not_set_up(&self) -> bool {
    self.readiness.is_some_and(|readiness| !readiness.is_ready())
  }

  /// The composer, saying `placeholder`, or that it can't answer yet while the Orchestrator is not set up; `starts` a
  /// new conversation when it says so.
  fn composer(
    &self,
    ctx: &mut Ctx,
    view: &View,
    (project, starts): (&ShownProject, bool),
    placeholder: Arc<str>,
  ) -> Column {
    self.composer_with(ctx, view, (project, placeholder), (None, starts))
  }

  fn composer_with(
    &self,
    ctx: &mut Ctx,
    view: &View,
    (project, placeholder): (&ShownProject, Arc<str>),
    (stop, starts): (Option<super::send_menu::Press>, bool),
  ) -> Column {
    let not_set_up = self.not_set_up();
    let plan = (!not_set_up).then(|| super::plan_only::composer::parts(ctx, view, &project.open.repo_root));
    let placeholder = if not_set_up {
      ctx.t("orchestrator.composer.notSetUp")
    } else if view.plan_only.on(view.of.get()) {
      ctx.t("orchestrator.plan.placeholder")
    } else {
      placeholder
    };
    let set_up = not_set_up.then(|| set_up_button(ctx, project));
    let runs_on = ctx.mount_keyed::<RunsOnPicker>(
      "orchestrator-runs-on-picker",
      RunsOnProps {
        root: project.open.repo_root.clone(),
        project: project.open.project.id(),
        setup: self.reads.setup.ready().cloned(),
        failed: matches!(self.reads.setup, Read::Failed(_)),
        open: view.runs_on.clone(),
      },
    );
    let mentions = mention::menu(
      ctx,
      MenuProps {
        id: "orchestrator-composer",
        field: view.mention.clone(),
        anchor: view.mention_anchor.clone(),
        root: project.open.repo_root.clone(),
        project: Arc::from(project.open.project.name().as_str()),
        own: None,
        run: None,
        revision: project.feed.saves.get(),
      },
    );
    let fresh = matches!(self.reads.shown, Shown::New);
    let provider = self.reads.setup.ready().and_then(|setup| setup.provider());
    let commands = super::commands::menu(ctx, view, project, (fresh, self.state.press(), provider));
    let context = super::context::meter(ctx, view, project, (self.reads, self.readiness));
    composer(
      ctx,
      view,
      ComposerBox {
        placeholder,
        sending: self.state,
        focused: &view.focused,
        stop,
        runs_on,
        context,
        set_up,
        starts,
        menus: [mentions, commands],
        plan,
        reach: SkillReach::orchestrator(provider),
      },
    )
  }
}

/// *Set up the Orchestrator*, which opens its settings.
fn set_up_button(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  ActionButton::new(
    "orchestrator-open-settings",
    ButtonKind::Primary,
    ctx.t("orchestrator.composer.setUp"),
  )
  .icon(Icon::Settings)
  .opens(navigation.to(Route::Project(
    project.open.project.id(),
    ProjectPage::OrchestratorSettings,
  )))
  .into()
}

/// What the Orchestrator runs on could not be read, so nothing can be sent: why, and *Retry*.
fn setup_failed(ctx: &mut Ctx, project: &ShownProject, error: &Message) -> Element {
  let runners = Services::of(ctx).runners;
  let root = project.open.repo_root.clone();
  let retry = ActionButton::new(
    "orchestrator-setup-retry",
    ButtonKind::Primary,
    ctx.t("orchestrator.failure.retry"),
  )
  .icon(Icon::RotateCcw)
  .on_click(move || runners.chat_changed(&root));
  Notice::new(
    "orchestrator-setup-failed",
    NoticeTone::Danger,
    ctx.t("orchestrator.setupFailed"),
  )
  .icon(Icon::CircleAlert)
  .body(message_text(ctx, error))
  .action(retry)
  .into()
}

/// An *Edit* or *Remove* of a queued message that changed nothing: why (its turn had started, or its files could not
/// be read back), until the person dismisses it or changes a queued message again.
fn queued_problem(ctx: &mut Ctx, view: &View) -> Option<Element> {
  let state = view.queued.state();
  let error = state.get().error?;
  let dismiss = ActionButton::new(
    "orchestrator-queued-dismiss",
    ButtonKind::Secondary,
    ctx.t("orchestrator.queued.dismiss"),
  )
  .on_click(move || state.set(FutureState::idle()));
  Some(
    Notice::new(
      "orchestrator-queued-problem",
      NoticeTone::Danger,
      ctx.t("orchestrator.queued.failed"),
    )
    .icon(Icon::CircleAlert)
    .body(message_text(ctx, &error))
    .action(dismiss)
    .into(),
  )
}

/// What sending does now: nothing while the Orchestrator has nothing to run on, busy while a message or its files are
/// on their way, and otherwise the draft, with the skill its chip holds, goes to conversation `chat` (a new one when
/// `None`) as a turn of its own, or, while the Orchestrator answers (`thinking`), after the answer, now or after
/// stopping it.
fn sending_state(
  ctx: &Ctx,
  view: &View,
  (project, chat): (&ShownProject, Option<ChatId>),
  (readiness, thinking): (Option<&Readiness>, bool),
) -> Sending {
  // Files still being read go with the message: it is sent once they are added.
  if view.send.state().get().is_pending() || view.attached.reading.get() > 0 {
    return Sending::Busy;
  }
  if !readiness.is_some_and(Readiness::is_ready) {
    return Sending::Held(ctx.t("orchestrator.composer.notReady"));
  }
  let way = |how| view.send_draft(project, (chat, how));
  if view.draft.get().trim().is_empty() && view.attached.attached.get().0.is_empty() {
    return Sending::Empty {
      why: ctx.t("orchestrator.composer.sendEmpty"),
      press: way(thinking.then_some(WhileAnswering::AfterAnswer)),
      answering: thinking,
    };
  }
  if !thinking {
    return Sending::Ready(way(None));
  }
  Sending::WhileAnswering(Arc::new(Ways {
    after_answer: way(Some(WhileAnswering::AfterAnswer)),
    now: way(Some(WhileAnswering::Now)),
    stop_and_send: way(Some(WhileAnswering::StopAndSend)),
  }))
}

/// The turns of `shown` (none for a new conversation) and the one being sent, over the composer's area.
fn conversation(
  ctx: &mut Ctx,
  view: &View,
  (project, shown, pending): (&ShownProject, Option<&ChatView>, Option<&Pending>),
  (foot, notices): (&Foot<'_>, [Option<Element>; 3]),
) -> Element {
  let all = all_turns(view, shown, pending);
  let chat = shown.map(|shown| shown.chat.id());
  let answering = shown.is_some_and(ChatView::thinking);
  // Another conversation opens at its own end, or at the match History's search opened it at. A new one's first
  // message names the conversation it starts, so the list follows on as that conversation is kept and read.
  let follows = chat.or_else(|| pending.map(|pending| pending.chat));
  view
    .follow
    .shows(&follows.map_or_else(|| "new".to_owned(), |chat| chat.to_string()));
  let found = (view.listing.opened_at.get())
    .filter(|at| Some(at.chat) == chat)
    .and_then(|at| row_of(&all, at.seq));
  let connection = match foot.readiness {
    Some(Readiness::Ready { connection, .. }) => Some(connection.clone()),
    _ => None,
  };
  let env = Arc::new(env(
    ctx,
    view,
    project,
    (chat, connection, answering),
    &foot.reads.names,
  ));
  let can_ask = matches!(foot.state, Sending::Ready(_) | Sending::Held(_) | Sending::Empty { .. }) && chat.is_some();
  let stopping = view.stop.state().get().is_pending();
  // The newest turn, which can be stopped or asked again: a wait that started none, kept after it, is no turn, and
  // neither is a message that waits for the answer.
  let newest = all
    .iter()
    .rposition(|turn| turn.state.is_some() && turn.arrival != Arrival::Waiting);
  let rows = all
    .into_iter()
    .enumerate()
    .map(|(index, turn)| TurnRow {
      turn: Arc::new(turn),
      last: Some(index) == newest,
      can_ask,
      stopping,
      env: env.clone(),
    })
    .collect();
  let stop = (answering && !stopping).then(|| env.stop.clone());
  let placeholder = if shown.is_some_and(ChatView::compacting) {
    ctx.t("orchestrator.context.placeholderCompacting")
  } else {
    ctx.t("orchestrator.composer.placeholder")
  };
  let boxed = foot.composer_with(ctx, view, (project, placeholder), (stop, false));
  // Until it is set up, the chat says under its last turn that it stays readable (`02 Not set up` `Wd5CL`).
  let readable = foot.not_set_up().then(|| ctx.t("orchestrator.composer.readable"));
  let plan = shown.and_then(|shown| pinned_plan(ctx, shown));
  let below = Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(COLUMN_WIDTH))
    .spacing(SpacingSize::Sm)
    .with_children(notices.into_iter().flatten())
    .with_children(plan)
    .child(boxed);
  // The turns scroll across the whole chat area, so the thumb sits at its right edge; each turn, the notices and
  // the composer keep to the column, centred.
  Column::new()
    .id("orchestrator-main")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(
      Column::new()
        .id("orchestrator-conversation")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(
          Column::new()
            .id("orchestrator-scroll")
            .width(Dimension::Pct(100.0))
            .height(Dimension::Pct(100.0))
            .child(list(ctx, &view.follow, (rows, found), readable)),
        ),
    )
    .child(composer_area(below))
    .into()
}

/// While the Orchestrator answers, the todo list its agent keeps for the answer, over the composer (08 `lEQgI`); its
/// fold is the answer's own.
fn pinned_plan(ctx: &mut Ctx, shown: &ChatView) -> Option<Element> {
  if !shown.thinking() {
    return None;
  }
  let (message, _) = shown.last_turn()?;
  let list = shown.todo_of(message)?.clone();
  let props = PinnedProps {
    id: "orchestrator-plan".into(),
    list,
    owner: ctx.t("orchestrator.plan.owner"),
  };
  Some(ctx.mount_keyed::<PinnedPlan>(&format!("orchestrator-plan-{message}"), props))
}

/// The composer's area: 8 over the composer, 16 under it, 40 at the chat's sides, the column centred.
fn composer_area(below: Column) -> Row {
  Row::new()
    .id("orchestrator-composer-area")
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .top(SpacingSize::Sm)
        .bottom(SpacingSize::Lg)
        .left(OrchestratorSpacing::PageSide.role())
        .right(OrchestratorSpacing::PageSide.role()),
    )
    .justify(Justify::Center)
    .child(below)
}

/// A turn's room in the scrolling turns: in from the chat area's sides, the column's width at most, centred on the
/// chat as the composer is: the scrollbar's lane at the right takes the place of as much of the side.
pub(super) fn turn_room(ctx: &Ctx, turn: Column) -> Row {
  let beside_lane = beside_chat_lane(&chat_scrollbar(&ctx.theme().scrollbar()));
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(OrchestratorSpacing::PageSide.role())
        .right(Dimension::Px(beside_lane)),
    )
    .justify(Justify::Center)
    .child(turn.max_width(Dimension::Px(COLUMN_WIDTH)))
}

/// The chat's room for a state of its own: in from its sides, its content centered, the composer's area under it.
fn frame(content: Element) -> Element {
  Column::new()
    .id("orchestrator-main")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(OrchestratorSpacing::PageSide.role())
        .right(OrchestratorSpacing::PageSide.role())
        .bottom(SpacingSize::Lg),
    )
    .align_items(Alignment::Center)
    .child(content)
    .into()
}

/// While a conversation is read: its turns' bones (`super::reading`), over the faded composer.
fn loading(ctx: &mut Ctx, view: &View, project: &ShownProject, reads: &Reads) -> Element {
  let content = super::reading::turns(ctx);
  let held = Sending::Held(ctx.t("orchestrator.composer.loading"));
  let foot = Foot {
    state: &held,
    readiness: None,
    reads,
  };
  let boxed = foot
    .composer(ctx, view, (project, false), ctx.t("orchestrator.composer.loading"))
    .opacity(DISABLED_OPACITY);
  Column::new()
    .id("orchestrator-main")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .child(content)
    .child(composer_area(
      Column::new()
        .width(Dimension::Pct(100.0))
        .max_width(Dimension::Px(COLUMN_WIDTH))
        .child(boxed),
    ))
    .into()
}

/// A conversation the project does not keep: why, and a new one.
fn missing(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let name = project.open.project.name().as_str();
  let new = ActionButton::new(
    "orchestrator-missing-new",
    ButtonKind::Primary,
    ctx.t("orchestrator.newConversation"),
  )
  .icon(Icon::SquarePen)
  .opens(navigation.to(Route::Project(id, ProjectPage::Orchestrator(OrchestratorChat::New))));
  Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(COLUMN_WIDTH))
    .padding_top(SpacingSize::Xl)
    .child(
      Notice::new(
        "orchestrator-missing",
        NoticeTone::Info,
        ctx.t_args("orchestrator.notFound.title", [("project", name)]),
      )
      .body(ctx.t("orchestrator.notFound.body"))
      .action(new),
    )
    .into()
}

/// A send that failed: why, that the draft is kept, and *Retry*, which sends it again once, the way it was sent.
fn send_problem(
  ctx: &mut Ctx,
  view: &View,
  project: &ShownProject,
  chat: Option<ChatId>,
  error: &SendError,
) -> Element {
  let (SendError::NotSent(why) | SendError::Unconfirmed(why)) = error;
  let retry = view.send_draft(project, (chat, view.how.get()));
  let unconfirmed = matches!(error, SendError::Unconfirmed(_));
  let mut row = Row::new()
    .id("orchestrator-send-problem")
    .describe("kind", if unconfirmed { "unconfirmed" } else { "not-sent" })
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(ExtraSpacing::Loose.role(), SpacingSize::Sm))
    .spacing(ExtraSpacing::Loose.role())
    .align_items(Alignment::Center)
    .background(PaletteColor::DangerMuted)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Danger))
    .rounded(ExtraRadius::Tile.role())
    .child(icon(Icon::CircleAlert, IconSize::Md, PaletteColor::Danger))
    .child(
      Column::new()
        .flex(1.0)
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&message_text(ctx, why)).variant(OrchestratorText::Emphasis.role()))
        .with_children((!unconfirmed).then(|| {
          Text::new(&ctx.t("orchestrator.sendProblem.body"))
            .variant(ExtraTypography::Note.role())
            .color(PaletteColor::TextSecondary)
        })),
    );
  if !unconfirmed && (!view.draft.get().trim().is_empty() || !view.attached.attached.get().0.is_empty()) {
    row = row.child(
      ActionButton::new(
        "orchestrator-send-retry",
        ButtonKind::Secondary,
        ctx.t("orchestrator.sendProblem.retry"),
      )
      .icon(Icon::RotateCcw)
      .on_click(move || retry()),
    );
  }
  row.into()
}
