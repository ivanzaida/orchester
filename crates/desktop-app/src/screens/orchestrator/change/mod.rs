//! The card of a publication, cancel or merge the Orchestrator's agent proposed or the chat did itself, and of a run
//! it started without asking because auto mode is on (`Orchestrator · publish proposal` `b55NqO` and its states
//! `UKgrR`, `Orchestrator · run proposals · states` `Ee2JE`): in the action card's frame, its head with the state at
//! its end and, for a start in auto mode, the *Auto mode* chip; its facts and, while it waits, the diff of the text it
//! publishes; what it did or would do; its notes; and its actions. While it waits, only a person applies it: lurq's
//! synthetic input is refused in the handler, visibly; *Dismiss* changes nothing and stays open to automation. The
//! facts are read when the card shows ([`orchester_workspace::orchestrator::facts`]).

mod facts;
mod widens;
mod words;

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    events::is_synthetic_input,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::{
  id::ProjectId,
  orchestrator::{
    ChatAction,
    change::{Change, Publishing},
  },
  time::Timestamp,
};
use orchester_workspace::{
  background,
  messages::{self, Message},
  orchestrator::{facts as read, proposal::Decision, tools::ChatScope},
};

use self::words::{Accept, Phase, View, Words};
use super::card::{self, DecideAction, Deciding};
use crate::{
  navigation::Navigation,
  routes::{Route, TaskTab},
  services::Services,
  ui::{
    button::{ActionButton, ButtonKind},
    chip::TagChip,
    icon::Icon,
    message::message_text,
  },
};

#[derive(Clone, PartialEq)]
pub(super) struct ChangeProps {
  pub(super) id: String,
  /// The conversation's scope; without it nothing is read and no decision is taken.
  pub(super) scope: Option<ChatScope>,
  /// The project, and its name as people read it.
  pub(super) project: (ProjectId, Arc<str>),
  /// The action's entry in the conversation.
  pub(super) seq: i64,
  pub(super) action: ChatAction,
  pub(super) at: Timestamp,
}

impl DevtoolsInspectable for ChangeProps {}

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
}

impl Component for ChangeCard {
  type Props = ChangeProps;

  fn create(ctx: &mut Ctx) -> Self {
    Self {
      decide: card::decide_action(ctx),
      refused: ctx.signal(false),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<ChangeProps>().clone();
    let id = props.id.as_str();
    let loaded = read_facts(ctx, &props);
    let read = loaded.data.as_ref();
    let view = View {
      action: &props.action,
      at: props.at,
      facts: read.and_then(|read| read.facts.as_ref()),
      project: &props.project.1,
    };
    let Words {
      glyph,
      title,
      auto_mode,
      state,
      effect,
      accept,
      person_only,
    } = words::words(ctx, &view);
    let chip =
      auto_mode.then(|| TagChip::new(format!("{id}-auto-mode"), ctx.t("orchestrator.change.start.autoMode")).into());
    let head = card::head(id, (glyph, &title), chip, state);
    let facts = facts::rows(ctx, id, &view, props.project.0);
    let diff = facts::diff(ctx, id, &view);
    let notes = self.notes(ctx, (&props, &view, person_only), (read, &loaded.error));
    // What the person is shown it widens, once its facts were read without an error; until then it cannot be applied,
    // and the apply refuses anything else than what was shown.
    let shown = match (&loaded.error, read.and_then(|read| read.facts.as_ref())) {
      (None, Some(read::Facts::Publication(publication))) => Some(publication.widens.clone()),
      (None, Some(read::Facts::Run(_))) => Some(Vec::new()),
      _ => None,
    };
    card::frame(id, view.phase().name())
      .child(head)
      .with_children(facts)
      .with_children(diff)
      .with_children(notes.0)
      .child(
        Text::new(&effect)
          .id(format!("{id}-effect"))
          .variant(TypographyStyle::Body)
          .width(Dimension::Pct(100.0)),
      )
      .with_children(notes.1)
      .child(self.actions(ctx, &props, (&view, accept), shown))
  }
}

impl ChangeCard {
  /// *Dismiss*, which changes nothing and stays open to automation.
  fn dismiss(&self, dismiss: ActionButton, dismissing: Deciding) -> ActionButton {
    let decide = self.decide.clone();
    dismiss.on_click(move || decide.run(dismissing.clone()))
  }

