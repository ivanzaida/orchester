//! The project's Orchestrator chat (`Orchestrator · redesign` `hujxu` in the new orchester.pen;
//! `docs/design/orchester-workspace.md`, "Project orchestrator conversation") and its settings (`Orchestrator
//! settings` `Xir64`, [`settings`]). The chat talks with an agent CLI the project's runner starts read-only in the
//! project folder, which acts through Orchester's domain tools with the same checks as the app
//! (`orchester_workspace::orchestrator`). The screen shows the conversation the route names, the latest one by
//! default, or a new one (`start`); its one-line header (`header`), the turns (`timeline`) in a 760 column, the
//! composer with what the Orchestrator runs on (`composer`, `runs_on`), and History over the chat's right edge
//! (`history`). Each conversation's draft is kept for as long as the app runs (`draft`). Sending keeps the draft until
//! its message is kept, and a failed send keeps it with *Retry*, which sends the same message once.

mod action;
mod answer;
mod body;
pub(in crate::screens) mod card;
pub(in crate::screens) mod change;
mod commands;
mod compact;
mod compaction;
mod composer;
mod context;
pub(in crate::screens) mod data;
mod decision;
mod delivery;
mod digest;
mod draft;
mod ending;
mod env;
mod export;
mod failure;
mod header;
mod held;
mod history;
mod line;
mod page;
mod plan_only;
mod proposal;
mod reading;
mod report;
mod rewind;
mod run_end;
mod runs_on;
mod send;
mod send_menu;
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
    events::{KeyboardEvent, is_synthetic_input},
  },
  components::{Column, Stack},
  core::{ElementRef, Signal},
  layout::StackAlignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::{ChatId, ChatMessageId, ProjectId},
  run::Answerer,
  time::Timestamp,
};
use orchester_workspace::{
  display,
  orchestrator::{
    chat::History,
    queue::{TakenBack, WhileAnswering},
    send::Outgoing,
  },
  skills::offered::OfferedSkill,
};

use self::{
  data::Shown,
  draft::Conversation,
  header::Named,
  send::{QueuedAction, SendAction, Sending, Sent, StopAction},
  timeline::OpenedPlans,
};
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{OrchestratorChat, ProjectPage, Route},
  ui::{
    attachments::draft::DraftAttachments,
    chat::{self, follow::Follow},
    markdown::LinkHandler,
    mention::MentionField,
    slash::SlashField,
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
  /// The files the draft holds, and why the last ones given were not taken.
  attached: DraftAttachments,
  /// Whose draft `draft` holds; `None` until the screen knows which conversation it shows.
  of: Signal<Option<Conversation>>,
  /// The draft's message this screen sends, shown in the conversation until it is kept, and how it goes while the
  /// Orchestrator answers.
  outgoing: Signal<Option<Outgoing>>,
  how: Signal<Option<WhileAnswering>>,
  history: Signal<bool>,
  /// What History keeps while it shows: Esc ends the delete it asks about, then cancels a title edited in place,
  /// before it closes History.
  listing: history::HistoryState,
  /// The composer's runs-on picker is open.
  runs_on: Signal<bool>,
  /// Set to put the focus in the message field.
  focus: Signal<bool>,
  /// The message field has the focus.
  focused: Signal<bool>,
  /// The message field, the field's @ menu and its mentions (`ui::mention`), its slash menu (`ui::slash`) and the
  /// skill its chip holds, and the box the menus open over.
  input: ElementRef,
  mention: MentionField,
  slash: SlashField,
  skill: Signal<Option<OfferedSkill>>,
  mention_anchor: ElementRef,
  /// The send menu over the send button is open while the Orchestrator answers, and its chevron.
  menu: Signal<bool>,
  menu_anchor: ElementRef,
  send: SendAction,
  stop: StopAction,
  /// Compacts the conversation's session (ORC-309).
  compact: compact::CompactAction,
  /// Takes a queued message's words back into the composer, or removes it.
  queued: QueuedAction,
  /// What a press on a link in a reply does; made once, so a reply is laid out again only when it changes.
  links: LinkHandler,
  /// The turns open at their end and follow it; another conversation opens at its own end.
  follow: Follow,
  /// The answers whose todo list the person opened under their name line.
  plans: Signal<OpenedPlans>,
  /// The *Plan only* switch and the plan being changed (ORC-646).
  plan_only: plan_only::PlanOnly,
  /// Edit and *Rewind to here* on an earlier message (ORC-333).
  rewind: rewind::Rewinding,
  /// The header's … with *Copy conversation as Markdown* and *Export conversation…* (ORC-327).
  export: export::ChatExport,
}

