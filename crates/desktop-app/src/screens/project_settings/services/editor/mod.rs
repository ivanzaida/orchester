//! A service's page under Project settings › Services, for a new service and an existing one (`02 · Service editor`
//! `TGRTt`; `docs/architecture/agent-testing.md`): the crumb back to Services and the head, then in a 720 pane the
//! Program group (program with whether it is found, arguments one per line, working folder, variables plain or a
//! project secret by name) and Needs (chips, *Add a service* and the start-order chain) ([`fields`], [`needs`]), and
//! at the pane's end Delete with the draft's state and Publish ([`actions`], [`delete`]); beside the pane the *Start
//! order* card. Readiness, seed, per-run data, the UI driver, owned paths and approval are not edited here yet: what
//! the service holds of them is kept as stored.
//!
//! The draft saves as a person leaves a field, as the rest of Project settings does, and its status says at the
//! page's title; a new service is made with its first save, and its page then opens under its id. Every value is
//! checked by the core type it becomes and refused in place (`orchester_workspace::agent_testing::editor::form`);
//! placeholders that name a service it does not need are said in place too, and keep Publish waiting. A service runs
//! its program with the person's rights, so lurq's synthetic input (an MCP client driving the window) makes one,
//! changes what it runs or publishes it only under *Let automation change agent permissions*, as
//! `save_service_draft` and `publish_service` do: what such input typed into those fields otherwise is put back as the
//! draft next saves, whoever's input saves it.

mod actions;
mod delete;
mod fields;
mod needs;
mod variables;

use std::{path::PathBuf, sync::Arc};

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction, FutureState},
    theme::SpacingSize,
  },
  components::{Column, Row},
  core::Signal,
  layout::Alignment,
  node::{Element, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ResourceId, service::Service};
use orchester_workspace::{
  agent_testing::{
    data::Entry,
    editor::{
      EditorRead,
      form::{ServiceForm, ServiceProblems},
      load_in,
      order::{Field, placeholder_problem},
      publish_in, save_in,
    },
    widening::{FIRST_VERSION, NEW_SERVICE, changes},
  },
  background,
  messages::Message,
};

use self::fields::Fields;
use super::super::{
  Status, TabProps,
  kit::{back, page_head},
  page::column_scroll_wide,
};
use crate::{
  mcp::input_gate::{Guarded, InputGate, input_may_decide},
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route, ServicesPage},
  theme::ExtraSpacing,
  ui::{
    message::message_text,
    state::{load_error, loading},
  },
};

/// The page's room: the 720 pane, 32 and the side column (`Body` `czsh2`, `Pane` `aEj3e`, `Aside` `hJqTm`).
const PAGE_WIDTH: f32 = 1136.0;
/// The pane (`Pane` `aEj3e`).
const PANE_WIDTH: f32 = 720.0;
/// Where the side column's first card starts, level with the Program group (`Gap` `ElHia`, the aside's 2 and 20).
const ASIDE_TOP: f32 = 92.0;
/// A row's label column (`Text` `m8rIK4`, `l1tJi`).
const LABEL_WIDTH: f32 = 220.0;

/// Saves the draft as the fields are now.
type Commit = Arc<dyn Fn() + Send + Sync>;

/// The page of service `id`, or of a new one.
pub(in crate::screens::project_settings) fn editor(ctx: &mut Ctx, props: &TabProps, id: Option<ResourceId>) -> Element {
  let key = id.map_or_else(
    || "settings-service-editor-new".to_owned(),
    |id| format!("settings-service-editor-{id}"),
  );
  ctx.mount_keyed::<Editor>(&key, EditorProps { tab: props.clone(), id })
}

#[derive(Clone)]
struct EditorProps {
  tab: TabProps,
  id: Option<ResourceId>,
}

impl PartialEq for EditorProps {
  fn eq(&self, other: &Self) -> bool {
    self.tab == other.tab && self.id == other.id
  }
}

impl DevtoolsInspectable for EditorProps {}

/// What is stored: the page as it was read, or as the last save or publish read it back.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Read(EditorRead);

impl DevtoolsInspectable for Read {}

/// A save asked for: the project's folder, the service (none for a new one), its body and the revision it is made on.
type SaveAsk = (PathBuf, Option<ResourceId>, Service, Option<String>);
type Save = FutureAction<SaveAsk, Read, Message>;
type Publish = FutureAction<(PathBuf, ResourceId, Service), Read, Message>;

/// What the fields make of themselves against what is stored, for the parts of the page.
struct View {
  stored: EditorRead,
  built: Result<Service, Box<ServiceProblems>>,
  /// The first placeholder that cannot be used, with the field it is in.
  placeholder: Option<(Field, Message)>,
  /// The fields hold a change that is not saved, valid or not.
  changed: bool,
}

impl View {
  fn base(&self) -> Option<&Service> {
    self.stored.entry.as_ref().map(|entry| &entry.body)
  }

