//! The card of a publication, cancel, merge, answer, message, continuation, pause, resume, release, repository, git
//! work or MCP server change the Orchestrator's agent proposed or the chat did itself, and of a run it started without
//! asking because auto mode is on. While it waits for the person it is a decision card (`3 · Decisions` `bxJZp`,
//! [`super::decision`]): its question, what it is about, its facts (the first three, the rest behind *Show details*)
//! and, while it waits, the diff of the text it publishes, its git commands or the `.gitignore` a first commit
//! honours; a warning strip for what is risky in it; and its actions. Only a person applies it: lurq's synthetic input
//! is refused in the handler, visibly; *Dismiss* changes nothing and stays open to automation. Decided, being carried
//! out, done by the chat itself or refused, it folds into its event line ([`folded`]). The facts are read when the card
//! shows ([`orchester_workspace::orchestrator::facts`]); a repository's are the ones its proposal keeps.

pub(in crate::screens) mod answer;
mod continuation;
mod control;
mod facts;
mod folded;
mod git;
mod mcp;
mod opens;
mod reading;
mod repository;
mod widens;
mod words;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, Timeout},
    events::is_synthetic_input,
  },
  components::Row,
  core::Signal,
  node::Element,
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{ChatAction, change::Change},
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  orchestrator::{facts as read, proposal::Decision, tools::ChatScope},
  workspace::OrchestratorConsent,
};