  /// The notes before the effect (a draft that widens while the switch is on) and after it (how the chat published,
  /// what was checked, a refused click of automation, why reading or deciding failed).
  fn notes(
    &self,
    ctx: &Ctx,
    (props, view, person_only): (&ChangeProps, &View, &'static str),
    (read, unread): (Option<&Read>, &Option<Message>),
  ) -> (Vec<Row>, Vec<Row>) {
    let id = props.id.as_str();
    let project = &*props.project.1;
    let mut before = Vec::new();
    let mut after = Vec::new();
    let publication = view.action.change.as_ref().and_then(|change| match change.change {
      Change::Publish { of, .. } => Some(of),
      _ => None,
    });
    let widens = matches!(read.and_then(|read| read.facts.as_ref()), Some(read::Facts::Publication(found)) if !found.widens.is_empty());
    if view.phase() == Phase::Waits && widens && read.is_some_and(|read| read.publishes && !read.widens) {
      let name = view.facts.map_or_else(String::new, |facts| match facts {
        read::Facts::Publication(found) => found.name.clone(),
        read::Facts::Run(_) => String::new(),
      });
      before.push(card::note(
        id,
        "widens",
        (Icon::ShieldAlert, PaletteColor::Warning),
        &ctx.t_args("orchestrator.change.publish.widens", [("name", name.as_str())]),
      ));
    }
    match (view.phase(), publication) {
      (Phase::Done, Some(of)) => {
        let place = ctx.t(words::screen_of(of).title_key());
        after.push(card::note(
          id,
          "checked",
          (Icon::ShieldCheck, PaletteColor::TextMuted),
          &ctx.t_args(
            "orchestrator.change.publish.publishedAs",
            [("place", &*place), ("project", project)],
          ),
        ));
      }
      (Phase::Done | Phase::Refused(_), None) if view.action.auto_mode => after.push(card::note(
        id,
        "checked",
        (Icon::ShieldCheck, PaletteColor::TextMuted),
        &ctx.t_args("orchestrator.action.checked", [("project", project)]),
      )),
      _ => {}
    }
    if self.refused.get() {
      after.push(card::note(
        id,
        "person-only",
        (Icon::Hand, PaletteColor::Warning),
        &ctx.t(person_only),
      ));
    }
    let failed = self.decide.state().get().error.or_else(|| unread.clone());
    if let Some(error) = failed {
      after.push(card::note(
        id,
        "error",
        (Icon::CircleAlert, PaletteColor::Danger),
        &message_text(ctx, &error),
      ));
    }
    (before, after)
  }

  /// The person's action and *Dismiss* while it waits, then the places it opens. The action is taken only once the
  /// card knows what it widens (`shown`).
  fn actions(
    &self,
    ctx: &mut Ctx,
    props: &ChangeProps,
    (view, accept): (&View, Option<Accept>),
    shown: Option<Vec<Message>>,
  ) -> Row {
    let id = props.id.as_str();
    let mut row = Row::new()
      .id(format!("{id}-actions"))
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .align_items(Alignment::Center);
    if let (Phase::Waits, Some(scope), Some(accept)) = (view.phase(), &props.scope, accept) {
      let pending = self.decide.state().get().is_pending();
      let deciding = |decision| Deciding {
        scope: scope.clone(),
        seq: props.seq,
        decision,
      };
      let accept = ActionButton::new(format!("{id}-start"), accept.kind, accept.label).icon(accept.glyph);
      let dismiss = ActionButton::new(
        format!("{id}-dismiss"),
        ButtonKind::Secondary,
        ctx.t("orchestrator.proposal.dismiss"),
      );
      let (accept, dismiss) = match (pending, shown) {
        (true, _) => (accept.disabled(), dismiss.disabled()),
        (false, None) => (accept.disabled(), self.dismiss(dismiss, deciding(Decision::Dismiss))),
        (false, Some(widens)) => {
          let (decide, refused, accepting) = (
            self.decide.clone(),
            self.refused.clone(),
            deciding(Decision::Accept { widens }),
          );
          let accept = accept.on_click(move || {
            if is_synthetic_input() {
              tracing::warn!("synthetic input reached a change the Orchestrator proposed; not taken");
              refused.set(true);
              return;
            }
            decide.run(accepting.clone());
          });
          (accept, self.dismiss(dismiss, deciding(Decision::Dismiss)))
        }
      };
      row = row.child(accept).child(dismiss);
    }
    let navigation = Navigation::of(ctx);
    row.with_children(
      opens(ctx, props, view)
        .into_iter()
        .enumerate()
        .map(|(index, (glyph, label, route))| {
          let navigation = navigation.clone();
          ActionButton::new(format!("{id}-open-{index}"), ButtonKind::Ghost, label)
            .icon(glyph)
            .on_click(move || navigation.go(route.clone()))
        }),
    )
  }
}

/// The places a card opens: the draft or what was published, *What automation may do* after the chat published
/// itself, the run's changes while a merge waits, the task otherwise.
fn opens(ctx: &Ctx, props: &ChangeProps, view: &View) -> Vec<(Icon, Arc<str>, Route)> {
  let project = props.project.0;
  let task = view.action.task.as_deref();
  let task_open = |tab| {
    task.and_then(|key| {
      facts::task_route(project, key, tab).map(|route| {
        (
          Icon::ArrowUpRight,
          ctx.t_args("orchestrator.action.openTask", [("task", key)]),
          route,
        )
      })
    })
  };
  match view.action.change.as_ref().map(|change| &change.change) {
    Some(Change::Publish { of, resource, .. }) => publish_opens(ctx, (props, view), (*of, *resource)),
    Some(Change::Merge { .. }) if view.phase().waits() => task
      .and_then(|key| facts::task_route(project, key, TaskTab::Changes))
      .map(|route| {
        (
          Icon::ArrowUpRight,
          ctx.t("orchestrator.change.merge.openChanges"),
          route,
        )
      })
      .into_iter()
      .collect(),
    Some(Change::Cancel { .. }) if view.phase().waits() => Vec::new(),
    _ => task_open(TaskTab::Summary).into_iter().collect(),
  }
}

fn publish_opens(
  ctx: &Ctx,
  (props, view): (&ChangeProps, &View),
  publication: (Publishing, orchester_core::id::ResourceId),
) -> Vec<(Icon, Arc<str>, Route)> {
  let route = facts::draft_route(props.project.0, publication);
  match view.phase() {
    Phase::Applied | Phase::Done => {
      let name = view.facts.map_or_else(
        || {
          view
            .action
            .change
            .as_ref()
            .map(|change| change.subject.clone())
            .unwrap_or_default()
        },
        |facts| match facts {
          read::Facts::Publication(found) => found.name.clone(),
          read::Facts::Run(_) => String::new(),
        },
      );
      let mut opens = vec![(
        Icon::ArrowUpRight,
        ctx.t_args("orchestrator.change.publish.open", [("name", name.as_str())]),
        route,
      )];
      if view.phase() == Phase::Done {
        opens.push((
          Icon::Settings,
          ctx.t("orchestrator.change.publish.automation"),
          Route::Settings,
        ));
      }
      opens
    }
    Phase::Waits | Phase::Applying | Phase::Dismissed | Phase::Refused(_) => {
      vec![(
        Icon::ArrowUpRight,
        ctx.t("orchestrator.change.publish.openDraft"),
        route,
      )]
    }
  }
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
          let facts = match root {
            Some(root) => read::load_in(&root, &services.programs, &action)?,
            None => None,
          };
          let workspace = &services.workspace;
          let publishes = workspace
            .orchestrator_publishes_drafts()
            .map_err(|error| messages::store_error(&error))?;
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
