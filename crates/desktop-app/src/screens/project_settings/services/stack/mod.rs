//! A stack's page under Project settings › Services, for a new stack and an existing one (`03 · Stack editor`
//! `iNxTM`; `docs/architecture/agent-testing.md`, "Stack"): the crumb back to Services through Stacks and the head,
//! then in a 720 pane the Stack group (its name), Services (every service a start brings up, in start order with its
//! step, the ones chosen with their × and the ones a need brings marked so, then *Add a service*) ([`members`]),
//! Slots (the *Services at once* slots a start takes of the computer's) and Used by (the suites that need it)
//! ([`parts`]), and at the pane's end *Delete stack* with the draft's state and Publish; beside the pane the
//! *Dependencies* card, the same services as a tree of needs.
//!
//! The draft saves as a person leaves the name or adds or removes a service, as the rest of Project settings does,
//! and its status says at the page's title; a new stack is made with its first save, and its page then opens under
//! its id. Every value is checked by the core type it becomes and refused in place
//! (`orchester_workspace::agent_testing::stack_editor`). A stack starts only published services, so saving and
//! publishing one widen nothing: an MCP client's input is taken here as `save_stack_draft` takes it.

mod members;
mod parts;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
    theme::SpacingSize,
  },
  components::{Column, Row, Spacer, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ResourceId, service::Stack};
use orchester_workspace::{
  agent_testing::stack_editor::{
    StackForm, StackProblems, StackRead, load_stack_in,
    order::{Member, members},
    publish_stack_in, save_stack_in,
  },
  background,
  messages::Message,
};

use super::{
  super::{
    Status, TabProps,
    kit::{back_through, page_head},
    page::column_scroll_wide,
  },
  delete::{Deleted, dialog},
  editor::{ASIDE_TOP, Commit, PAGE_WIDTH, PANE_WIDTH, fields::Needs},
  lifecycle::{next_version, stale_reason, state_words},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route, ServicesPage},
  theme::{ExtraSpacing, ExtraTypography},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    state::{load_error, loading},
  },
};

/// The page of stack `id`, or of a new one.
pub(super) fn editor(ctx: &mut Ctx, props: &TabProps, id: Option<ResourceId>) -> Element {
  let key = id.map_or_else(
    || "settings-stack-editor-new".to_owned(),
    |id| format!("settings-stack-editor-{id}"),
  );
  ctx.mount_keyed::<StackEditor>(&key, StackProps { tab: props.clone(), id })
}

#[derive(Clone)]
struct StackProps {
  tab: TabProps,
  id: Option<ResourceId>,
}

impl PartialEq for StackProps {
  fn eq(&self, other: &Self) -> bool {
    self.tab == other.tab && self.id == other.id
  }
}

impl DevtoolsInspectable for StackProps {}

/// What is stored: the page as it was read, or as the last save or publish read it back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(StackRead);

impl DevtoolsInspectable for Read {}

/// A save asked for: the project's folder, the stack (none for a new one), its body and the revision it is made on.
type SaveAsk = (PathBuf, Option<ResourceId>, Stack, Option<String>);
type Save = FutureAction<SaveAsk, Read, Message>;
type Publish = FutureAction<(PathBuf, ResourceId, Stack), Read, Message>;

/// What the fields make of themselves against what is stored, for the parts of the page.
struct View {
  stored: StackRead,
  built: Result<Stack, Box<StackProblems>>,
  /// The services a start brings up, from the stack as the fields make it or, while they hold a mistake, as stored.
  members: Option<Result<Vec<Member>, Message>>,
  /// The fields hold a change that is not saved, valid or not.
  changed: bool,
}

impl View {
  fn base(&self) -> Option<&Stack> {
    self.stored.entry.as_ref().map(|entry| &entry.body)
  }

  fn problems(&self) -> Option<&StackProblems> {
    self.built.as_ref().err().map(AsRef::as_ref)
  }

  /// The stack as the fields make it, or as stored while they hold a mistake.
  fn stack(&self) -> Option<&Stack> {
    self.built.as_ref().ok().or_else(|| self.base())
  }

  /// The stack's name: as stored, or as typed for a new one.
  fn name(&self, typed: &str) -> String {
    self
      .base()
      .map_or_else(|| typed.trim().to_owned(), |base| base.name().as_str().to_owned())
  }
}

