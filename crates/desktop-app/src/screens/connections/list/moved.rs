//! What an address of the Connections tab shows now that the tab is gone (`Agents on accounts` D1 `E6kvpm`): a
//! link kept from before, to the tab's list or to an agent connection's page, says where that went. A project's
//! agents run on the person's accounts, in Settings › Accounts; GitHub is under Project settings › General.

use lurq::{
  app::{
    ctx::Ctx,
    theme::{SpacingSize, TypographyStyle},
  },
  components::{Column, Row, Text},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};

use crate::{
  navigation::Navigation,
  project::ShownProject,
  routes::{AccountsView, ProjectPage, ProjectSettingsTab, Route},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::Icon,
  },
};

pub(super) fn page(ctx: &mut Ctx, project: &ShownProject) -> Element {
  let navigation = Navigation::of(ctx);
  let id = project.open.project.id();
  let accounts = ActionButton::new(
    "connections-moved-accounts",
    ButtonKind::Primary,
    ctx.t("connections.moved.accounts"),
  )
  .icon(Icon::ArrowUpRight)
  .opens(navigation.to(Route::Accounts(AccountsView::default())));
  let general = ActionButton::new(
    "connections-moved-general",
    ButtonKind::Secondary,
    ctx.t("connections.moved.general"),
  )
  .opens(navigation.to(Route::Project(
    id,
    ProjectPage::Settings(ProjectSettingsTab::General),
  )));
  Column::new()
    .id("connections-moved")
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Md)
    .child(
      Text::new(&ctx.t("connections.moved.title"))
        .id("connections-moved-title")
        .variant(TypographyStyle::Title),
    )
    .child(
      Text::new(&ctx.t("connections.moved.body"))
        .id("connections-moved-body")
        .variant(TypographyStyle::Description)
        .width(Dimension::Pct(100.0)),
    )
    .child(
      Row::new()
        .spacing(SpacingSize::Sm)
        .align_items(Alignment::Center)
        .child(accounts)
        .child(general),
    )
    .into()
}