use self::{
  facts::Fact,
  words::{Accept, Phase, View, Words},
};
use super::{
  card::{self, DecideAction, Deciding},
  decision,
};
use crate::{
  navigation::Navigation,
  services::Services,
  theme::SKELETON_DELAY,
  ui::{
    button::ActionButton,
    icon::Icon,
    message::message_text,
    tinted_note::{person_only, tinted_note},
    tooltip::Tooltip,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct ChangeProps {
  pub(super) id: String,
  /// The conversation's scope; without it nothing is read and no decision is taken.
  pub(super) scope: Option<ChatScope>,
  /// The project, and its name as people read it.
  pub(super) project: (ProjectId, std::sync::Arc<str>),
  /// The action's entry in the conversation.
  pub(super) seq: i64,
  pub(super) action: ChatAction,
  pub(super) at: Timestamp,
  /// Its line is one of a list in its answer, which says no time (`Actions` `J0N1u`).
  pub(super) listed: bool,
}

impl DevtoolsInspectable for ChangeProps {}

/// Whether the card of `action` waits for the person: a decision card rather than a line.
pub(super) fn waits(action: &ChatAction) -> bool {
  Phase::of(action) == Phase::Waits
}

/// What was read for a card: its facts, and whether the person lets the Orchestrator publish drafts and lets
/// automation widen what agents may do.
#[derive(Clone, Debug, PartialEq)]
struct Read {
  facts: Option<read::Facts>,
  publishes: bool,
  widens: bool,
}

impl DevtoolsInspectable for Read {}

pub(super) struct ChangeCard {
  decide: DecideAction,
  /// Synthetic input tried the person's action.
  refused: Signal<bool>,
  /// The facts after the first three are shown.
  details: Signal<bool>,
  /// The facts' reading took [`SKELETON_DELAY`]: their bones show.
  slow: Signal<bool>,
  /// Started only while the card's facts are read.
  delay: Timeout,
}

impl Component for ChangeCard {
  type Props = ChangeProps;

  fn create(ctx: &mut Ctx) -> Self {
    let slow = ctx.signal(false);
    let show = slow.clone();
    let delay = ctx.create_timeout(SKELETON_DELAY, move || show.set(true));
    Self {
      decide: card::decide_action(ctx),
      refused: ctx.signal(false),
      details: ctx.signal(false),
      slow,
      delay,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChangeProps>().clone();
    let loaded = read_facts(ctx, &props);
    let read = loaded.data.as_ref();
    let view = View {
      action: &props.action,
      at: props.at,
      facts: read.and_then(|read| read.facts.as_ref()),
      project: &props.project.1,
    };
    let words = words::words(ctx, &view);
    let opens = opens::opens(ctx, (props.project.0, &props.project.1), &view);
    let deciding = view.phase() == Phase::Waits && words.accept.is_some() && words.settled.is_none();
    if !deciding {
      let failed = self.decide.state().get().error.or_else(|| loaded.error.clone());
      return folded::line(ctx, (&props.id, &view, props.listed), &words, (opens, failed));
    }
    self.decision(ctx, (&props, &view), words, (read, &loaded.error, opens))
  }
}

/// What a decision card shows besides its words: what was read, why reading failed, and the places it opens.
type Shown<'a> = (Option<&'a Read>, &'a Option<Message>, Vec<opens::Opens>);

impl ChangeCard {
  /// The decision card of `view`, which waits for the person.
  fn decision(
    &self,
    ctx: &mut Ctx,
    (props, view): (&ChangeProps, &View),
    words: Words,
    (read, unread, opens): Shown<'_>,
  ) -> Element {
    let id = props.id.as_str();
    let (rows, boxes) = self.content(ctx, props, view);
    // Until facts that come from the read arrive, they keep their room, so the card does not grow when they do; a
    // card whose proposal holds its facts shows them at once.
    let shape = reading::shape(view);
    let facts = if read.is_none() && unread.is_none() && rows.is_empty() && !shape.is_empty() {
      if !self.slow.get() {
        self.delay.start();
      }
      reading::column(id, &shape, self.slow.get())
    } else {
      facts::column(ctx, id, rows, &self.details)
    };
    let warnings = warnings(ctx, id, (view, read));
    let widens = warnings.iter().any(|(widening, _)| *widening);
    // What the person is shown it widens, once its facts were read without an error; until then it cannot be applied,
    // and the apply refuses anything else than what was shown.
    let shown = match (unread, read.map(|read| read.facts.as_ref())) {
      (None, Some(Some(read::Facts::Publication(publication)))) => Some(publication.widens.clone()),
      // A change with nothing to read (a release of new starts, a run or draft no longer there) widens nothing. Apply
      // checks it again first (`orchestrator::change::apply`: nothing held to release, a run that went on, a draft
      // saved again or gone) and refuses it with the card's own words.
      (None, Some(Some(read::Facts::Run(_)) | None)) => Some(Vec::new()),
      _ => None,
    };
    let notes = self.notes(ctx, (id, view, words.person_only), unread);
    let actions = self.actions(ctx, props, (words.accept, opens), shown);
    let time = orchester_workspace::display::clock(view.at);
    let (below, tip) = answer::shown(ctx, id, view, &words.effect);
    let head = decision::head(ctx, id, (words.glyph, &words.title), view.action.task.as_deref(), &time);
    let head = match tip {
      Some(tip) => Tooltip::new(format!("{id}-why"), tip).wrap(ctx, head),
      None => head.into(),
    };
    decision::frame(id, view.phase().name(), widens)
      .describe("on_its_own", props.action.on_its_own.to_string())
      .child(head)
      .with_children(below)
      .with_children(facts)
      .with_children(boxes)
      .with_children(warnings.into_iter().map(|(_, strip)| strip))
      .with_children(notes)
      .with_children(actions)
      .into()
  }

  /// The facts of the card and, while it waits, the boxes of what it would do: a diff, git's commands, a `.gitignore`.
  fn content(&self, ctx: &mut Ctx, props: &ChangeProps, view: &View) -> (Vec<Fact>, Vec<Element>) {
    let id = props.id.as_str();
    let root = props.scope.as_ref().map(|scope| scope.root.as_path());
    match view.action.change.as_ref().map(|change| &change.change) {
      // A first commit shows what its proposal keeps, and the `.gitignore` it honours.
      Some(change @ Change::InitRepository { .. }) => (
        repository::rows(ctx, change, root),
        repository::gitignore(ctx, id, (view, change)).into_iter().collect(),
      ),
      // Git's work shows where it would run, its commands and what it would lose.
      Some(Change::Git(git)) => (git::rows(ctx, git, root), git::boxes(ctx, id, (view, git))),
      // An MCP server's change shows the server as applying it leaves it, which its proposal keeps.
      Some(Change::Mcp(mcp)) => (mcp::rows(ctx, mcp), Vec::new()),
      _ => (
        facts::rows(ctx, view, props.project.0),
        facts::diff(ctx, id, view).map(Into::into).into_iter().collect(),
      ),
    }
  }

  /// The notes over the actions: that the person approved it in the chat, a refused click of automation, why reading
  /// or deciding failed.
  fn notes(&self, ctx: &Ctx, (id, view, person): (&str, &View, &'static str), unread: &Option<Message>) -> Vec<Row> {
    let mut notes: Vec<Row> = card::approved_in_chat(ctx, id, view.action).into_iter().collect();
    if self.refused.get() {
      notes.push(person_only(format!("{id}-person-only"), &ctx.t(person)));
    }
    if let Some(error) = self.decide.state().get().error.or_else(|| unread.clone()) {
      notes.push(tinted_note(
        format!("{id}-error"),
        (Icon::CircleAlert, lurq::app::theme::PaletteColor::Danger),
        &message_text(ctx, &error),
      ));
    }
    notes
  }

  /// The accept, *Dismiss* and the places the card opens, with *Only you can apply*; nothing without the
  /// conversation's scope. The accept is taken only once the card knows what it widens (`shown`).
  fn actions(
    &self,
    ctx: &mut Ctx,
    props: &ChangeProps,
    (accept, opens): (Option<Accept>, Vec<opens::Opens>),
    shown: Option<Vec<Message>>,
  ) -> Option<Row> {
    let id = props.id.as_str();
    let (scope, accept) = (props.scope.as_ref()?, accept?);
    let pending = self.decide.state().get().is_pending();
    let deciding = |decision| Deciding {
      scope: scope.clone(),
      seq: props.seq,
      decision,
    };
    let start = ActionButton::new(format!("{id}-start"), accept.kind, accept.label).icon(accept.glyph);
    let dismiss = decision::dismiss_button(ctx, id);
    let (start, dismiss) = match (pending, shown) {
      (true, _) => (start.disabled(), dismiss.disabled()),
      (false, None) => (start.disabled(), self.dismiss(dismiss, deciding(Decision::Dismiss))),
      (false, Some(widens)) => {
        let (decide, refused, accepting) = (
          self.decide.clone(),
          self.refused.clone(),
          deciding(Decision::Accept { widens }),
        );
        let start = start.on_click(move || {
          if is_synthetic_input() {
            tracing::warn!("synthetic input reached a change the Orchestrator proposed; not taken");
            refused.set(true);
            return;
          }
          decide.run(accepting.clone());
        });
        (start, self.dismiss(dismiss, deciding(Decision::Dismiss)))
      }
    };
    let navigation = Navigation::of(ctx);
    let mut buttons: Vec<Element> = vec![start.into(), dismiss.into()];
    buttons.extend(opens.into_iter().enumerate().map(|(index, (_, label, route))| {
      decision::open_button(format!("{id}-open-{index}"), label)
        .opens(navigation.to(route))
        .into()
    }));
    Some(decision::actions(ctx, id, buttons))
  }

  /// *Dismiss*, which changes nothing and stays open to automation.
  fn dismiss(&self, dismiss: ActionButton, dismissing: Deciding) -> ActionButton {
    let decide = self.decide.clone();
    dismiss.on_click(move || decide.run(dismissing.clone()))
  }
}

/// The warning strips of a waiting card, each with whether it says the card widens what agents may do: a draft that
/// widens (saying why it waits while *Let the Orchestrator publish drafts* is on), files that look as if they hold
/// secrets, git work that loses work, an MCP server that widens or whose program is not found.
fn warnings(ctx: &Ctx, id: &str, (view, read): (&View, Option<&Read>)) -> Vec<(bool, Row)> {
  let mut strips = Vec::new();
  let widening = match read.and_then(|read| read.facts.as_ref()) {
    Some(read::Facts::Publication(found)) if !found.widens.is_empty() => Some(found.name.as_str()),
    _ => None,
  };
  if let Some(name) = widening {
    let key = if read.is_some_and(|read| read.publishes && !read.widens) {
      "orchestrator.change.publish.widens"
    } else {
      "orchestrator.change.publish.widensWaits"
    };
    let words = ctx.t_args(key, [("name", name)]);
    strips.push((true, decision::warning(id, "widens", &words)));
  }
  let Some(change) = view.action.change.as_ref() else {
    return strips;
  };
  strips.extend(repository::sensitive(ctx, id, (view, &change.change)).map(|strip| (false, strip)));
  match &change.change {
    Change::Git(git) => strips.extend(git::warning(ctx, id, (view, git)).map(|strip| (false, strip))),
    Change::Mcp(mcp) => strips.extend(
      mcp::warnings(ctx, id, (view, mcp))
        .into_iter()
        .map(|strip| (mcp.widens, strip)),
    ),
    _ => {}
  }
  strips
}

/// The card's facts and the person's switches, read again whenever the action's outcome changes.
fn read_facts(ctx: &mut Ctx, props: &ChangeProps) -> lurq::app::ctx::FutureState<Read, Message> {
  let services = Services::of(ctx);
  let (root, action) = (
    props.scope.as_ref().map(|scope| scope.root.clone()),
    props.action.clone(),
  );
  let key = (props.seq, Phase::of(&props.action).name(), props.action.settled);
  ctx
    .future(key, move |_| {
      let (services, root, action) = (services.clone(), root.clone(), action.clone());
      async move {
        background::run(move || {
          let workspace = &services.workspace;
          // Without a scope there is no project whose switch could let it publish.
          let (facts, publishes) = match root {
            Some(root) => (
              read::load_in(&root, &services.programs, &action)?,
              workspace
                .orchestrator_allows(&root, OrchestratorConsent::PublishDrafts)
                .map_err(|error| messages::store_error(&error))?,
            ),
            None => (None, false),
          };
          let widens = workspace
            .automation_changes_permissions()
            .map_err(|error| messages::store_error(&error))?;
          Ok(Read {
            facts,
            publishes,
            widens,
          })
        })
        .await?
      }
    })
    .state()
    .get()
}
