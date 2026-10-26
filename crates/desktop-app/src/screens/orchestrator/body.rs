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
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{
  attachment::{NewAttachment, names_text},
  id::ChatId,
  orchestrator::{Queued, TurnState},
};
use orchester_workspace::{
  messages::Message,
  orchestrator::{chat::ChatView, queue::WhileAnswering, send::SendError, settings::Readiness},
};

use super::{
  View,
  composer::{ComposerBox, Sending, composer},
  data::{Read, Reads, Shown},
  env::env,
  runs_on::{RunsOnPicker, RunsOnProps},
  send_menu::Ways,
  start::start,
  timeline::{TurnRow, list},
  turns::{Arrival, Turn, turns},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  services::Services,
  theme::{
    DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorSpacing, OrchestratorText,
  },
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::load_error,
  },
};

/// The conversation's column (`Column` `Hoare`).
pub(super) const COLUMN_WIDTH: f32 = 760.0;

/// The chat for what `reads` found.
pub(super) fn main(ctx: &mut Ctx, view: &View, (project, reads): (&ShownProject, &Reads)) -> Element {
  let readiness = reads.setup.ready().map(|setup| setup.readiness());
  let sending = view.send.state().get();
  let thinking = matches!(&reads.shown, Shown::Chat(shown) if shown.thinking());
  let state = sending_state(ctx, view, (project, reads.target), (readiness.as_ref(), thinking));
  let problem = sending
    .error
    .as_ref()
    .map(|error| send_problem(ctx, view, project, reads.target, error));
  let unread = match &reads.setup {
    Read::Failed(error) => Some(setup_failed(ctx, project, error)),
    _ => None,
  };
  // The message being sent, by its words, or its files' names when it has none.
  let pending = sending
    .is_pending()
    .then(|| view.outgoing.get())
    .flatten()
    .map(|outgoing| {
      if outgoing.text.trim().is_empty() {
        names_text(outgoing.attachments.iter().map(NewAttachment::name))
      } else {
        outgoing.text
      }
    });
  let foot = Foot {
    state: &state,
    readiness: readiness.as_ref(),
    reads,
  };
  match &reads.shown {
    Shown::New if pending.is_none() => {
      let boxed = foot.composer(ctx, view, project, ctx.t("orchestrator.composer.placeholder"));
      start(ctx, view, (boxed, unread.or(problem)))
    }
    Shown::New => conversation(
      ctx,
      view,
      (project, None, pending.as_deref()),
      (&foot, [problem, None, unread]),
    ),
    Shown::Chat(shown) => {
      let queued = queued_problem(ctx, view);
      conversation(
        ctx,
        view,
        (project, Some(shown), pending.as_deref()),
        (&foot, [problem, queued, unread]),
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

  /// The composer, saying `placeholder`, or that it can't answer yet while the Orchestrator is not set up.
  fn composer(&self, ctx: &mut Ctx, view: &View, project: &ShownProject, placeholder: Arc<str>) -> Column {
    let stop = None;
    self.composer_with(ctx, view, (project, placeholder), stop)
  }

  fn composer_with(
    &self,
    ctx: &mut Ctx,
    view: &View,
    (project, placeholder): (&ShownProject, Arc<str>),
    stop: Option<super::send_menu::Press>,
  ) -> Column {
    let not_set_up = self.not_set_up();
    let placeholder = if not_set_up {
      ctx.t("orchestrator.composer.notSetUp")
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
    composer(
      ctx,
      view,
      ComposerBox {
        placeholder,
        sending: self.state,
        focused: &view.focused,
        stop,
        runs_on,
        set_up,
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

/// What sending does now: nothing while the Orchestrator has nothing to run on, busy while a message or its files
/// are on their way, and otherwise the draft goes to conversation `chat` (a new one when `None`) as a turn of its own,
/// or, while the Orchestrator answers (`thinking`), after the answer, now or after stopping it.
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

/// The turns of `shown` (none for a new conversation), and the message being sent (`pending`) until the conversation
/// holds it: as a turn of its own, or as it will wait while the Orchestrator answers.
fn all_turns(view: &View, shown: Option<&ChatView>, pending: Option<&str>) -> Vec<Turn> {
  let mut all = shown
    .map(|shown| turns(&shown.entries, &shown.attachments))
    .unwrap_or_default();
  let kept = view.outgoing.get().map(|outgoing| outgoing.message);
  if let Some(text) = pending
    && !all.iter().any(|turn| turn.message.is_some() && turn.message == kept)
  {
    let waits = view.how.get().map(|how| match how {
      WhileAnswering::AfterAnswer => TurnState::Queued(Queued::AfterAnswer),
      WhileAnswering::Now => TurnState::Steering,
      WhileAnswering::StopAndSend => TurnState::Queued(Queued::Next),
    });
    all.push(Turn::sending(text, waits.unwrap_or(TurnState::Thinking)));
  }
  all
}

/// The turns of `shown` (none for a new conversation) and the one being sent, over the composer's area.
fn conversation(
  ctx: &mut Ctx,
  view: &View,
  (project, shown, pending): (&ShownProject, Option<&ChatView>, Option<&str>),
  (foot, notices): (&Foot<'_>, [Option<Element>; 3]),
) -> Element {
  let all = all_turns(view, shown, pending);
  let chat = shown.map(|shown| shown.chat.id());
  let answering = shown.is_some_and(ChatView::thinking);
  // Another conversation opens at its own end.
  view
    .follow
    .shows(&chat.map_or_else(|| "new".to_owned(), |chat| chat.to_string()));
  let connection = match foot.readiness {
    Some(Readiness::Ready { connection, .. }) => Some(connection.clone()),
    _ => None,
  };
  let env = Arc::new(env(view, project, (chat, connection), &foot.reads.names));
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
  let boxed = foot.composer_with(ctx, view, (project, ctx.t("orchestrator.composer.placeholder")), stop);
  // Until it is set up, the chat says under its last turn that it stays readable (`02 Not set up` `Wd5CL`).
  let readable = foot.not_set_up().then(|| ctx.t("orchestrator.composer.readable"));
  let below = Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(COLUMN_WIDTH))
    .spacing(SpacingSize::Sm)
    .with_children(notices.into_iter().flatten())
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
            .child(list(ctx, &view.follow, rows, readable)),
        ),
    )
    .child(composer_area(below))
    .into()
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

/// A turn's room in the scrolling turns: in from the chat area's sides, the column's width at most, centred.
pub(super) fn turn_room(turn: Column) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(OrchestratorSpacing::PageSide.role())
        .right(OrchestratorSpacing::PageSide.role()),
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

/// While a conversation is read: its loader and what loads, over the faded composer.
fn loading(ctx: &mut Ctx, view: &View, project: &ShownProject, reads: &Reads) -> Element {
  let content = Column::new()
    .id("orchestrator-loading")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(Spacer::new().flex(1.0))
    .child(icon(Icon::LoaderCircle, IconSize::Lg, PaletteColor::TextSecondary))
    .child(Text::new(&ctx.t("orchestrator.loading.title")).variant(OrchestratorText::Emphasis.role()))
    .child(Text::new(&ctx.t("orchestrator.loading.note")).variant(ExtraTypography::Note.role()))
    .child(Spacer::new().flex(1.0));
  let held = Sending::Held(ctx.t("orchestrator.composer.loading"));
  let foot = Foot {
    state: &held,
    readiness: None,
    reads,
  };
  let boxed = foot
    .composer(ctx, view, project, ctx.t("orchestrator.composer.loading"))
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
