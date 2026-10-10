//! Library and Add tool menu (`vBImC`, `aaHkr`, `mxgkm`).
use std::sync::Arc;

use lurq::{
  app::{
    component::{Component, DevtoolsInspectable},
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text, VirtualizedList},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::{id::ProjectId, resource::ToolAction};
use orchester_workspace::tools::data::{ProjectTools, ToolEntry};

use super::words;
use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{ProjectPage, Route, ToolTab},
  theme::{ConnectionsSpacing, ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    inspector::note,
    menu::{MenuItem, menu_panel},
    menu_button::MenuButton,
    state::EmptyState,
    table::HEADER_HEIGHT,
    tool_row::{self, ToolRow},
    unavailable::reason_line,
  },
};

#[derive(Clone, PartialEq)]
pub struct Props {
  pub project: ShownProject,
  pub listed: ProjectTools,
}
impl DevtoolsInspectable for Props {}
pub struct Library {
  menu: Signal<bool>,
  anchor: ElementRef,
}
impl Component for Library {
  type Props = Props;
  fn create(ctx: &mut Ctx) -> Self {
    Self {
      menu: ctx.signal(false),
      anchor: ctx.element_ref(),
    }
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<Props>().clone();
    let project = props.project.open.project.id();
    let page = Column::new()
      .id("tools-library")
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .padding(
        Padding::new()
          .top(ExtraSpacing::Roomy.role())
          .left(ConnectionsSpacing::Page.role())
          .right(ConnectionsSpacing::Page.role()),
      )
      .spacing(SpacingSize::Lg)
      .child(self.header(ctx, &props));
    let ownership = note(
      "tools-ownership",
      Icon::Info,
      &ctx.t_args(
        "tools.ownership",
        [("project", props.project.open.project.name().as_str())],
      ),
    );
    if props.listed.tools.is_empty() {
      let nav = Navigation::of(ctx);
      return page
        .child(
          EmptyState::new(
            "tools-empty",
            Icon::Wrench,
            ctx.t("tools.emptyTitle"),
            ctx.t("tools.emptyDetail"),
          )
          .action(
            ActionButton::new("tools-create-command", ButtonKind::Primary, ctx.t("tools.command"))
              .icon(Icon::Terminal)
              .on_click(move || nav.go(Route::Project(project, ProjectPage::NewTool))),
          )
          .child(reason_line("tools-other-kinds", &ctx.t("tools.otherKinds"))),
        )
        .child(ownership);
    }
    page
      .child(
        Row::new()
          .spacing(ExtraSpacing::Loose.role())
          .align_items(Alignment::Center)
          .child(
            ActionButton::new("tools-type-filter", ButtonKind::Secondary, ctx.t("tools.allTypes"))
              .unavailable(ctx.t("tools.filterUnavailable")),
          )
          .child(reason_line("tools-filter-reason", &ctx.t("tools.filterUnavailable"))),
      )
      .child(table(ctx, project, &props.listed))
      .child(ownership)
  }
}
impl Library {
  fn header(&self, ctx: &mut Ctx, props: &Props) -> Row {
    let panel = self.menu(ctx, props.project.open.project.id());
    Row::new()
      .width(Dimension::Pct(100.0))
      .spacing(SpacingSize::Lg)
      .align_items(Alignment::End)
      .child(
        Column::new()
          .flex(1.0)
          .spacing(ExtraSpacing::Snug.role())
          .child(Text::new(&ctx.t("tools.title")).variant(ExtraTypography::Display.role()))
          .child(
            Text::new(&ctx.t_args(
              "tools.description",
              [("project", props.project.open.project.name().as_str())],
            ))
            .variant(ExtraTypography::Prose.role()),
          ),
      )
      .child(
        ActionButton::new("tools-import", ButtonKind::Secondary, ctx.t("tools.import"))
          .icon(Icon::Download)
          .unavailable(ctx.t("tools.importUnavailable")),
      )
      .child(MenuButton::new(
        "tools-add",
        (Icon::Plus, ctx.t("tools.add")),
        self.menu.clone(),
        self.anchor.clone(),
      ))
      .child(
        Popup::new(self.anchor.clone(), panel)
          .open(self.menu.clone())
          .placement(Placement::BottomEnd)
          .offset(0.0, ExtraSpacing::Snug.value()),
      )
  }
  fn menu(&self, ctx: &mut Ctx, project: ProjectId) -> Column {
    // Add menu `c7vkR`: 340 wide.
    const MENU_WIDTH: f32 = 340.0;
    let nav = Navigation::of(ctx);
    let command = MenuItem::new("tools-add-command", ctx.t("tools.command"), move || {
      nav.go(Route::Project(project, ProjectPage::NewTool))
    })
    .icon(Icon::Terminal)
    .detail(ctx.t("tools.newDetail"))
    .render(ctx, &self.menu);
    let others = [
      ("tools.http", Icon::Globe),
      ("tools.mcp", Icon::Server),
      ("tools.database", Icon::Database),
      ("tools.pullRequest", Icon::GitPullRequest),
    ];
    let mut panel = menu_panel("tools-add-menu")
      .width(Dimension::Px(MENU_WIDTH))
      .child(Text::new(&ctx.t("tools.menuTitle")).variant(TypographyStyle::Title))
      .child(Text::new(&ctx.t("tools.menuDetail")).variant(ExtraTypography::Note.role()))
      .child(command);
    for (key, icon) in others {
      panel = panel.child(
        MenuItem::unavailable(
          format!("tools-add-{}", key.replace('.', "-")),
          ctx.t(key),
          ctx.t("tools.otherKinds"),
        )
        .icon(icon)
        .render(ctx, &self.menu),
      );
    }
    panel
  }
}