#[derive(Clone)]
struct StackEditor {
  name: Signal<String>,
  services: Signal<Needs>,
  /// The service *Add a service* chose, until it is added.
  adding: Signal<String>,
  stored: Signal<Option<Read>>,
  /// Counts *Try again*s after a failed read.
  retry: Signal<u64>,
  save: Save,
  publish: Publish,
  /// A field was left or a service changed while a save was under way: the page saves again once it lands.
  again: Signal<bool>,
  /// The delete question shows.
  deleting: Signal<bool>,
}

impl Component for StackEditor {
  type Props = StackProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<StackProps>().clone();
    let editor = Self {
      name: ctx.signal(String::new()),
      services: ctx.signal(Needs::default()),
      adding: ctx.signal(String::new()),
      stored: ctx.signal(None),
      retry: ctx.signal(0),
      save: ctx.future_action(|(root, id, stack, expected): SaveAsk| async move {
        background::run(move || {
          let saved = save_stack_in(&root, id, stack, expected.as_deref())?;
          load_stack_in(&root, Some(saved.id)).map(Read)
        })
        .await?
      }),
      publish: ctx.future_action(|(root, id, stack): (PathBuf, ResourceId, Stack)| async move {
        background::run(move || {
          publish_stack_in(&root, id, &stack)?;
          load_stack_in(&root, Some(id)).map(Read)
        })
        .await?
      }),
      again: ctx.signal(false),
      deleting: ctx.signal(false),
    };
    editor.follow(ctx, &props);
    // A service chosen in *Add a service* is added after the others, and the draft saves.
    let commit = editor.commit(props.tab.project.open.repo_root.clone());
    let (adding, services) = (editor.adding.clone(), editor.services.clone());
    ctx.watch(&editor.adding, move |chosen: &String| {
      if chosen.is_empty() {
        return;
      }
      let chosen = chosen.clone();
      adding.set(String::new());
      services.update(|services| {
        if !services.0.contains(&chosen) {
          services.0.push(chosen);
        }
      });
      commit();
    });
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<StackProps>().clone();
    let root = props.tab.project.open.repo_root.clone();
    let read = ctx
      .future((root.clone(), props.id, self.retry.get()), |(root, id, _)| async move {
        background::run(move || load_stack_in(&root, id).map(Read)).await?
      })
      .state()
      .get();
    if self.stored.get_untracked().is_none()
      && let Some(read) = &read.data
    {
      self.fill(read);
    }
    let Some(Read(stored)) = self.stored.get() else {
      return self.pending(ctx, &read);
    };
    if self.again.get_untracked() && !self.save.state().get_untracked().is_pending() {
      self.again.set(false);
      self.save_now(&root);
    }
    let view = self.view(stored);
    self.status(&props.tab, &view);
    let commit = self.commit(root);
    let pane = Column::new()
      .id("settings-stack-editor")
      .width(Dimension::Px(PANE_WIDTH))
      .flex_shrink(0.0)
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb(ctx, &props, &view, &self.name.get()))
      .child(head(ctx, &view))
      .child(members::stack_group(ctx, self, &view, &commit))
      .child(members::services_group(ctx, self, &view, &commit))
      .with_children(parts::slots_group(ctx, &view))
      .child(parts::used_by_group(ctx, &view, &self.name.get()))
      .child(self.actions(ctx, &props, &view));
    let aside = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::new().top(Dimension::Px(ASIDE_TOP)))
      .spacing(ExtraSpacing::Tabs.role())
      .with_children(parts::dependencies_card(ctx, &view));
    let deleting = view.stored.entry.as_ref().filter(|_| self.deleting.get()).map(|entry| {
      dialog(
        ctx,
        &props.tab,
        (Deleted::STACK, entry.id, entry.name.clone()),
        &self.deleting,
      )
    });
    let page = Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Xl)
      .align_items(Alignment::Start)
      .child(pane)
      .child(aside)
      .with_children(deleting);
    Element::from(column_scroll_wide(page, PAGE_WIDTH))
  }
}