struct OrchestratorScreen {
  view: View,
}

impl Component for OrchestratorScreen {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let (draft, focused) = (ctx.signal(String::new()), ctx.signal(false));
    let skill = ctx.signal(None);
    let mention = MentionField::new(ctx, draft.clone(), focused.clone());
    let slash = SlashField::new(ctx, draft.clone(), (focused.clone(), skill.clone()));
    let view = View {
      draft,
      attached: DraftAttachments::new(ctx, Vec::new()),
      of: ctx.signal(None),
      outgoing: ctx.signal(None),
      how: ctx.signal(None),
      history: ctx.signal(false),
      listing: history::HistoryState::new(ctx),
      runs_on: ctx.signal(false),
      focus: ctx.signal(false),
      focused,
      input: mention.element(),
      mention,
      slash,
      skill,
      mention_anchor: ElementRef::new(),
      menu: ctx.signal(false),
      menu_anchor: ElementRef::new(),
      send: send::send_action(ctx),
      stop: send::stop_action(ctx),
      compact: compact::compact_action(ctx),
      queued: send::queued_action(ctx),
      links: chat::links(ctx),
      follow: Follow::new(ctx),
      plans: ctx.signal(OpenedPlans::default()),
      plan_only: plan_only::PlanOnly::new(ctx),
      rewind: rewind::Rewinding::new(ctx),
      export: export::ChatExport::new(ctx),
    };
    rewind::watch(ctx, &view);
    let taker = view.clone();
    ctx.watch(&view.queued.state(), move |state| {
      if let Some(taken) = state
        .data
        .as_ref()
        .filter(|_| state.is_fulfilled())
        .and_then(|changed| changed.taken.as_ref())
      {
        taker.take_back(taken);
      }
    });
    let project = ctx.props::<Props>().project.open.project.id();
    let of = view.of.clone();
    ctx.watch(&view.draft, move |text| {
      if let Some(of) = of.get_untracked() {
        draft::set(project, of, text);
      }
    });
    let of = view.of.clone();
    ctx.watch(&view.attached.attached, move |held| {
      if let Some(of) = of.get_untracked() {
        draft::set_attachments(project, of, &held.0);
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
    // A result of History's search opens its conversation at the match, the one shown too.
    let follow = view.follow.clone();
    ctx.watch(&view.listing.opened_at, move |_| follow.open_anew());
    Self { view }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let view = &self.view;
    if view.focus.get() {
      ctx.focus(&view.input);
      view.focus.set(false);
    }
    let saves = props.project.feed.saves.get();
    let reads = data::read(ctx, (&props.project.open.repo_root, saves), props.chat);
    // Until History says which conversation is the latest, the screen shows none.
    if reads.target.is_some() || reads.shown != Shown::Loading {
      view.hold_draft_of(props.project.open.project.id(), reads.target.into());
    }
    view
      .plan_only
      .hold(matches!(&reads.shown, Shown::Chat(shown) if shown.chat.plan_only()));
    view.rewind.hold(match &reads.shown {
      Shown::Chat(shown) => Some(&**shown),
      _ => None,
    });
    let chat = match &reads.shown {
      Shown::Chat(shown) => Named::Chat(&shown.chat, rewind::forked::is_fork(&shown.entries)),
      Shown::Loading => Named::Opening(reads.target.and_then(|target| {
        let chats = &reads.history.ready()?.chats;
        let listing = chats.iter().find(|listing| listing.chat.id() == target)?;
        Some(listing.chat.title())
      })),
      Shown::New | Shown::Failed(_) | Shown::Missing => Named::New,
    };
    // A conversation that is kept can be copied and exported.
    let kept = match &reads.shown {
      Shown::Chat(shown) => Some(&**shown),
      _ => None,
    };
    let more = kept.map(|shown| view.export.more(ctx, (&props.project, shown)));
    let head = header::header(ctx, view, (&props.project, chat), more);
    let main = body::main(ctx, view, (&props.project, &reads));
    // History opens over the chat's right edge, under the header.
    let panel = view.history.get().then(|| {
      // While the conversations are read the list is bones; a failed reading lists none.
      let none = History::default();
      let read = match &reads.history {
        data::Read::Loading => None,
        data::Read::Failed(_) => Some(&none),
        data::Read::Ready(history) => Some(history),
      };
      let rows = history::Rows::new(
        props.project.open.project.id(),
        &props.project.open.repo_root,
        &view.listing,
      );
      history::panel(ctx, &view.listing, &rows, (read, reads.target))
    });
    // The question before going back, and the export, are modals over the window.
    let asked = rewind::dialog(ctx, view, &props.project);
    let exported = kept.and_then(|shown| view.export.dialog(ctx, (&props.project, shown)));
    Column::new()
      .id("screen-orchestrator")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .on_key_down(escape(view))
      .child(head)
      .child(
        Stack::new()
          .width(Dimension::Pct(100.0))
          .flex(1.0)
          .min_height(Dimension::Px(0.0))
          .stack_align(StackAlignment::TopEnd)
          .child(main)
          .with_children(panel),
      )
      .with_children(asked)
      .with_children(exported)
  }
}

/// Esc while History shows: it first ends the delete History asks about, then cancels a title edited in place, and
/// only then closes History, which opens on its list again.
fn escape(view: &View) -> impl Fn(KeyboardEvent) + Send + Sync + 'static {
  let (history, listing) = (view.history.clone(), view.listing.clone());
  move |event: KeyboardEvent| {
    if event.key != "Escape" || !history.get_untracked() {
      return;
    }
    if listing.deleting.get_untracked().0.is_some() {
      listing.deleting.set(history::Deleting::default());
    } else if listing.renaming.get_untracked().0.is_some() {
      listing.renaming.set(history::Renaming::default());
    } else {
      history.set(false);
      if listing.archived.get_untracked() {
        listing.archived.set(false);
      }
    }
    event.prevent_default();
  }
}

impl View {
  /// The composer holds the draft of `of` in `project`, as it was when that conversation was last shown.
  fn hold_draft_of(&self, project: ProjectId, of: Conversation) {
    if self.of.get_untracked() != Some(of) {
      self.load_draft(project, of);
    }
  }

