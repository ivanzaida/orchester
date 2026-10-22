//! The project's Orchestrator chat (`Orchestrator` `TrG83`; `docs/design/orchester-workspace.md`, "Project
//! orchestrator conversation") and its settings (`Orchestrator settings` `Xir64`, [`settings`]). The chat talks with
//! an agent CLI the project's runner starts read-only in the project folder, which acts through Orchester's domain
//! tools with the same checks as the app (`orchester_workspace::orchestrator`). The screen shows the conversation the
//! route names, the latest one by default, or a new one (`start`); its header (`header`), the turns (`timeline`),
//! the composer (`composer`) and History beside them (`history`). Each conversation's draft is kept for as long as
//! the app runs (`draft`). Sending keeps the draft until its message is kept, and a failed send keeps it with
//! *Retry*, which sends the same message once.

mod action;
mod body;
pub(in crate::screens) mod card;
pub(in crate::screens) mod change;
mod composer;
pub(in crate::screens) mod data;
mod draft;
mod failure;
mod header;
mod held;
mod history;
mod page;
mod proposal;
mod report;
mod run_end;
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
  id::{ChatId, ChatMessageId, ProjectId},
  time::Timestamp,
};
use orchester_workspace::{display, orchestrator::send::Outgoing};

use self::{
  data::Shown,
  draft::Conversation,
  header::{Head, Named},
  send::{SendAction, Sending, Sent, StopAction},
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  ui::{
    chat::{self, follow::Follow},
    markdown::LinkHandler,
  },
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
  /// Whose draft `draft` holds; `None` until the screen knows which conversation it shows.
  of: Signal<Option<Conversation>>,
  /// The draft's message this screen sends, shown in the conversation until it is kept.
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
  /// The turns open at their end and follow it; another conversation opens at its own end.
  follow: Follow,
}

struct OrchestratorScreen {
  view: View,
}

impl Component for OrchestratorScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let view = View {
      draft: ctx.signal(String::new()),
      of: ctx.signal(None),
      outgoing: ctx.signal(None),
      history: ctx.signal(false),
      search: ctx.signal(String::new()),
      focus: ctx.signal(false),
      focused: ctx.signal(false),
      input: ElementRef::new(),
      send: send::send_action(ctx),
      stop: send::stop_action(ctx),
      links: chat::links(ctx),
      follow: Follow::new(ctx),
    };
    let project = ctx.props::<Props>().project.open.project.id();
    let of = view.of.clone();
    ctx.watch(&view.draft, move |text| {
      if let Some(of) = of.get_untracked() {
        draft::set(project, of, text);
      }
    });
    let navigation = Navigation::of(ctx);
    let done = view.clone();
    ctx.watch(&view.send.state(), move |state| {
      if !state.is_fulfilled() {
        return;
      }
      let Some(sent) = state.data else {
        return;
      };
      let route = navigation.route_now();
      let on = |chat| route == Route::Project(project, ProjectPage::Orchestrator(chat));
      // A new conversation is kept now: the route names it, so History and a reload find it.
      if done.sent(project, sent, on(OrchestratorChat::New) || on(OrchestratorChat::Latest)) {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Orchestrator(OrchestratorChat::Chat(sent.chat)),
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
    // Until History says which conversation is the latest, the screen shows none.
    if reads.target.is_some() || reads.shown != Shown::Loading {
      view.hold_draft_of(props.project.open.project.id(), reads.target.into());
    }
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
  /// The composer holds the draft of `of` in `project`, as it was when that conversation was last shown.
  fn hold_draft_of(&self, project: ProjectId, of: Conversation) {
    if self.of.get_untracked() != Some(of) {
      self.load_draft(project, of);
    }
  }

  /// Words typed before the screen knew which conversation it shows become that one's draft when it has none.
  fn load_draft(&self, project: ProjectId, of: Conversation) {
    let typed = self.of.get_untracked().is_none().then(|| self.draft.get_untracked());
    self.of.set(Some(of));
    let mut kept = draft::draft(project, of);
    if let Some(typed) = typed.filter(|typed| !typed.is_empty())
      && kept.is_empty()
    {
      draft::set(project, of, &typed);
      kept = typed;
    }
    if self.draft.get_untracked() != kept {
      self.draft.set(kept);
    }
  }

  /// `sent` was kept. The draft it was sent from empties while it still holds the words sent: what the person wrote
  /// since stays, and goes with the conversation when it was a new one's first message and the route still says a
  /// new or the latest conversation (`fresh`). Whether it started a conversation so.
  fn sent(&self, project: ProjectId, sent: Sent, fresh: bool) -> bool {
    if self
      .outgoing
      .get_untracked()
      .is_some_and(|outgoing| outgoing.message == sent.message)
    {
      self.outgoing.set(None);
    }
    if !sent.from_draft {
      return false;
    }
    let Some(from) = draft::sent(project, (sent.chat, sent.message)) else {
      return false;
    };
    let started = fresh && from == Conversation::New;
    let of = if started {
      draft::started(project, sent.chat);
      Conversation::Chat(sent.chat)
    } else {
      from
    };
    if self.of.get_untracked() == Some(from) {
      self.load_draft(project, of);
    }
    started
  }

  /// Sends the draft, as the message it was last sent as when its words did not change since, to conversation
  /// `chat` (a new one when `None`).
  /// The chat goes to its end and follows it again (`ui::chat::follow`).
  fn send_draft(&self, project: &ShownProject, chat: Option<ChatId>) -> Arc<dyn Fn() + Send + Sync> {
    let (view, project) = (self.clone(), project.open.clone());
    let from = Conversation::from(chat);
    Arc::new(move || {
      let text = view.draft.get_untracked();
      if text.trim().is_empty() || view.send.state().get_untracked().is_pending() {
        return;
      }
      let to = chat.unwrap_or_else(ChatId::generate);
      let outgoing = draft::sending(project.project.id(), from, (to, text));
      view.outgoing.set(Some(outgoing.clone()));
      view.follow.follow_again();
      view.send.run(Sending {
        project: project.clone(),
        outgoing,
        from_draft: true,
      });
    })
  }

  /// Sends words once more to conversation `chat`, as a new message; the draft stays as it is. The chat goes to its
  /// end and follows it again.
  fn ask_again(&self, project: &ShownProject, chat: ChatId) -> Arc<dyn Fn(Arc<str>) + Send + Sync> {
    let (send, follow, project) = (self.send.clone(), self.follow.clone(), project.open.clone());
    Arc::new(move |text: Arc<str>| {
      if send.state().get_untracked().is_pending() {
        return;
      }
      follow.follow_again();
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
