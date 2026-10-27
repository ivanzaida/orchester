//! A pipeline's row in Pipelines (`Feature delivery` `k4AoE` in `x1lmXb`): its name with *Default* when New task
//! starts with it, its steps as the hint, its version state with its tasks and age at the end, and a published
//! row's `…` menu (`Row menu` `v8j3h`): *Open*, *Duplicate*, which opens New pipeline copying it, and *Make default*.
//! A draft-only row has no menu: Copy lists only published pipelines and New task only those.
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::{Ctx, FutureAction},
    theme::{PaletteColor, SpacingSize},
  },
  components::{Column, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::Element,
};
use orchester_core::id::ProjectId;
use orchester_workspace::{
  background,
  messages::Message,
  pipelines::library::{LibraryEntry, VersionState, make_default_in},
  tasks::outside::Outside,
};

use crate::{
  navigation::Navigation,
  routes::{PipelineView, ProjectPage, Route},
  theme::{ExtraSpacing, GroupText, IconSize},
  ui::{
    icon::{Icon, icon},
    icon_button::IconButton,
    menu::{MenuItem, menu_at_end, menu_panel},
    message::message_text,
    pressable::Press,
    row_group::{GroupRow, HintTone, tag},
  },
};

#[derive(Clone, PartialEq)]
pub struct RowProps {
  pub project: ProjectId,
  /// The project's repository, where *Make default* stores the default.
  pub root: std::path::PathBuf,
  pub entry: LibraryEntry,
}
impl DevtoolsInspectable for RowProps {}

type DefaultAction = FutureAction<(), (), Message>;

pub struct LibraryRow {
  open: Signal<bool>,
  anchor: ElementRef,
}
impl Component for LibraryRow {
  type Props = RowProps;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      open: ctx.signal(false),
      anchor: ElementRef::default(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<RowProps>().clone();
    let make_default = default_action(ctx, &props);
    let entry = &props.entry;
    let nav = Navigation::of(ctx);
    let id = format!("pipeline-row-{}", entry.id);
    let mut row = GroupRow::new(id.clone(), entry.name.clone())
      .press(Press::Open(nav.to(Route::Project(
        props.project,
        ProjectPage::Pipeline(entry.id, PipelineView::default()),
      ))))
      .hint(chain(ctx, entry));
    if entry.default {
      row = row.tag(tag(&format!("{id}-default"), &ctx.t("pipelines.default.chip"), false));
    }
    if let Some(error) = make_default.state().get().error {
      row = row.toned_hint(message_text(ctx, &error), HintTone::Danger);
    }
    if !matches!(entry.state, VersionState::DraftOnly) {
      row = row.end(self.menu(ctx, &props, &make_default));
    }
    row
      .end(state(ctx, entry, &id))
      .end(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted))
  }
}

impl LibraryRow {
  /// The `…` button and, while it is open, its menu.
  fn menu(&self, ctx: &mut Ctx, props: &RowProps, make_default: &DefaultAction) -> Element {
    let entry = &props.entry;
    let id = format!("pipeline-row-menu-{}", entry.id);
    let open = self.open.clone();
    let button = IconButton::new(
      id.clone(),
      Icon::Ellipsis,
      ctx.t_args("pipelines.rowMenu", [("name", &*entry.name)]),
    )
    .element(self.anchor.clone())
    .on_click(move || open.update(|open| *open = !*open));
    let shown = self.open.get();
    let button = if shown { button.active() } else { button }.render(ctx);
    let menu = shown.then(|| {
      let nav = Navigation::of(ctx);
      let page = |page| nav.to(Route::Project(props.project, page));
      let mut panel = menu_panel(&format!("{id}-panel"))
        .width(lurq::node::dimension::Dimension::Px(MENU_WIDTH))
        .child(
          MenuItem::opens(
            format!("pipeline-row-open-{}", entry.id),
            ctx.t("pipelines.open"),
            page(ProjectPage::Pipeline(entry.id, PipelineView::default())),
          )
          .icon(Icon::ArrowUpRight)
          .render(ctx, &self.open),
        )
        .child(
          MenuItem::opens(
            format!("pipeline-row-duplicate-{}", entry.id),
            ctx.t("pipelines.duplicatePipeline"),
            page(ProjectPage::CopyPipeline(entry.id)),
          )
          .icon(Icon::Copy)
          .render(ctx, &self.open),
        );
      if !entry.default {
        let action = make_default.clone();
        panel = panel.child(
          MenuItem::new(
            format!("pipeline-row-default-{}", entry.id),
            ctx.t("pipelines.default.make"),
            move || action.run(()),
          )
          .icon(Icon::Pin)
          .render(ctx, &self.open),
        );
      }
      menu_at_end(self.anchor.clone(), self.open.clone(), panel)
    });
    Row::new().child(button).with_children(menu).into()
  }
}

