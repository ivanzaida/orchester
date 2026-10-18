//! The project's Orchestrator chat (`Orchestrator` `TrG83`; `docs/design/orchester-workspace.md`, "Project
//! orchestrator conversation") and its settings (`Orchestrator settings` `Xir64`, [`settings`]). The chat talks with
//! an agent CLI the project's runner starts read-only in the project folder, which acts through Orchester's domain
//! tools with the same checks as the app (`orchester_workspace::orchestrator`). The screen shows the conversation the
//! route names, the latest one by default, or a new one (`start`); its header (`header`), the turns (`timeline`),
//! the composer (`composer`) and History beside them (`history`). Sending keeps the draft until its message is kept,
//! and a failed send keeps it with *Retry*, which sends the same message once.

mod action;
mod body;
pub(in crate::screens) mod card;
mod change;
mod composer;
pub(in crate::screens) mod data;
mod header;
mod held;
mod history;
mod proposal;
mod send;
mod settings;
mod start;
mod timeline;
mod turns;
mod wait;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::KeyboardEvent,
  },
  components::{Column, Row},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ChatId, ChatMessageId},
  time::Timestamp,
};
use orchester_workspace::{display, orchestrator::send::Outgoing};

use self::{
  data::Shown,
  header::{Head, Named},
  send::{SendAction, Sending, Sent, StopAction},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  ui::{chat, markdown::LinkHandler},
};

pub fn screen(ctx: &mut Ctx, project: &ShownProject, chat: OrchestratorChat) -> Element {
  ctx.mount_keyed::<OrchestratorScreen>(
    &format!("orchestrator-{}", project.open.project.id()),
    Props {
      project: project.clone(),
      chat,
    },
  )
}

pub fn settings(ctx: &mut Ctx, project: &ShownProject) -> Element {
  settings::screen(ctx, project)
}

#[derive(Clone, PartialEq)]
struct Props {
  project: ShownProject,
  chat: OrchestratorChat,
}

impl DevtoolsInspectable for Props {}

/// What the screen keeps while it is shown.
#[derive(Clone)]
struct View {
  draft: Signal<String>,
  /// The message the draft was last sent as, so sending it again keeps it once.
  outgoing: Signal<Option<Outgoing>>,
  history: Signal<bool>,
  search: Signal<String>,
  /// Set to put the focus in the message field.
  focus: Signal<bool>,
  /// The message field has the focus.
  focused: Signal<bool>,
  input: ElementRef,
  send: SendAction,
  stop: StopAction,
  /// What a press on a link in a reply does; made once, so a reply is laid out again only when it changes.
  links: LinkHandler,
}

struct OrchestratorScreen {
  view: View,
}

impl Component for OrchestratorScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let view = View {
      draft: ctx.signal(String::new()),
      outgoing: ctx.signal(None),
      history: ctx.signal(false),
      search: ctx.signal(String::new()),
      focus: ctx.signal(false),
      focused: ctx.signal(false),
      input: ElementRef::new(),
      send: send::send_action(ctx),
      stop: send::stop_action(ctx),
      links: chat::links(ctx),
    };
    let project = ctx.props::<Props>().project.open.project.id();
    let navigation = Navigation::of(ctx);
    let (draft, outgoing) = (view.draft.clone(), view.outgoing.clone());
    ctx.watch(&view.send.state(), move |state| {
      if !state.is_fulfilled() {
        return;
      }
      let Some(Sent {
        chat,
        message,
        from_draft,
      }) = state.data
      else {
        return;
      };
      // The draft is done with only while it still is the message sent: what the person wrote since stays.
      let sent = outgoing.get_untracked();
      let unchanged = sent.is_some_and(|sent| sent.message == message && sent.text == draft.get_untracked());
      if from_draft && unchanged {
        draft.set(String::new());
        outgoing.set(None);
      }
      // A new conversation is kept now: the route names it, so History and a reload find it.
      let new = Route::Project(project, ProjectPage::Orchestrator(OrchestratorChat::New));
      if navigation.route_now() == new {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Orchestrator(OrchestratorChat::Chat(chat)),
        ));
      }
    });
    Self { view }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let view = &self.view;
    if view.focus.get() {
      ctx.focus(&view.input);
      view.focus.set(false);
    }
    let reads = data::read(ctx, &props.project.open.repo_root, props.chat);
    let readiness = reads.setup.ready().map(|setup| setup.readiness());
    let chat = match &reads.shown {
      Shown::Chat(shown) => Named::Chat(&shown.chat),
      Shown::Loading => Named::Opening(reads.target.and_then(|target| {
        let chats = &reads.history.ready()?.chats;
        let listing = chats.iter().find(|listing| listing.chat.id() == target)?;
        Some(listing.chat.title())
      })),
      Shown::New | Shown::Failed(_) | Shown::Missing => Named::New,
    };
    let head = header::header(
      ctx,
      view,
      &Head {
        project: &props.project,
        chat,
        readiness: readiness.as_ref(),
      },
    );
    let main = body::main(ctx, view, (&props.project, &reads, readiness.as_ref()));
    let panel = view.history.get().then(|| {
      let open = &props.project.open.project;
      let chats = reads
        .history
        .ready()
        .map_or(&[][..], |history| history.chats.as_slice());
      history::panel(
        ctx,
        (&view.history, &view.search),
        (open.id(), open.name().as_str()),
        (chats, reads.target),
      )
    });
    let history = view.history.clone();
    Column::new()
      .id("screen-orchestrator")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .on_key_down(move |event: KeyboardEvent| {
        if event.key == "Escape" && history.get_untracked() {
          history.set(false);
          event.prevent_default();
        }
      })
      .child(head)
      .child(
        Row::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .child(main)
          .with_children(panel),
      )
  }
}

impl View {
  /// Sends the draft, as the message it was last sent as when its words did not change since, to conversation
  /// `chat` (a new one when `None`).
  fn send_draft(&self, project: &ShownProject, chat: Option<ChatId>) -> Arc<dyn Fn() + Send + Sync> {
    let (view, project) = (self.clone(), project.open.clone());
    Arc::new(move || {
      let text = view.draft.get_untracked();
      if text.trim().is_empty() || view.send.state().get_untracked().is_pending() {
        return;
      }
      let outgoing = match view.outgoing.get_untracked() {
        Some(kept) if kept.text == text && chat.is_none_or(|chat| kept.chat == chat) => kept,
        _ => Outgoing {
          chat: chat.unwrap_or_else(ChatId::generate),
          message: ChatMessageId::generate(),
          text,
        },
      };
      view.outgoing.set(Some(outgoing.clone()));
      view.send.run(Sending {
        project: project.clone(),
        outgoing,
        from_draft: true,
      });
    })
  }

  /// Sends words once more to conversation `chat`, as a new message; the draft stays as it is.
  fn ask_again(&self, project: &ShownProject, chat: ChatId) -> Arc<dyn Fn(Arc<str>) + Send + Sync> {
    let (send, project) = (self.send.clone(), project.open.clone());
    Arc::new(move |text: Arc<str>| {
      if send.state().get_untracked().is_pending() {
        return;
      }
      send.run(Sending {
        project: project.clone(),
        outgoing: Outgoing {
          chat,
          message: ChatMessageId::generate(),
          text: text.to_string(),
        },
        from_draft: false,
      });
    })
  }
}

/// The local date of `at` as History and the header say it (*Oct 8*).
pub(super) fn date(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  let (month, day) = display::month_day(at);
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  ctx.t_args(
    "orchestrator.date",
    [("month", month.to_string()), ("day", day.to_string())],
  )
}
