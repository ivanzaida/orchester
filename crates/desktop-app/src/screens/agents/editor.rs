//! A profile's page (`01b Profile` `acOlR`, `01c` `WAWHr`): its head and draft bar, then Role, Connection & model and
//! the permissions, with what enforces them, what else reaches its prompts and where it is used beside them. Edits save
//! as a draft once the fields rest ([`super::autosave`]); a widening change is asked about at its section first. A new
//! profile is created with Create draft, and saves the same way from then on.
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
use orchester_core::resource::{AgentProfile, ResourceKind};
use orchester_workspace::{
  agents::{
    data::{ProfileEntry, ProjectAgents, VersionState},
    editor::{Action, Request},
  },
  messages::Message,
  tasks::outside::Outside,
};

use super::{
  AgentsNav,
  autosave::{Autosave, Source},
  fields::Fields,
  list, model, page_head,
  permissions::{self, Context, Section},
  questions::{self, Asking},
  shared, side,
  work::{self, Allowed, Asked, Follow, Pending, Saved, Work},
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  project::{ShownProject, own::OwnChanges},
  screens::hub::{self, Head, discard, removal},
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
    self.autosave.held.set(props.reading || props.load_error.is_some());
    self.autosave.listed.set(Some(props.listed.clone()));
    let state = self.state(&props);
    let head: Head = list::head(ctx, &props.listed, &props.nav).saved(self.saved_state(ctx, &state));
    let pane = self.pane(ctx, &props, &state);
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

  fn pane(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Element {
    let more = state.saved.as_ref().map(|_| page_head::More {
      open: self.more.clone(),
      anchor: self.anchor.clone(),
      remove: {
        let ask = self.ask.clone();
        std::sync::Arc::new(move || ask.set(Ask::Remove))
      },
    });
    let (shared_bars, runs_here) = shared::parts(ctx, props);
    let top = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Row.role())
      .child(page_head::crumb(ctx, &props.nav))
      .child(page_head::head(ctx, (&self.fields, &props.listed), more))
      .with_children(shared_bars)
      .with_children(self.remove_question(ctx, props, state))
      .child(self.bar_or_question(ctx, props, state))
      .with_children(self.notices(ctx, props, state));
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
    let role = model::role(ctx, &self.fields, unsaved);
    let (runs_here, runs_note) = runs_here.unzip();
    let extra = runs_here.into_iter().chain(connection_asked).collect();
    let connection = model::connection_model(ctx, &self.fields, &props.listed, (extra, runs_note));
    let context = Context {
      fields: &self.fields,
      listed: &props.listed,
      project: &props.project,
    };
    let rest = permissions::sections(ctx, &context, section.zip(asked));
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(HubSpacing::Sections.role())
      .child(top)
      .child(role)
      .child(connection)
      .with_children(rest)
      .into()
  }

  fn remove_question(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Option<Element> {
    let entry = state.saved.as_ref().filter(|_| self.ask.get() == Ask::Remove)?;
    let (keep, nav) = (self.ask.clone(), props.nav.clone());
    Some(removal::question(
      ctx,
      removal::RemoveProps {
        project: props.project.clone(),
        resource: entry.id,
        kind: ResourceKind::AgentProfile,
        name: entry.profile.role().as_str().into(),
        on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
        on_removed: std::sync::Arc::new(move || nav.removed()),
      },
    ))
  }

  /// The draft bar, or what is asked about the draft in its place.
  fn bar_or_question(&self, ctx: &mut Ctx, props: &Props, state: &State) -> Element {
    let asking = Asking {
      pending: self.pending.clone(),
      allowed: self.allowed.clone(),
      work: self.work.clone(),
    };
    if let Some(question) = questions::stale(ctx, &asking) {
      return question;
    }
    if let Some(entry) = state.saved.as_ref().filter(|_| self.ask.get() == Ask::Discard) {
      return self.discard_question(ctx, props, entry);
    }
    let published = state
      .saved
      .as_ref()
      .and_then(|entry| props.listed.published.iter().find(|p| p.id == entry.id))
      .map(|p| &p.profile);
    let actions = self.bar_actions(ctx, props, state);
    page_head::bar(
      ctx,
      &self.fields,
      &props.listed,
      page_head::Bar {
        entry: state.saved.as_ref(),
        published,
        saved_at: state
          .saved
          .as_ref()
          .and_then(|entry| props.listed.draft_times.get(&entry.id).copied()),
        publishable: self.publishable(props),
        autopilot_waits: questions::autopilot_waits(&self.pending.get()),
        actions,
      },
    )
  }

  fn discard_question(&self, ctx: &mut Ctx, props: &Props, entry: &ProfileEntry) -> Element {
    let published = entry.state.published().map_or(0, |v| v.get());
    let (root, id, expected) = (props.project.open.repo_root.clone(), entry.id, entry.profile.clone());
    let (keep, done, revision) = (self.ask.clone(), self.ask.clone(), props.revision.clone());
    discard::question(
      ctx,
      discard::DiscardProps {
        project: props.project.clone(),
        versions: (published + 1, published),
        discard: std::sync::Arc::new(move || {
          orchester_workspace::removal::discard_in::<AgentProfile>(&root, id, &expected)
        }),
        change: Outside::Profiles,
        on_keep: std::sync::Arc::new(move || keep.set(Ask::Nothing)),
        on_discarded: std::sync::Arc::new(move || {
          done.set(Ask::Nothing);
          // The page reads the published version anew into its fields.
          revision.update(|n| *n += 1);
        }),
      },
    )
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

  fn bar_actions(&self, ctx: &Ctx, props: &Props, state: &State) -> Vec<Element> {
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
      actions.push(self.publish_button(ctx, props, (state, entry)).into());
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

  fn publish_button(&self, ctx: &Ctx, props: &Props, (state, entry): (&State, &ProfileEntry)) -> ActionButton {
    let next = entry.state.published().map_or(1, |v| v.get().saturating_add(1));
    let publish = ActionButton::new(
      "agent-publish",
      ButtonKind::Primary,
      ctx.t_args("agents.publish", [("version", next.to_string())]),
    )
    .icon(Icon::Upload);
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