  fn services(&self) -> &[Entry<Service>] {
    &self.stored.services
  }

  fn problems(&self) -> Option<&ServiceProblems> {
    self.built.as_ref().err().map(AsRef::as_ref)
  }
}

/// The service a save under way sends, which is stored once it lands.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Sending(Option<Service>);

impl DevtoolsInspectable for Sending {}

#[derive(Clone)]
struct Editor {
  fields: Fields,
  stored: Signal<Option<Read>>,
  /// Counts *Try again*s after a failed read.
  retry: Signal<u64>,
  save: Save,
  publish: Publish,
  /// What the save under way sends.
  sending: Signal<Sending>,
  /// A field was left or a need changed while a save was under way: the page saves again once it lands.
  again: Signal<bool>,
  /// An MCP client's input asked for what only a person decides here; nothing was saved or published.
  refused: Signal<bool>,
  /// The delete question shows.
  deleting: Signal<bool>,
  gate: Option<InputGate>,
}

impl Component for Editor {
  type Props = EditorProps;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<EditorProps>().clone();
    let editor = Self {
      fields: Fields::new(ctx),
      stored: ctx.signal(None),
      retry: ctx.signal(0),
      save: ctx.future_action(|(root, id, service, expected): SaveAsk| async move {
        background::run(move || {
          let saved = save_in(&root, id, service, expected.as_deref())?;
          load_in(&root, Some(saved.id)).map(Read)
        })
        .await?
      }),
      publish: ctx.future_action(|(root, id, service): (PathBuf, ResourceId, Service)| async move {
        background::run(move || {
          publish_in(&root, id, &service)?;
          load_in(&root, Some(id)).map(Read)
        })
        .await?
      }),
      sending: ctx.signal(Sending(None)),
      again: ctx.signal(false),
      refused: ctx.signal(false),
      deleting: ctx.signal(false),
      gate: ctx.use_context::<InputGate>(),
    };
    editor.fields.watch_automation(ctx, editor.gate.as_ref());
    editor.follow(ctx, &props);
    // A service chosen in *Add a service* is needed after the others, and the draft saves.
    let commit = editor.commit(props.tab.project.open.repo_root.clone());
    let (adding, needs) = (editor.fields.adding.clone(), editor.fields.needs.clone());
    ctx.watch(&editor.fields.adding, move |chosen: &String| {
      if chosen.is_empty() {
        return;
      }
      let chosen = chosen.clone();
      adding.set(String::new());
      needs.update(|needs| {
        if !needs.0.contains(&chosen) {
          needs.0.push(chosen);
        }
      });
      commit();
    });
    editor
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<EditorProps>().clone();
    let root = props.tab.project.open.repo_root.clone();
    let read = ctx
      .future((root.clone(), props.id, self.retry.get()), |(root, id, _)| async move {
        background::run(move || load_in(&root, id).map(Read)).await?
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
      // A field was left while the last save was under way: what it holds now saves after it.
      self.again.set(false);
      self.save_now(&root);
    }
    self.fields.follow();
    let view = self.view(stored);
    self.status(&props.tab, &view);
    let commit = self.commit(root);
    let pane = Column::new()
      .id("settings-service-editor")
      .width(Dimension::Px(PANE_WIDTH))
      .flex_shrink(0.0)
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb(ctx, &props, &view))
      .child(head(ctx, &view))
      .child(fields::program_group(ctx, &self.fields, &view, &commit))
      .child(needs::needs_group(ctx, &self.fields, &view, &commit))
      .child(actions::actions(ctx, self, &props, &view));
    let aside = Column::new()
      .flex(1.0)
      .min_width(Dimension::Px(0.0))
      .padding(Padding::new().top(Dimension::Px(ASIDE_TOP)))
      .with_children(needs::start_order_card(ctx, &view));
    let deleting = self
      .deleting
      .get()
      .then(|| delete::dialog(ctx, &props.tab, &view, &self.deleting))
      .flatten();
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

impl Editor {
  /// After a save or a publish: what it read back is stored, the page's status says so, and the project's other
  /// screens read again; a new service's page opens under its id, once a save asked for meanwhile landed too.
  fn follow(&self, ctx: &mut Ctx, props: &EditorProps) {
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
        // The page saves again as it renders, and opens the new service's page after that save.
        return;
      }
      if let (true, Some(id)) = (new, id) {
        navigation.replace(Route::Project(
          project,
          ProjectPage::Services(ServicesPage::Service(id)),
        ));
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
      .map(|entry| ServiceForm::of(&entry.body))
      .unwrap_or_default();
    self.fields.fill(&form);
    self.stored.set(Some(read.clone()));
  }