fn table(ctx: &mut Ctx, project: ProjectId, listed: &ProjectTools) -> Column {
  let count = listed.tools.len();
  let words = [
    "tools.columns.tool",
    "tools.columns.kind",
    "tools.columns.secrets",
    "tools.columns.version",
    "tools.columns.usedBy",
    "tools.columns.status",
  ]
  .map(|k| ctx.t(k));
  let list = listed.clone();
  let rows = VirtualizedList::new_shared(ctx, listed.tools.clone())
    .list_key("tools-rows")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(tool_row::ROW_HEIGHT * count as f32))
    .mount_keyed::<LibraryRow, _, _, _>(
      |e| e.id.to_string(),
      move |e| RowProps {
        project,
        entry: e.clone(),
        listed: list.clone(),
      },
    );
  Column::new()
    .id("tools-list")
    .width(Dimension::Pct(100.0))
    .flex(1.0)
    .max_height(Dimension::Px(
      HEADER_HEIGHT + tool_row::ROW_HEIGHT * count as f32 + HEADER_HEIGHT,
    ))
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Lg)
    .clip()
    .child(tool_row::header("tools-list-header", &words))
    .child(rows)
    .child(
      Row::new()
        .padding(Padding::symmetric(SpacingSize::Md, ExtraSpacing::Loose.role()))
        .child(
          Text::new(&ctx.t_args("tools.count", [("count", count.to_string())])).variant(ExtraTypography::Note.role()),
        ),
    )
}
#[derive(Clone, PartialEq)]
struct RowProps {
  project: ProjectId,
  entry: ToolEntry,
  listed: ProjectTools,
}
impl DevtoolsInspectable for RowProps {}
struct LibraryRow;
impl Component for LibraryRow {
  type Props = RowProps;
  fn create(_ctx: &mut Ctx) -> Self {
    Self
  }
  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let p = ctx.props::<RowProps>().clone();
    let (key, glyph) = words::kind(&p.entry.tool);
    let count = p.listed.used_by(p.entry.id).count();
    let used = if count == 0 {
      ctx.t("tools.unused")
    } else {
      ctx.t_args("tools.usedSteps", [("count", count.to_string())])
    };
    let secrets = ctx.t(if p.entry.tool.access().is_some() {
      "tools.needsSecret"
    } else {
      "tools.noSecrets"
    });
    let status = ctx.t(if !p.entry.runnable() {
      "tools.unavailable"
    } else if p.entry.state.published().is_some() {
      "tools.ready"
    } else {
      "tools.draft"
    });
    let detail = match p.entry.tool.action() {
      ToolAction::Command(c) => c.command_line(),
      _ => p.entry.tool.description().to_owned(),
    };
    let nav = Navigation::of(ctx);
    ToolRow {
      id: format!("tool-row-{}", p.entry.id),
      glyph,
      name: p.entry.name.clone(),
      detail: detail.into(),
      detail_mono: matches!(p.entry.tool.action(), ToolAction::Command(_)),
      cells: vec![
        Text::new(&ctx.t(key)).variant(ExtraTypography::Property.role()).into(),
        Row::new()
          .spacing(ExtraSpacing::Snug.role())
          .align_items(Alignment::Center)
          .child(icon(Icon::KeyRound, IconSize::Sm, PaletteColor::TextMuted))
          .child(
            Text::new(&secrets)
              .variant(ExtraTypography::Property.role())
              .color(PaletteColor::TextMuted),
          )
          .into(),
        Text::new(&words::version(ctx, &p.entry))
          .variant(ExtraTypography::Property.role())
          .into(),
        Text::new(&used).variant(ExtraTypography::Property.role()).into(),
        Row::new()
          .spacing(ExtraSpacing::Pair.role())
          .align_items(Alignment::Center)
          .child(icon(
            if p.entry.runnable() && p.entry.state.published().is_some() {
              Icon::CircleCheck
            } else if p.entry.runnable() {
              Icon::CircleDot
            } else {
              Icon::Lock
            },
            IconSize::Base,
            if p.entry.runnable() && p.entry.state.published().is_some() {
              PaletteColor::Success
            } else {
              PaletteColor::TextMuted
            },
          ))
          .child(
            Text::new(&status)
              .variant(TypographyStyle::Label)
              .color(PaletteColor::TextSecondary),
          )
          .into(),
      ],
      on_open: Arc::new(move || {
        nav.go(Route::Project(
          p.project,
          ProjectPage::Tool(p.entry.id, ToolTab::Configuration),
        ))
      }),
    }
  }
}
