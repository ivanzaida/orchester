//! A profile's page (`01b Profile` `acOlR`, `01c` `WAWHr`): its head and draft bar ([`top`]), then Role, Connection &
//! model and the permissions, with what enforces them, what else reaches its prompts and where it is used beside them.
//! Edits save as a draft once the fields rest ([`super::autosave`]); a widening change is asked about at its section
//! first. A new profile is created with Create draft, and saves the same way from then on. A member of a shared project
//! reads the team's profile: its values as plain text, nothing saved (03 `pjEjf`).
mod top;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::SpacingSize,
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::resource::AgentProfile;
use orchester_workspace::{
  agents::{
    data::{ProfileEntry, ProjectAgents, VersionState},
    editor::{Action, Request},
  },
  messages::Message,
  sharing::team::TeamConfiguration,
};

use super::{
  AgentsNav,
  autosave::{Autosave, Source},
  fields::Fields,
  list, model,
  permissions::{self, Context, Section},
  questions::{self, Asking},
  shared, side,
  work::{self, Allowed, Asked, Follow, Pending, Saved, Work},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  project::{ShownProject, own::OwnChanges},
  screens::hub::{self, Head, team},
  theme::{ExtraSpacing, HubSpacing, HubText, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    failure::{Failure, field_hint, not_saved},
    field::field_error,
    icon::{Icon, icon},
    message::message_text,
  },
};

#[derive(Clone)]
pub struct Props {
  pub project: ShownProject,
  pub listed: ProjectAgents,
  pub entry: Option<ProfileEntry>,
  pub(super) nav: AgentsNav,
  /// Counts the reads of the profile into its page anew: a reload, a discarded draft.
  pub revision: Signal<u64>,
  pub retry: Signal<u64>,
  /// Where a save records its change of the profiles.
  pub profiles: OwnChanges,
  pub load_error: Option<Message>,
  pub reading: bool,
}
impl PartialEq for Props {
  fn eq(&self, other: &Self) -> bool {
    self.project == other.project
      && self.listed == other.listed
      && self.entry == other.entry
      && self.load_error == other.load_error
      && self.reading == other.reading
  }
}
impl DevtoolsInspectable for Props {}

/// A question that opens under the head about the profile itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
  Nothing,
  Discard,
  Remove,
}
impl DevtoolsInspectable for Ask {}

pub struct Editor {
  fields: Fields,
  work: Work,
  saved: Signal<Saved>,
  pending: Signal<Pending>,
  allowed: Signal<Allowed>,
  autosave: Autosave,
  ask: Signal<Ask>,
  more: Signal<bool>,
  anchor: ElementRef,
  /// Create draft was pressed: what keeps a new profile from being saved shows.
  tried: Signal<bool>,
  /// Who pressed Publish: what it changes in the role's prompts is a person's, or automation's under their consent.
  gate: Option<InputGate>,
}

impl Component for Editor {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let fields = Fields::new(ctx, &props.listed, props.entry.as_ref());
    fields.follow(ctx, props.listed.clone());
    let pending = ctx.signal(Pending(None));
    // Editing a field takes a question away: its fixed candidate is no longer what the fields hold.
    let dismiss = pending.clone();
    fields.on_edit(ctx, move || {
      if dismiss.get_untracked().0.is_some() {
        dismiss.set(Pending(None));
      }
    });
    let work = work::work(ctx);
    let saved = ctx.signal(Saved(props.entry.clone()));
    let allowed = ctx.signal(Allowed::default());
    let autosave = Autosave::new(
      ctx,
      Source {
        fields: fields.clone(),
        saved: saved.clone(),
        pending: pending.clone(),
        allowed: allowed.clone(),
        work: work.clone(),
      },
    );
    let again = autosave.clone();
    work::follow(
      ctx,
      &work,
      Follow {
        saved: saved.clone(),
        pending: pending.clone(),
        again: std::sync::Arc::new(move || again.again()),
      },
    );
    Self {
      fields,
      work,
      saved,
      pending,
      allowed,
      autosave,
      ask: ctx.signal(Ask::Nothing),
      more: ctx.signal(false),
      anchor: ctx.element_ref(),
      tried: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let team = team::of(&props.project);
    let may_change = team::may_change(team.as_ref());
    // A member of a shared project reads the team's profile: nothing of it is saved here.
    self
      .autosave
      .held
      .set(props.reading || props.load_error.is_some() || !may_change);
    self.autosave.listed.set(Some(props.listed.clone()));
    let state = self.state(&props);
    let saved = if may_change {
      self.saved_state(ctx, &state)
    } else {
      None
    };
    let head: Head = list::head(ctx, (&props.listed, team.as_ref()), &props.nav).saved(saved);
    let pane = self.pane(ctx, &props, (&state, team.as_ref()));
    let side = side::render(ctx, &props, &self.fields, self.pipeline_blocked(&props, &state));
    let body = hub::item_body("agent-editor", pane, Some(side));
    super::frame(ctx, &props.project, head, body)
  }
}