impl StackEditor {
  /// After a save or a publish: what it read back is stored, the page's status says so, and the project's other
  /// screens read again; a new stack's page opens under its id, once a save asked for meanwhile landed too.
  fn follow(&self, ctx: &mut Ctx, props: &StackProps) {
    let navigation = Navigation::of(ctx);
    let project = props.tab.project.open.project.id();
    let (stored, status, feed, new, again) = (
      self.stored.clone(),
      props.tab.status.clone(),
      props.tab.project.feed.clone(),
      props.id.is_none(),
      self.again.clone(),
    );
    ctx.watch(&self.save.state(), move |state| {
      if state.is_pending() {
        status.set(Status::Saving);
        return;
      }
      if state.is_rejected() {
        again.set(false);
        status.set(Status::Failed);
        return;
      }
      let Some(read) = state.data.clone().filter(|_| state.is_fulfilled()) else {
        return;
      };
      let id = read.0.entry.as_ref().map(|entry| entry.id);
      stored.set(Some(read));
      status.set(Status::Saved);
      feed.saved();
      if again.get_untracked() {
        return;
      }
      if let (true, Some(id)) = (new, id) {
        navigation.replace(Route::Project(project, ProjectPage::Services(ServicesPage::Stack(id))));
      }
    });
    let (stored, feed) = (self.stored.clone(), props.tab.project.feed.clone());
    ctx.watch(&self.publish.state(), move |state| {
      if let Some(read) = state.data.clone().filter(|_| state.is_fulfilled()) {
        stored.set(Some(read));
        feed.saved();
      }
    });
  }

  fn fill(&self, read: &Read) {
    let form = read
      .0
      .entry
      .as_ref()
      .map(|entry| StackForm::of(&entry.body))
      .unwrap_or_default();
    self.name.set(form.name);
    self.services.set(Needs(form.services));
    self.stored.set(Some(read.clone()));
  }

  /// The fields as they are now, without following them.
  fn form(&self) -> StackForm {
    StackForm {
      name: self.name.get_untracked(),
      services: self.services.get_untracked().0,
    }
  }

  fn view(&self, stored: StackRead) -> View {
    self.name.get();
    self.services.get();
    let form = self.form();
    let base = stored.entry.as_ref().map(|entry| &entry.body);
    let built = form.stack(base);
    let shown = base.map(StackForm::of).unwrap_or_default();
    let changed = match &built {
      Ok(stack) => base != Some(stack),
      Err(_) => form != shown,
    };
    let bodies = stored.bodies();
    let members = built.as_ref().ok().or(base).map(|stack| members(stack, &bodies));
    View {
      stored,
      built,
      members,
      changed,
    }
  }

  /// The page's save status, unless a save is under way or failed and says its own.
  fn status(&self, props: &TabProps, view: &View) {
    let state = self.save.state().get();
    if state.is_pending() || (state.is_rejected() && view.changed) {
      return;
    }
    props.status.set(match (&view.built, view.changed) {
      (_, false) => Status::Saved,
      (Err(_), true) => Status::Invalid,
      (Ok(_), true) => Status::Unsaved,
    });
  }

  /// Saves the fields as they are now: the name left, a service added or removed.
  fn commit(&self, root: PathBuf) -> Commit {
    let editor = self.clone();
    Arc::new(move || editor.save_now(&root))
  }

  /// Saves the fields as they are now; a save asked for while one is under way follows it.
  fn save_now(&self, root: &std::path::Path) {
    let Some(Read(stored)) = self.stored.get_untracked() else {
      return;
    };
    if self.save.state().get_untracked().is_pending() {
      self.again.set(true);
      return;
    }
    let base = stored.entry.as_ref().map(|entry| &entry.body);
    let Ok(stack) = self.form().stack(base) else {
      return;
    };
    if base == Some(&stack) {
      return;
    }
    let id = stored.entry.as_ref().map(|entry| entry.id);
    self.save.run((root.to_path_buf(), id, stack, stored.revision));
  }

  /// Before the stack was ever read: why reading failed with Try again, or that it is being read.
  fn pending(&self, ctx: &mut Ctx, read: &FutureState<Read, Message>) -> Element {
    let shown = match &read.error {
      Some(error) => {
        let retry = self.retry.clone();
        load_error(
          ctx,
          "settings-stack-editor-load-error",
          (
            &ctx.t("project_settings.services.stack.loadFailed"),
            &message_text(ctx, error),
          ),
          move || retry.update(|count| *count += 1),
        )
      }
      None => loading(
        "settings-stack-editor-loading",
        &ctx.t("project_settings.services.stack.reading"),
      )
      .into(),
    };
    Element::from(column_scroll_wide(shown, PAGE_WIDTH))
  }