  /// Words typed and files attached before the screen knew which conversation it shows become that one's draft when
  /// it has none. A new conversation's composer takes the focus. The runs-on picker closes: its room was measured over
  /// the chip where the other conversation had it.
  fn load_draft(&self, project: ProjectId, of: Conversation) {
    let unknown = self.of.get_untracked().is_none();
    let typed = unknown.then(|| self.draft.get_untracked());
    let given = unknown.then(|| self.attached.held());
    self.of.set(Some(of));
    self.plan_only.stop_changing();
    if self.runs_on.get_untracked() {
      self.runs_on.set(false);
    }
    if of == Conversation::New {
      self.focus.set(true);
    }
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
    let mut files = draft::attachments(project, of);
    if let Some(given) = given.filter(|given| !given.is_empty())
      && files.is_empty()
    {
      draft::set_attachments(project, of, &given);
      files = given;
    }
    self.attached.replace(files);
  }

  /// `sent` was kept. The draft it was sent from empties while it still holds the words sent: what the person wrote
  /// since stays, and goes with the conversation when it was a new one's first message and the route still says a
  /// new or the latest conversation (`fresh`). The chip empties while it still holds the skill sent. Whether it started
  /// a conversation so.
  fn sent(&self, project: ProjectId, sent: Sent, fresh: bool) -> bool {
    let outgoing = self
      .outgoing
      .get_untracked()
      .filter(|outgoing| outgoing.message == sent.message);
    if outgoing.is_some() {
      self.outgoing.set(None);
    }
    if !sent.from_draft {
      return false;
    }
    // The change of a plan went with it, and so did the skill.
    self.plan_only.stop_changing();
    let skill = outgoing.and_then(|outgoing| outgoing.skill);
    if skill.is_some() && self.skill.get_untracked().and_then(|held| held.chosen()) == skill {
      self.skill.set(None);
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

  /// Sends the draft with its files, as the message it was last sent as when neither changed since, to conversation
  /// `chat` (a new one when `None`), as a turn of its own or, while the Orchestrator answers, `how` the person chose;
  /// nothing while files are still being read. The send menu closes, and the chat goes to its end and follows it again
  /// (`ui::chat::follow`).
  fn send_draft(
    &self,
    project: &ShownProject,
    (chat, how): (Option<ChatId>, Option<WhileAnswering>),
  ) -> Arc<dyn Fn() + Send + Sync> {
    let (view, project) = (self.clone(), project.open.clone());
    let from = Conversation::from(chat);
    Arc::new(move || {
      let text = view.draft.get_untracked();
      let attachments = view.attached.held();
      let reading = view.attached.reading.get_untracked() > 0;
      if (text.trim().is_empty() && attachments.is_empty()) || reading || view.send.state().get_untracked().is_pending()
      {
        return;
      }
      view.menu.set(false);
      let to = chat.unwrap_or_else(ChatId::generate);
      let on = view.plan_only.on_now(Some(from));
      let sent_as = (sender(), view.plan_only.mode(on), on);
      let skill = view.skill.get_untracked().and_then(|skill| skill.chosen());
      let outgoing = draft::sending(project.project.id(), from, (to, text, attachments, skill), sent_as);
      view.outgoing.set(Some(outgoing.clone()));
      view.how.set(how);
      view.follow.follow_again();
      view.send.run(Sending {
        project: project.clone(),
        outgoing,
        from_draft: true,
        how,
      });
    })
  }

  /// Sends words once more to conversation `chat`, as a new message without files; the draft stays as it is. The
  /// chat goes to its end and follows it again.
  fn ask_again(&self, project: &ShownProject, chat: ChatId) -> Arc<dyn Fn(Arc<str>) + Send + Sync> {
    let (view, project) = (self.clone(), project.open.clone());
    Arc::new(move |text: Arc<str>| {
      if view.send.state().get_untracked().is_pending() {
        return;
      }
      view.how.set(None);
      view.follow.follow_again();
      let on = view.plan_only.on_now(Some(Conversation::Chat(chat)));
      view.send.run(Sending {
        project: project.clone(),
        outgoing: Outgoing {
          chat,
          message: ChatMessageId::generate(),
          text: text.to_string(),
          attachments: Vec::new(),
          skill: None,
          sent_by: sender(),
          mode: view.plan_only.mode(on),
          plan_only: on,
        },
        from_draft: false,
        how: None,
      });
    })
  }

  /// *Change it* on a plan: the composer holds the plan as a chip, and the focus goes there; what is sent next plans.
  fn change_plan(&self) -> Arc<dyn Fn(plan_only::Changing) + Send + Sync> {
    let view = self.clone();
    Arc::new(move |changing| {
      view.plan_only.changing.set(Some(changing));
      view.focus.set(true);
    })
  }

  /// A queued message taken back: its words go into the composer after what is written there, its files after the
  /// draft's, its skill into the chip, with the focus there. The facts its mentions carried are read again when it is
  /// sent again.
  fn take_back(&self, taken: &TakenBack) {
    if let Some(skill) = OfferedSkill::carried_by(&taken.words) {
      self.skill.set(Some(skill));
    }
    let words = orchester_workspace::mentions::facts::said(&taken.words);
    let draft = self.draft.get_untracked();
    let text = if draft.trim().is_empty() {
      words.to_owned()
    } else if words.trim().is_empty() {
      draft
    } else {
      format!("{}\n\n{words}", draft.trim_end())
    };
    if self.draft.get_untracked() != text {
      self.draft.set(text);
    }
    self.attached.take_back(taken.attachments.clone());
    self.focus.set(true);
  }
}

/// Who sends the message the press under way sends: automation driving the window (lurq's synthetic input), whose
/// message never approves a proposal for the person, or the person.
fn sender() -> Answerer {
  if is_synthetic_input() {
    Answerer::Automation
  } else {
    Answerer::Person
  }
}

/// The local date of `at` as History and the header say it (*Oct 8*).
pub(super) fn date(ctx: &Ctx, at: Timestamp) -> Arc<str> {
  month_day(ctx, display::month_day(at))
}

/// The calendar day `day` as History and the header say a date (*Oct 8*).
pub(super) fn day(ctx: &Ctx, day: chrono::NaiveDate) -> Arc<str> {
  use chrono::Datelike;
  month_day(ctx, (day.month0() as usize, day.day()))
}

/// The `month` (a [`display::MONTH_KEYS`] index) and `day` of a date, as *Oct 8*.
fn month_day(ctx: &Ctx, (month, day): (usize, u32)) -> Arc<str> {
  let month = ctx.t_ns(orchester_workspace::i18n::NAMESPACE, display::MONTH_KEYS[month]);
  ctx.t_args(
    "orchestrator.date",
    [("month", month.to_string()), ("day", day.to_string())],
  )
}
