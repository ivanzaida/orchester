//! The head's Import from project and Add tool with its menu (`Create a tool` in 04): a command tool opens a new
//! draft; the other kinds say what they need first.
use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Placement, Popup, Row, Text},
  core::{ElementRef, Signal},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::id::ProjectId;

use crate::{
  navigation::Navigation,
  routes::{ImportTab, ProjectPage, Route},
  theme::ExtraSpacing,
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
    menu::{MenuItem, menu_panel},
    menu_button::MenuButton,
  },
};

/// The menu's width (`Tc6qI`).
const MENU_WIDTH: f32 = 392.0;

/// The head's actions on the Tools tab.
pub(super) fn actions(ctx: &mut Ctx, project: ProjectId, (menu, anchor): (&Signal<bool>, &ElementRef)) -> Vec<Element> {
  let nav = Navigation::of(ctx);
  let import = ActionButton::new("tools-import", ButtonKind::Ghost, ctx.t("tools.import"))
    .icon(Icon::Download)
    .opens(nav.to(Route::Project(project, ProjectPage::Import(ImportTab::Tools))));
  let panel = panel(ctx, project, menu);
  let add = Row::new()
    .align_items(Alignment::Center)
    .child(MenuButton::new(
      "tools-add",
      (Icon::Plus, ctx.t("tools.add")),
      menu.clone(),
      anchor.clone(),
    ))
    .child(
      Popup::new(anchor.clone(), panel)
        .open(menu.clone())
        .placement(Placement::BottomEnd)
        .offset(0.0, ExtraSpacing::Snug.value()),
    );
  vec![import.into(), add.into()]
}

fn panel(ctx: &mut Ctx, project: ProjectId, menu: &Signal<bool>) -> Column {
  let nav = Navigation::of(ctx);
  let command = MenuItem::opens(
    "tools-add-command",
    ctx.t("tools.command"),
    nav.to(Route::Project(project, ProjectPage::NewTool)),
  )
  .icon(Icon::Terminal)
  .detail(ctx.t("tools.newDetail"))
  .render(ctx, menu);
  let others = [
    ("tools.http", Icon::Globe, "tools.menuSecretsReason"),
    ("tools.databaseQuery", Icon::Database, "tools.menuSecretsReason"),
    ("tools.importMcp", Icon::Plug, "tools.menuSecretsReason"),
    ("tools.pullRequest", Icon::GitPullRequest, "tools.menuPushReason"),
  ];
  let mut panel = menu_panel("tools-add-menu")
    .width(Dimension::Px(MENU_WIDTH))
    .child(
      Column::new()
        .width(Dimension::Pct(100.0))
        .padding(SpacingSize::Sm)
        .spacing(ExtraSpacing::Tight.role())
        .child(Text::new(&ctx.t("tools.menuTitle")).variant(TypographyStyle::Label))
        .child(Text::new(&ctx.t("tools.menuDetail")).variant(TypographyStyle::Caption)),
    )
    .child(command);
  for (key, icon, reason) in others {
    panel = panel.child(
      MenuItem::unavailable(
        format!("tools-add-{}", key.replace('.', "-")),
        ctx.t(key),
        ctx.t(reason),
      )
      .icon(icon)
      .render(ctx, menu),
    );
  }
  panel
}