  /// The pane's last line (`Actions` `Xh7ea`): *Delete stack* at its start, and at its end where the stack stands with
  /// *Publish vN* while there is a draft to publish; under it why a save or a publish failed.
  fn actions(&self, ctx: &mut Ctx, props: &StackProps, view: &View) -> Element {
    let delete = view
      .stored
      .entry
      .as_ref()
      .map(|_| Deleted::STACK.button(ctx, &self.deleting));
    let stored = view.stored.entry.as_ref().map(|entry| entry.state);
    let line = Row::new()
      .id("settings-stack-actions")
      .width(Dimension::Pct(100.0))
      .padding(Padding::new().top(SpacingSize::Xs))
      .spacing(ExtraSpacing::Loose.role())
      .align_items(Alignment::Center)
      .with_children(delete)
      .child(Spacer::new().flex(1.0))
      .child(
        Text::new(&state_words(ctx, stored))
          .id("settings-stack-state")
          .variant(ExtraTypography::Note.role()),
      )
      .with_children(self.publish_button(ctx, props, view));
    let failures = [
      ("settings-stack-save-error", self.save.state().get()),
      ("settings-stack-publish-error", self.publish.state().get()),
    ]
    .into_iter()
    .filter_map(|(id, state)| {
      let error = state.error.as_ref().filter(|_| state.is_rejected())?;
      Some(Element::from(field_error(id, &message_text(ctx, error))))
    })
    .collect::<Vec<_>>();
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Snug.role())
      .child(line)
      .with_children(failures)
      .into()
  }

  /// *Publish vN*: it waits, and says why, for a draft from an older version, a mistake, a change not saved yet and
  /// services that do not resolve.
  fn publish_button(&self, ctx: &Ctx, props: &StackProps, view: &View) -> Option<Element> {
    let entry = view.stored.entry.as_ref()?;
    let next = next_version(entry.state, view.changed)?;
    let button = ActionButton::new(
      "settings-stack-publish",
      ButtonKind::Primary,
      ctx.t_args(
        "project_settings.services.editor.publish",
        [("version", next.get().to_string())],
      ),
    )
    .icon(Icon::Upload);
    let waits = stale_reason(ctx, entry.state).or_else(|| {
      if view.built.is_err() {
        Some(ctx.t("project_settings.services.editor.publishWaitsFix"))
      } else if view.changed || self.save.state().get().is_pending() {
        Some(ctx.t("project_settings.services.editor.publishWaitsSave"))
      } else {
        view
          .members
          .as_ref()
          .and_then(|members| members.as_ref().err())
          .map(|problem| message_text(ctx, problem))
      }
    });
    let button = match (waits, &view.built) {
      (Some(reason), _) => button.unavailable(reason),
      (None, _) if self.publish.state().get().is_pending() => button.loading(),
      (None, Ok(stack)) => {
        let (publish, root, id, stack) = (
          self.publish.clone(),
          props.tab.project.open.repo_root.clone(),
          entry.id,
          stack.clone(),
        );
        button.on_click(move || {
          if !publish.state().get_untracked().is_pending() {
            publish.run((root.clone(), id, stack.clone()));
          }
        })
      }
      (None, Err(_)) => button.disabled(),
    };
    Some(button.into())
  }
}

/// The crumb back to Services through Stacks, with the stack's name or *New stack* after it (`Crumb` `tiCDt`).
fn crumb(ctx: &mut Ctx, props: &StackProps, view: &View, typed: &str) -> Element {
  let here = match &view.stored.entry {
    Some(entry) => entry.name.to_string(),
    None if !typed.trim().is_empty() => view.name(typed),
    None => ctx.t("project_settings.services.newStack").to_string(),
  };
  back_through(
    "settings-stack-editor-crumb",
    (
      &ctx.t("project_settings.services.tab"),
      &ctx.t("project_settings.services.stacks"),
      &here,
    ),
    Navigation::of(ctx).to(Route::Project(
      props.tab.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Services),
    )),
  )
}

/// The stack's name over what starting it does (`Head` `z2PJt9`).
fn head(ctx: &Ctx, view: &View) -> Element {
  let (title, lead) = match &view.stored.entry {
    Some(entry) => (entry.name.to_string(), ctx.t("project_settings.services.stack.lead")),
    None => (
      ctx.t("project_settings.services.newStack").to_string(),
      ctx.t("project_settings.services.stack.newLead"),
    ),
  };
  page_head("settings-stack-editor-head", &title, Some(&lead))
}
