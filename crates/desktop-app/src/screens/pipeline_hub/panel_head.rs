//! The head of the chosen pipeline's panel (`Head` in `Pipelines · v2` P1 `s7LMyQ`, its ⋯ in P11 `nrTt0`): what it is
//! and its version over its name, and at its end the ⋯ menu ([`menu`]). *Rename* makes the name editable in place
//! ([`rename`]); *Archive* asks the hub for the confirmation it hosts, which outlives this panel; *Make default for New
//! task* stores the pipeline as the project's default and says why when it could not.

use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::SpacingSize,
  },
  components::{Column, Placement, Popup, Text},
  core::{ElementRef, Signal},
  node::{Element, dimension::Dimension},
};
use orchester_core::{id::ProjectId, resource::ResourceKind, time::Timestamp};
use orchester_workspace::{
  background,
  local_archive::Archivable,
  messages::Message,
  pipelines::{library::make_default_in, rename::rename_in},
  tasks::outside::Outside,
};

use super::{
  menu::{self, Actions},
  model::Entry,
  rename::{self, Editing},
  words,
};
use crate::{
  project::ShownProject,
  screens::{
    local_hub::archive::{Asking, Target},
    team_hub::panel::head_titled,
  },
  services::Services,
  theme::HubText,
  ui::{field::field_error, icon::Icon, icon_button::IconButton, message::message_text},
};

#[derive(Clone, PartialEq)]
pub(super) struct Props {
  pub project: ShownProject,
  pub entry: Entry,
  /// Whether New task offers this pipeline first.
  pub default: bool,
  pub now: Timestamp,
  pub asking: Asking,
}
impl DevtoolsInspectable for Props {}

/// The head of `props.entry`'s panel; its state is the pipeline's, so another pipeline starts with a closed menu and
/// no rename.
pub(super) fn head(ctx: &mut Ctx, props: Props) -> Element {
  let key = format!("pipeline-hub-head-{}", props.entry.id);
  ctx.mount_keyed::<Head>(&key, props)
}

struct Head {
  menu: Signal<bool>,
  anchor: ElementRef,
  editing: Editing,
  rename: FutureAction<String, Arc<str>, Message>,
  /// A name just saved with the name it replaced, shown until the hub reads the pipeline again.
  saved: Signal<Option<(String, Arc<str>)>>,
  make_default: FutureAction<(), (), Message>,
}

impl Component for Head {
  type Props = Props;

  fn create(ctx: &mut Ctx) -> Self {
    let props = ctx.props::<Props>().clone();
    let changes = Services::of(ctx).changes;
    let (root, project, id) = (
      props.project.open.repo_root.clone(),
      props.project.open.project.id(),
      props.entry.id,
    );
    let editing = Editing::new(ctx);
    let saved = ctx.signal(None);
    let (stop, record, task_root) = (editing.clone(), changes.clone(), root.clone());
    let (shown, remember) = (saved.clone(), saved.clone());
    let rename = ctx.future_action(move |name: String| {
      let (root, stop, changes, remember) = (task_root.clone(), stop.clone(), record.clone(), remember.clone());
      async move {
        let saved = background::run(move || rename_in(&root, id, &name).map_err(|error| error.message())).await??;
        let saved: Arc<str> = Arc::from(saved.as_str());
        remember.set(Some((stop.began(), saved.clone())));
        changes.record(project, Outside::Pipelines);
        changes.record(project, Outside::Tasks);
        stop.open.set(false);
        Ok(saved)
      }
    });
    let make_default = ctx.future_action(move |(): ()| {
      let (root, changes) = (root.clone(), changes.clone());
      async move {
        background::run(move || make_default_in(&root, id).map_err(|error| error.message())).await??;
        changes.record(project, Outside::Pipelines);
        changes.record(project, Outside::Tasks);
        Ok(())
      }
    });
    Self {
      menu: ctx.signal(false),
      anchor: ctx.element_ref(),
      editing,
      rename,
      saved: shown,
      make_default,
    }
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let entry = &props.entry;
    let title = self.title(ctx, entry);
    let more = self.more(ctx, &props);
    let head = head_titled(
      title,
      (
        ctx.t("pipeline_hub.panel.where").to_string(),
        Some(words::head_version(ctx, props.now, entry)),
      ),
      Some(more),
    );
    let failed = self.make_default.state().get();
    let failed = failed.error.as_ref().filter(|_| !failed.is_pending());
    Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Sm)
      .child(head)
      .with_children(failed.map(|error| {
        let reason = message_text(ctx, error);
        field_error(
          "pipeline-hub-default-error",
          &ctx.t_args("pipeline_hub.default.failed", [("reason", &*reason)]),
        )
      }))
  }
}

impl Head {
  /// The name, or the field that edits it.
  fn title(&self, ctx: &mut Ctx, entry: &Entry) -> Element {
    if self.editing.open.get() {
      return rename::field(ctx, &self.editing, &self.rename);
    }
    let saved = self.saved.get();
    let name = saved
      .as_ref()
      .filter(|(from, _)| **from == *entry.name)
      .map_or(&*entry.name, |(_, name)| &**name);
    Text::new(name)
      .id("team-hub-panel-title")
      .variant(HubText::PanelTitle.role())
      .into()
  }

  /// The ⋯ button and, while it is open, its menu.
  fn more(&self, ctx: &mut Ctx, props: &Props) -> Element {
    let entry = &props.entry;
    let toggle = self.menu.clone();
    let button = IconButton::new(
      "pipeline-hub-more",
      Icon::Ellipsis,
      ctx.t_args("pipeline_hub.menu.more", [("name", &*entry.name)]),
    )
    .small()
    .element(self.anchor.clone())
    .on_click(move || toggle.update(|open| *open = !*open));
    // While the menu is open its button has no tooltip, which would stand beside the menu and be clipped at the window.
    let button = if self.menu.get() { button.active() } else { button }.render(ctx);
    let (editing, name) = (self.editing.clone(), entry.name.clone());
    let target = Target {
      root: props.project.open.repo_root.clone(),
      project: props.project.open.project.id(),
      what: Archivable::Resource(entry.id),
      kind: ResourceKind::Pipeline,
      name: Arc::from(&*entry.name),
    };
    let asking = props.asking.clone();
    let actions = Actions {
      rename: Arc::new(move || editing.begin(&name)),
      make_default: self.make_default.clone(),
      archive: Arc::new(move || asking.ask(target.clone())),
    };
    let project: ProjectId = props.project.open.project.id();
    let panel = menu::panel(ctx, (project, entry, props.default), &self.menu, &actions);
    Column::new()
      .child(button)
      .child(
        Popup::new(self.anchor.clone(), panel)
          .open(self.menu.clone())
          .placement(Placement::BottomEnd)
          .offset(0.0, ctx.theme().spacing_value(SpacingSize::Xs).to_px()),
      )
      .into()
  }
}
