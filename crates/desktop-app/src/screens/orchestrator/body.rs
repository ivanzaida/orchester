//! The chat beside History (`Main` in `ZJzV0`, `AJeWg` in `kPs8W`): a new conversation's start, or the conversation
//! — its turns over the composer, 800 wide at most and 32 in from the panel's sides, the composer 14 over the panel's
//! foot — or why it cannot be shown. A send that failed shows over the composer (`Send problem` `htsjO`), and so does
//! what keeps the chat from sending (`notReady`).

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, SpacingSize},
  },
  components::{Column, Row, Spacer, Text},
  layout::{Alignment, layout_kind::Justify},
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::id::ChatId;
use orchester_workspace::{
  messages::Message,
  orchestrator::{chat::ChatView, send::SendError, settings::Readiness, tools::ChatScope},
};

use super::{
  View,
  composer::{ComposerBox, Sending, composer, keys},
  data::{Read, Reads, Shown},
  start::start,
  timeline::{TurnEnv, TurnRow, list},
  turns::{Turn, turns},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  services::Services,
  theme::{DISABLED_OPACITY, ExtraRadius, ExtraSpacing, ExtraTypography, IconSize, OrchestratorText},
  ui::{
    button::{ActionButton, ButtonKind},
    chat::names::ProjectNames,
    icon::{Icon, icon},
    message::message_text,
    notice::{Notice, NoticeTone},
    state::load_error,
  },
};

/// The conversation's width at most (`Column` `WjWQJ`).
const COLUMN_WIDTH: f32 = 800.0;