  fn view(&self, stored: EditorRead) -> View {
    let form = self.fields.form();
    let base = stored.entry.as_ref().map(|entry| &entry.body);
    let built = form.service(base);
    let placeholder = built
      .as_ref()
      .ok()
      .and_then(|service| placeholder_problem(service, &stored.services));
    let shown = base.map(ServiceForm::of).unwrap_or_default();
    let changed = match &built {
      Ok(service) => base != Some(service),
      Err(_) => form != shown,
    };
    View {
      stored,
      built,
      placeholder,
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

  /// Saves the fields as they are now: a field left, a need added or removed.
  fn commit(&self, root: PathBuf) -> Arc<dyn Fn() + Send + Sync> {
    let editor = self.clone();
    Arc::new(move || editor.save_now(&root))
  }

  /// Saves the fields as they are now. What an MCP client changed of what the service runs, while only a person decides
  /// that, is put back first to what is stored (or being stored), whoever's input saves; a save asked for while one is
  /// under way follows it.
  fn save_now(&self, root: &std::path::Path) {
    let Some(Read(stored)) = self.stored.get_untracked() else {
      return;
    };
    let base = stored.entry.as_ref().map(|entry| &entry.body);
    let pending = self.save.state().get_untracked().is_pending();
    let sending = self.sending.get_untracked().0.filter(|_| pending);
    let to = sending.as_ref().or(base).map(ServiceForm::of).unwrap_or_default();
    let put_back = self.fields.put_back(&to);
    if put_back {
      tracing::warn!("synthetic input changed what a service runs; put back");
      self.refused.set(true);
    }
    if pending {
      self.again.set(true);
      return;
    }
    let Ok(service) = self.fields.form().service(base) else {
      return;
    };
    if base == Some(&service) {
      return;
    }
    let reasons = base.map_or_else(|| vec![Message::new(NEW_SERVICE)], |base| changes(base, &service));
    if !self.may(&reasons) {
      tracing::warn!("synthetic input asked to change what a service runs; not saved");
      return;
    }
    if !put_back {
      self.refused.set(false);
    }
    let id = stored.entry.as_ref().map(|entry| entry.id);
    self.sending.set(Sending(Some(service.clone())));
    self.save.run((root.to_path_buf(), id, service, stored.revision));
  }

  /// Publishes the saved draft `service` of service `id`.
  fn publish_now(&self, root: PathBuf, (id, service): (ResourceId, Service), latest: Option<&Service>) {
    if self.publish.state().get_untracked().is_pending() {
      return;
    }
    let reasons = latest.map_or_else(|| vec![Message::new(FIRST_VERSION)], |latest| changes(latest, &service));
    if !self.may(&reasons) {
      tracing::warn!("synthetic input asked to publish a service; not published");
      return;
    }
    self.refused.set(false);
    self.publish.run((root, id, service));
  }

  /// Whether the input being delivered may take a change that widens for `reasons`: a person's always, an MCP
  /// client's only under *Let automation change agent permissions*. A refusal is said under the actions.
  fn may(&self, reasons: &[Message]) -> bool {
    let may = reasons.is_empty() || input_may_decide(self.gate.as_ref(), Guarded::PermissionWidening);
    if !may {
      self.refused.set(true);
    }
    may
  }

  /// Before the service was ever read: why reading failed with Try again, or that it is being read.
  fn pending(&self, ctx: &mut Ctx, read: &FutureState<Read, Message>) -> Element {
    let shown = match &read.error {
      Some(error) => {
        let retry = self.retry.clone();
        load_error(
          ctx,
          "settings-service-editor-load-error",
          (
            &ctx.t("project_settings.services.editor.loadFailed"),
            &message_text(ctx, error),
          ),
          move || retry.update(|count| *count += 1),
        )
      }
      None => loading(
        "settings-service-editor-loading",
        &ctx.t("project_settings.services.editor.reading"),
      )
      .into(),
    };
    Element::from(column_scroll_wide(shown, PAGE_WIDTH))
  }
}

/// The crumb back to Services, with the service's name or *New service* after it (`Crumb` `gm2Lr`).
fn crumb(ctx: &mut Ctx, props: &EditorProps, view: &View) -> Element {
  let here = view.stored.entry.as_ref().map_or_else(
    || ctx.t("project_settings.services.newService"),
    |entry| entry.name.clone(),
  );
  back(
    "settings-service-editor-crumb",
    (&ctx.t("project_settings.services.tab"), Some(&here)),
    Navigation::of(ctx).to(Route::Project(
      props.tab.project.open.project.id(),
      ProjectPage::Settings(ProjectSettingsTab::Services),
    )),
  )
}

/// The service's name over what a service is (`Head` `bwNzh`).
fn head(ctx: &Ctx, view: &View) -> Element {
  let (title, lead) = match &view.stored.entry {
    Some(entry) => (entry.name.clone(), ctx.t("project_settings.services.editor.lead")),
    None => (
      ctx.t("project_settings.services.newService"),
      ctx.t("project_settings.services.editor.newLead"),
    ),
  };
  page_head("settings-service-editor-head", &title, Some(&lead))
}
