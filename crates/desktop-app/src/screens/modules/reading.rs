//! A module's page while it is read (`06 · Module · loading` `e0ZdI` in `Modules · redesign` `oEELJ`): the crumb's way
//! back, New task in module, the tab names, the section headings and the property names stay real; the module's name,
//! its state and facts, *More actions*, its description, its tasks and the burndown line are bones in the Summary's own
//! geometry, the lower rows fading. *Reading the module…* (`modules.page.reading`) names it.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{PaletteColor, SpacingSize, TypographyStyle},
  },
  components::{Column, Row, ScrollVertical, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::{ModuleId, ProjectId};

use super::{header, page::body_padding, properties};
use crate::{
  navigation::Navigation,
  routes::{ModuleListTab, ModuleTab, ProjectPage, Route},
  theme::{ExtraRadius, ExtraSpacing, IconSize, TaskSpacing, TaskText},
  ui::{
    breadcrumb::{Crumb, CrumbSeparator, breadcrumb},
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    pressable::opening,
    skeleton::{self, Skeleton, bone, fading},
    tabs::{PageTab, page_tabs},
  },
};

/// The name's bone and the facts' (`e0ZdI`), the state chip's, the crumb's, and *More actions*' square.
const NAME: f32 = 218.0;
const CHIP: (f32, f32) = (48.0, 20.0);
const FACTS: f32 = 434.0;
const CRUMB: f32 = 88.0;
const MORE: f32 = 28.0;
/// The tasks' lines: a group head's label, or a task's key and title.
const TASKS: [(bool, f32); 6] = [(true, 76.0), (false, 262.0), (true, 76.0), (false, 306.0), (true, 54.0), (false, 218.0)];

/// The header and the body of module `module`'s page on `tab` while it is read.
pub(super) fn page(ctx: &mut Ctx, (project, module): (ProjectId, ModuleId), tab: ModuleTab) -> (Element, Element) {
  let navigation = Navigation::of(ctx);
  let new_task = ActionButton::new("module-new-task", ButtonKind::Primary, ctx.t("modules.page.newTask"))
    .icon(Icon::Plus)
    .opens(navigation.to(Route::Project(project, ProjectPage::NewTaskInModule(module))));
  let tabs: Vec<PageTab> = ModuleTab::ALL
    .into_iter()
    .map(|shown| {
      let label = ctx.t(match shown {
        ModuleTab::Summary => "modules.page.tabs.summary",
        ModuleTab::Burndown => "modules.page.tabs.burndown",
      });
      let open = navigation.to(Route::Project(project, ProjectPage::Module(module, shown)));
      PageTab::to(format!("module-tab-{}", shown.name()), label, shown == tab, open)
    })
    .collect();
  let tabs = page_tabs(ctx, "module-tabs", tabs).into();
  let status = header::status_line(
    "module-status-row",
    vec![
      bone(CHIP.0, CHIP.1).rounded(ExtraRadius::Full.role()).into(),
      skeleton::line(ctx, TypographyStyle::Description, FACTS).into(),
    ],
  );
  let top = header::header(header::Header {
    id: "module-header",
    back: Some(back(ctx, project)),
    title: skeleton::line(ctx, TypographyStyle::Heading, NAME).into(),
    actions: vec![new_task.into(), skeleton::button(MORE).into()],
    status,
    tabs,
  });
  let summary = Row::new()
    .width(Dimension::Pct(100.0))
    .padding(body_padding())
    .spacing(TaskSpacing::Columns.role())
    .align_items(Alignment::Start)
    .child(column(ctx))
    .child(properties::bones(ctx));
  let body = Skeleton::new("module-loading", ctx.t("modules.page.reading")).show(ctx, summary);
  (top, ScrollVertical::new(body).id("module-scroll").flex(1.0).into())
}

/// `← Modules / ` and the name's bone.
fn back(ctx: &mut Ctx, project: ProjectId) -> Element {
  let navigation = Navigation::of(ctx);
  let list = Route::Project(project, ProjectPage::Modules(ModuleListTab::default()));
  let crumbs = vec![
    Crumb::to(ctx.t("modules.page.back"), navigation.to(list.clone())),
    Crumb::here(String::new()),
  ];
  let arrow = opening(navigation.to(list))
    .id("module-back")
    .describe("a11y_name", ctx.t("modules.page.back"))
    .child(icon(Icon::ArrowLeft, IconSize::Base, PaletteColor::TextMuted));
  Row::new()
    .id("module-back-row")
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(arrow)
    .child(breadcrumb(ctx, "module-crumbs", CrumbSeparator::Slash, crumbs))
    .child(skeleton::line(ctx, TypographyStyle::Description, CRUMB))
    .into()
}

/// The Summary's column: Description and its lines, Tasks with its count and its rows, and the burndown line.
fn column(ctx: &Ctx) -> Column {
  let head = |key: &str| Text::new(&ctx.t(key)).variant(TaskText::SectionTitle.role()).nowrap();
  let description = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(head("modules.summary.description"))
    .child(skeleton::line(ctx, TaskText::Brief.role(), 828.0))
    .child(skeleton::line(ctx, TaskText::Brief.role(), 566.0));
  let rows = TASKS
    .iter()
    .map(|(group, title)| {
      let line = Row::new()
        .width(Dimension::Pct(100.0))
        .padding(lurq::node::padding::Padding::horizontal(SpacingSize::Sm))
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(bone(14.0, 14.0));
      if *group {
        line
          .height(Dimension::Px(GROUP_HEIGHT))
          .child(bone(14.0, 14.0))
          .child(skeleton::line(ctx, TypographyStyle::Description, *title))
      } else {
        line
          .height(Dimension::Px(ROW_HEIGHT))
          .child(skeleton::line(ctx, TypographyStyle::Description, 48.0))
          .child(skeleton::line(ctx, crate::theme::ExtraTypography::Strong.role(), *title))
          .child(lurq::components::Spacer::new().flex(1.0))
          .child(skeleton::count())
      }
    })
    .collect();
  let tasks = Column::new()
    .width(Dimension::Pct(100.0))
    .spacing(ExtraSpacing::Loose.role())
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(head("modules.page.tasks"))
        .child(skeleton::count()),
    )
    .child(Column::new().width(Dimension::Pct(100.0)).with_children(fading(rows)));
  Column::new()
    .flex(1.0)
    .min_width(Dimension::Px(0.0))
    .spacing(TaskSpacing::Sections.role())
    .child(description)
    .child(tasks)
    .child(skeleton::line(ctx, TypographyStyle::Description, 434.0).opacity(crate::theme::SKELETON_FADE[2]))
}

/// A task group's head and a task's row, as the module's task list draws them.
const GROUP_HEIGHT: f32 = 34.0;
const ROW_HEIGHT: f32 = 40.0;