/// What the page knows of the profile in this render.
struct State {
  saved: Option<ProfileEntry>,
  checked: Result<AgentProfile, Box<orchester_workspace::agents::form::ProfileErrors>>,
  dirty: bool,
  busy: bool,
  changed_outside: bool,
}

impl Editor {
  fn state(&self, props: &Props) -> State {
    let saved = self.saved.get().0;
    let checked = self.fields.profile(&props.listed);
    let dirty = saved
      .as_ref()
      .is_none_or(|entry| checked.as_ref() != Ok(&entry.profile));
    let busy = self.work.state().get().is_pending();
    let changed_outside = !busy && saved.is_some() && props.entry != saved;
    State {
      saved,
      checked,
      dirty,
      busy,
      changed_outside,
    }
  }

  /// *Saving…*, *Draft saved* or *All changes saved* at the head's right; *A change was not saved · Try again* after a
  /// save failed.
  fn saved_state(&self, ctx: &Ctx, state: &State) -> Option<Element> {
    let entry = state.saved.as_ref()?;
    if !state.busy && !state.changed_outside && self.save_failure().is_some() {
      let again = self.autosave.clone();
      return Some(not_saved(ctx, "agent-save-status", move || again.retry()));
    }
    let (glyph, key) = if state.busy || (state.dirty && state.checked.is_ok()) {
      (Icon::LoaderCircle, "hub.saving")
    } else if state.dirty {
      return None;
    } else if entry.state.has_draft() {
      (Icon::Check, "hub.saved")
    } else {
      (Icon::Check, "hub.allSaved")
    };
    Some(
      Row::new()
        .id("agent-save-status")
        .spacing(ExtraSpacing::Snug.role())
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, lurq::app::theme::PaletteColor::TextMuted))
        .child(Text::new(&ctx.t(key)).variant(HubText::Hint.role()))
        .into(),
    )
  }

  fn pane(&self, ctx: &mut Ctx, props: &Props, (state, team): (&State, Option<&TeamConfiguration>)) -> Element {
    let may_change = team::may_change(team);
    let shared::Parts {
      bars: shared_bars,
      runs_here,
      status: here_status,
    } = shared::parts(ctx, props);
    let top = self.top(ctx, props, (state, team), (shared_bars, here_status));
    let asking = Asking {
      pending: self.pending.clone(),
      allowed: self.allowed.clone(),
      work: self.work.clone(),
    };
    let role = self.fields.role.get();
    let (section, mut asked) = match questions::widening(ctx, &asking, role.trim()) {
      Some((section, question)) => (Some(section), Some(question)),
      None => (None, None),
    };
    let connection_asked = if section == Some(Section::Connection) {
      asked.take()
    } else {
      None
    };
    // A change that wasn't saved stays in its field, which says so.
    let unsaved = self
      .save_failure()
      .filter(|_| !state.changed_outside)
      .zip(state.saved.as_ref())
      .map(|(error, entry)| (&entry.profile, field_hint(ctx, &error)));
    let (runs_here, runs_note) = runs_here.unzip();
    let (role, connection) = if may_change {
      let extra = runs_here.into_iter().chain(connection_asked).collect();
      (
        model::role(ctx, &self.fields, unsaved),
        model::connection_model(ctx, &self.fields, &props.listed, (extra, runs_note)),
      )
    } else {
      (
        model::role_read_only(ctx, &self.fields),
        model::connection_read_only(ctx, &self.fields, &props.listed, (runs_here, runs_note)),
      )
    };
    let context = Context {
      fields: &self.fields,
      listed: &props.listed,
      project: &props.project,
    };
    // A member reads the permissions as they are: nothing of them takes a press.
    let rest = permissions::sections(ctx, &context, section.zip(asked))
      .into_iter()
      .map(|section| {
        if may_change {
          section
        } else {
          Column::new()
            .width(Dimension::Pct(100.0))
            .pointer_events_none()
            .child(section)
            .into()
        }
      });
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(top)
      .child(role)
      .child(connection)
      .with_children(rest)
      .into()
  }

  /// Whether the profile's connection runs agents and enforces its permissions here.
  fn publishable(&self, props: &Props) -> bool {
    self.fields.connection(&props.listed).is_some_and(|connection| {
      connection.provider().runs_agents()
        && self
          .fields
          .permissions
          .get()
          .0
          .enforced_on(
            connection.provider(),
            orchester_core::connection::invocation::Platform::current(),
          )
          .is_ok()
    })
  }

  fn bar_actions(&self, ctx: &Ctx, props: &Props, (state, shared): (&State, bool)) -> Vec<Element> {
    let Some(entry) = state.saved.as_ref() else {
      return vec![self.create_button(ctx, state).into()];
    };
    let mut actions = Vec::new();
    if entry.state.has_draft() && entry.state.published().is_some() {
      let ask = self.ask.clone();
      let discard =
        ActionButton::new("agent-discard", ButtonKind::Ghost, ctx.t("hub.discard.action")).icon(Icon::Undo2);
      actions.push(
        if state.busy {
          discard.disabled()
        } else {
          discard.on_click(move || ask.set(Ask::Discard))
        }
        .into(),
      );
    }
    if entry.state.has_draft() {
      actions.push(self.publish_button(ctx, props, (state, entry), shared).into());
    }
    actions
  }

  fn create_button(&self, ctx: &Ctx, state: &State) -> ActionButton {
    let button = ActionButton::new("agent-create", ButtonKind::Primary, ctx.t("agents.createDraft")).icon(Icon::Check);
    if state.busy {
      return button.loading();
    }
    let (work, tried, allowed) = (self.work.clone(), self.tried.clone(), self.allowed.clone());
    let candidate = state.checked.as_ref().ok().cloned();
    button.on_click(move || {
      tried.set(true);
      if let Some(profile) = &candidate {
        work.run(Request {
          existing: None,
          profile: profile.clone(),
          action: Action::Save,
          allowed: allowed.get().0,
          prompt_decided: false,
        });
      }
    })
  }

  /// *Publish v5*, or *Publish for the team* in a shared project (02).
  fn publish_button(
    &self,
    ctx: &Ctx,
    props: &Props,
    (state, entry): (&State, &ProfileEntry),
    shared: bool,
  ) -> ActionButton {
    let next = entry.state.published().map_or(1, |v| v.get().saturating_add(1));
    let label = if shared {
      team::publish_label(ctx)
    } else {
      ctx.t_args("agents.publish", [("version", next.to_string())])
    };
    let publish = ActionButton::new("agent-publish", ButtonKind::Primary, label).icon(Icon::Upload);
    if !self.publishable(props) {
      return publish.unavailable(ctx.t("agents.notEnforced"));
    }
    if state.busy
      || state.dirty
      || props.reading
      || props.load_error.is_some()
      || state.changed_outside
      || self.pending.get().0.is_some()
    {
      return publish.disabled();
    }
    let (pending, work, gate, allowed) = (
      self.pending.clone(),
      self.work.clone(),
      self.gate.clone(),
      self.allowed.clone(),
    );
    let (entry, stale) = (entry.clone(), matches!(entry.state, VersionState::StaleDraft { .. }));
    publish.on_click(move || {
      let request = Request {
        existing: Some(entry.clone()),
        profile: entry.profile.clone(),
        action: if stale { Action::PublishStale } else { Action::Publish },
        allowed: allowed.get().0,
        // A stale draft waits for a person's confirmation, which no synthetic input gives; otherwise what publishing
        // changes in the role's prompts is the person's (ORC-30), read from the input that pressed it.
        prompt_decided: stale || input_may_decide(gate.as_ref(), Guarded::PermissionWidening),
      };
      if stale {
        pending.set(Pending(Some(Asked {
          request,
          reasons: Vec::new(),
        })));
      } else {
        work.run(request);
      }
    })
  }

  fn notices(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Vec<Element> {
    let mut notices = Vec::new();
    if let Some(error) = &props.load_error {
      let retry = props.retry.clone();
      notices.push(
        Failure::new("agents-load-error", ctx.t("agents.loadFailed"), error.clone())
          .retry(props.reading, move || retry.update(|n| *n += 1))
          .log(props.project.open.repo_root.clone())
          .page(ctx),
      );
    }
    if let Err(errors) = &state.checked
      && (state.saved.is_some() || self.tried.get())
    {
      for error in [
        &errors.role,
        &errors.instructions,
        &errors.connection,
        &errors.model,
        &errors.effort,
      ]
      .into_iter()
      .flatten()
      {
        notices.push(field_error("agent-validation-error", &message_text(ctx, error)).into());
      }
    }
    let failure = self.save_failure();
    if state.changed_outside || failure.is_some() {
      let text = failure.map_or_else(|| ctx.t("agents.outsideChange"), |error| message_text(ctx, &error));
      let revision = props.revision.clone();
      notices.push(
        Column::new()
          .width(Dimension::Pct(100.0))
          .spacing(SpacingSize::Sm)
          .child(field_error(
            if state.changed_outside {
              "agent-outside-change"
            } else {
              "agent-save-error"
            },
            &text,
          ))
          .child(
            ActionButton::new("agent-reload", ButtonKind::Secondary, ctx.t("agents.reload"))
              .icon(Icon::RotateCcw)
              .on_click(move || revision.update(|n| *n += 1)),
          )
          .into(),
      );
    }
    notices
  }

  /// Why the last save failed, while it is the last word of the page's writes.
  fn save_failure(&self) -> Option<Message> {
    let write = self.work.state().get();
    write.error.clone().filter(|_| write.is_rejected())
  }

  /// Why Open pipeline waits now.
  fn pipeline_blocked(&self, props: &Props, state: &State) -> Option<&'static str> {
    if self.pending.get().0.is_some() || state.busy {
      Some("agents.pipelineWait")
    } else if state.dirty {
      Some("agents.pipelineSaveFirst")
    } else if props.reading || props.load_error.is_some() || state.changed_outside {
      Some("agents.pipelineRefreshFirst")
    } else {
      None
    }
  }
}