/// The chat for what `reads` found.
pub(super) fn main(
  ctx: &mut Ctx,
  view: &View,
  (project, reads, readiness): (&ShownProject, &Reads, Option<&Readiness>),
) -> Element {
  let sending = view.send.state().get();
  let thinking = matches!(&reads.shown, Shown::Chat(shown) if shown.thinking());
  let held = held(ctx, (readiness, thinking));
  let state = if sending.is_pending() {
    Sending::Busy
  } else if let Some(why) = held {
    Sending::Held(why)
  } else if view.draft.get().trim().is_empty() {
    Sending::Empty {
      why: ctx.t("orchestrator.composer.sendEmpty"),
      press: view.send_draft(project, reads.target),
    }
  } else {
    Sending::Ready(view.send_draft(project, reads.target))
  };
  let name = project.open.project.name().as_str().to_owned();
  let problem = sending
    .error
    .as_ref()
    .map(|error| send_problem(ctx, view, project, reads.target, error));
  let not_ready = match &reads.setup {
    Read::Failed(error) => Some(setup_failed(ctx, project, error)),
    _ => readiness
      .filter(|readiness| !readiness.is_ready())
      .map(|readiness| not_ready(ctx, project, readiness)),
  };
  let pending = sending
    .is_pending()
    .then(|| view.outgoing.get())
    .flatten()
    .map(|outgoing| outgoing.text);
  match &reads.shown {
    Shown::New if pending.is_none() => {
      let boxed = ComposerBox {
        placeholder: ctx.t("orchestrator.composer.placeholderNew"),
        start: true,
        sending: &state,
        focused: &view.focused,
      };
      let column = composer(ctx, view, &boxed);
      let notice = not_ready.or(problem);
      start(ctx, view, (column, notice), &name)
    }
    Shown::New => conversation(
      ctx,
      view,
      (project, None, pending.as_deref(), &reads.names),
      (&state, readiness, [problem, not_ready]),
    ),
    Shown::Chat(shown) => conversation(
      ctx,
      view,
      (project, Some(shown), pending.as_deref(), &reads.names),
      (&state, readiness, [problem, not_ready]),
    ),
    Shown::Loading => loading(ctx, view),
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
    Shown::Missing => frame(missing(ctx, project, &name)),
  }
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

/// Why nothing can be sent now: the Orchestrator has nothing to run on, or it is still answering.
fn held(ctx: &Ctx, (readiness, thinking): (Option<&Readiness>, bool)) -> Option<Arc<str>> {
  if !readiness.is_some_and(Readiness::is_ready) {
    return Some(ctx.t("orchestrator.composer.notReady"));
  }
  thinking.then(|| ctx.t("orchestrator.composer.thinking"))
}

/// The turns of `shown` (none for a new conversation) and the one being sent, over the composer.
fn conversation(
  ctx: &mut Ctx,
  view: &View,
  (project, shown, pending, names): (&ShownProject, Option<&ChatView>, Option<&str>, &ProjectNames),
  (state, readiness, notices): (&Sending, Option<&Readiness>, [Option<Element>; 2]),
) -> Element {
  let mut all = shown.map(|shown| turns(&shown.entries)).unwrap_or_default();
  let kept = view.outgoing.get().map(|outgoing| outgoing.message);
  if let Some(text) = pending
    && !all.iter().any(|turn| turn.message.is_some() && turn.message == kept)
  {
    all.push(Turn::sending(text));
  }
  let chat = shown.map(|shown| shown.chat.id());
  let connection = match readiness {
    Some(Readiness::Ready { connection, .. }) => Some(connection.clone()),
    _ => None,
  };
  let env = Arc::new(env(view, project, (chat, connection), names));
  let can_ask = matches!(state, Sending::Ready(_) | Sending::Held(_) | Sending::Empty { .. }) && chat.is_some();
  let stopping = view.stop.state().get().is_pending();
  // The newest turn, which can be stopped or asked again: a wait that started none, kept after it, is no turn.
  let newest = all.iter().rposition(|turn| turn.state.is_some());
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
  let boxed = ComposerBox {
    placeholder: ctx.t("orchestrator.composer.placeholder"),
    start: false,
    sending: state,
    focused: &view.focused,
  };
  let foot = composer(ctx, view, &boxed).child(keys(ctx));
  // The turns scroll across the whole chat area, so the thumb sits at its right edge; each turn, the notices and
  // the composer keep to the column, centred (`ZJzV0`).
  let below = Column::new()
    .width(Dimension::Pct(100.0))
    .max_width(Dimension::Px(COLUMN_WIDTH))
    .spacing(SpacingSize::Sm)
    .with_children(notices.into_iter().flatten())
    .child(foot);
  let column = Column::new()
    .id("orchestrator-conversation")
    .width(Dimension::Pct(100.0))
    .height(Dimension::Pct(100.0))
    .align_items(Alignment::Center)
    .spacing(SpacingSize::Sm)
    .child(
      Column::new()
        .id("orchestrator-scroll")
        .width(Dimension::Pct(100.0))
        .flex(1.0)
        .min_height(Dimension::Px(0.0))
        .child(list(ctx, rows)),
    )
    .child(
      Row::new()
        .width(Dimension::Pct(100.0))
        .padding(Padding::new().left(SpacingSize::Section).right(SpacingSize::Section))
        .justify(Justify::Center)
        .child(below),
    );
  Column::new()
    .id("orchestrator-main")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .padding(Padding::new().bottom(ExtraSpacing::Section.role()))
    .child(column)
    .into()
}

/// A turn's room in the scrolling turns: 32 in from the chat area's sides, the column's width at most, centred.
pub(super) fn turn_room(turn: Column) -> Row {
  Row::new()
    .width(Dimension::Pct(100.0))
    .padding(Padding::new().left(SpacingSize::Section).right(SpacingSize::Section))
    .justify(Justify::Center)
    .child(turn.max_width(Dimension::Px(COLUMN_WIDTH)))
}

/// The chat's room: 32 in from its sides, its foot 14 over the panel's, its content centered.
fn frame(content: Element) -> Element {
  Column::new()
    .id("orchestrator-main")
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .height(Dimension::Pct(100.0))
    .padding(
      Padding::new()
        .left(SpacingSize::Section)
        .right(SpacingSize::Section)
        .bottom(ExtraSpacing::Section.role()),
    )
    .align_items(Alignment::Center)
    .child(content)
    .into()
}

/// What the turns' rows need from the screen: the conversation `chat` and the connection the Orchestrator runs on.
fn env(
  view: &View,
  project: &ShownProject,
  (chat, connection): (Option<ChatId>, Option<Arc<str>>),
  names: &ProjectNames,
) -> TurnEnv {
  let open = project.open.clone();
  let stop = view.stop.clone();
  let ask_again = match chat {
    Some(chat) => view.ask_again(project, chat),
    // A new conversation's first message is still being sent: nothing ended there to ask again.
    None => Arc::new(|_: Arc<str>| {}),
  };
  let scope = chat.map(|chat| ChatScope {
    project: open.project.id(),
    name: open.project.name().as_str().to_owned(),
    root: open.repo_root.clone(),
    chat,
  });
  TurnEnv {
    project: open.project.id(),
    name: Arc::from(open.project.name().as_str()),
    connection,
    scope,
    stop: Arc::new(move || {
      if let Some(chat) = chat {
        stop.run((open.clone(), chat));
      }
    }),
    ask_again,
    links: view.links.clone(),
    names: names.clone(),
  }
}

/// While a conversation is read: its loader and what loads, over the faded composer.
fn loading(ctx: &mut Ctx, view: &View) -> Element {
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
  let boxed = ComposerBox {
    placeholder: ctx.t("orchestrator.composer.loading"),
    start: false,
    sending: &held,
    focused: &view.focused,
  };
  let foot = composer(ctx, view, &boxed).opacity(DISABLED_OPACITY);
  frame(
    Column::new()
      .width(Dimension::Pct(100.0))
      .max_width(Dimension::Px(COLUMN_WIDTH))
      .height(Dimension::Pct(100.0))
      .child(content)
      .child(foot)
      .into(),
  )
}

/// A conversation the project does not keep: why, and a new one.
fn missing(ctx: &mut Ctx, project: &ShownProject, name: &str) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let new = ActionButton::new(
    "orchestrator-missing-new",
    ButtonKind::Primary,
    ctx.t("orchestrator.newConversation"),
  )
  .icon(Icon::MessageSquarePlus)
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

/// What keeps the chat from sending, and where to fix it.
fn not_ready(ctx: &mut Ctx, project: &ShownProject, readiness: &Readiness) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let body = match readiness {
    Readiness::Unusable(reason) => message_text(ctx, reason),
    Readiness::ConnectionGone => ctx.t("orchestrator.failure.connectionGone"),
    Readiness::NotChosen | Readiness::Ready { .. } => ctx.t("orchestrator.notReady.body"),
  };
  let open = ActionButton::new(
    "orchestrator-open-settings",
    ButtonKind::Primary,
    ctx.t("orchestrator.notReady.openSettings"),
  )
  .icon(Icon::Settings)
  .opens(navigation.to(Route::Project(id, ProjectPage::OrchestratorSettings)));
  Notice::new(
    "orchestrator-not-ready",
    NoticeTone::Warning,
    ctx.t("orchestrator.notReady.title"),
  )
  .body(body)
  .action(open)
  .into()
}

/// A send that failed: why, that the draft is kept, and *Retry*, which sends it again once.
fn send_problem(
  ctx: &mut Ctx,
  view: &View,
  project: &ShownProject,
  chat: Option<ChatId>,
  error: &SendError,
) -> Element {
  let (SendError::NotSent(why) | SendError::Unconfirmed(why)) = error;
  let retry = view.send_draft(project, chat);
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
  if !unconfirmed && !view.draft.get().trim().is_empty() {
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