/// *Make default*: stores the pipeline as the project's default, then the list reads it again.
fn default_action(ctx: &mut Ctx, props: &RowProps) -> DefaultAction {
  let root = props.root.clone();
  let changes = crate::services::Services::of(ctx).changes;
  let (project, pipeline) = (props.project, props.entry.id);
  ctx.future_action(move |()| {
    let (root, changes) = (root.clone(), changes.clone());
    async move {
      background::run(move || {
        make_default_in(&root, pipeline).map_err(|error| error.message())?;
        changes.record(project, Outside::Pipelines);
        changes.record(project, Outside::Tasks);
        Ok(())
      })
      .await?
    }
  })
}

/// The steps by stage, parallel ones joined (`Research → Plan → Implement → Run checks ∥ Review`).
fn chain(ctx: &Ctx, entry: &LibraryEntry) -> Arc<str> {
  let (then, parallel) = (ctx.t("pipelines.then"), ctx.t("pipelines.parallel"));
  if entry.stages.is_empty() {
    return ctx.t("pipelines.noSteps");
  }
  entry
    .stages
    .iter()
    .map(|stage| {
      stage
        .iter()
        .map(|name| name.as_ref())
        .collect::<Vec<_>>()
        .join(&parallel)
    })
    .collect::<Vec<_>>()
    .join(&then)
    .into()
}

/// The version state over its tasks and age, both at the row's end (`State` `T5CHp`).
fn state(ctx: &Ctx, entry: &LibraryEntry, id: &str) -> Column {
  let (words, glyph, color) = match entry.state {
    VersionState::DraftOnly => (ctx.t("pipelines.draftOnly"), Icon::Pencil, PaletteColor::TextMuted),
    VersionState::Published(v) => (
      ctx.t_args("pipelines.published", [("version", v.get().to_string())]),
      Icon::CircleCheck,
      PaletteColor::Success,
    ),
    VersionState::PublishedWithDraft(v) => (
      ctx.t_args("pipelines.withDraft", [("version", v.get().to_string())]),
      Icon::CircleCheck,
      PaletteColor::Success,
    ),
  };
  let tasks = if entry.used_by == 0 {
    ctx.t("pipelines.notUsed")
  } else {
    ctx.t_args("pipelines.tasks", [("count", entry.used_by.to_string())])
  };
  let meta = match entry.last_published {
    Some(at) => ctx.t_args(
      "pipelines.rowMeta",
      [
        ("tasks", tasks.to_string()),
        ("age", super::library::age(ctx, at).to_string()),
      ],
    ),
    None => tasks,
  };
  Column::new()
    .id(format!("{id}-state"))
    .width(lurq::node::dimension::Dimension::Px(STATE_WIDTH))
    .align_items(Alignment::End)
    .spacing(ExtraSpacing::Tight.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Xs)
        .align_items(Alignment::Center)
        .child(icon(glyph, IconSize::Sm, color))
        .child(Text::new(&words).variant(GroupText::Value.role()).nowrap()),
    )
    .child(Text::new(&meta).variant(GroupText::Hint.role()).nowrap())
}

/// The state column's width (`State` `T5CHp`).
const STATE_WIDTH: f32 = 170.0;
/// The row menu's width (`Row menu` `v8j3h`).
const MENU_WIDTH: f32 = 200.0;
