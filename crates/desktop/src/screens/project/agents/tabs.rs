//! The profile editor's tabs (`A7phQ`): Profile, Permissions and Used in pipelines with its count,
//! and at the right a one-line summary of the permissions that stays visible on every tab. Above
//! the tab content, what keeps the permissions from being published, and what a connection change
//! reset.

use std::sync::Arc;

use lurq::{
  app::{
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, TypographyStyle},
  },
  components::{Column, Row, Stack, Text},
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension},
};
use orchester_core::{
  connection::invocation::Platform,
  permission::capability::{EnforcedPermissions, PermissionProblem},
};

use super::{Editing, editor::ProfileEditor};
use crate::{
  agents::permissions::{Length, enforced_summary, option_title, problem},
  theme::{ExtraColor, ExtraSpacing},
  ui::{
    button::ActionButton,
    icon::{Icon, IconSize, icon},
    notice::Notice,
    tab::Tab,
  },
};

/// Tab strip and tab heights (`A7phQ`, `SOW6S`): the tabs sit at the bottom of the strip.
const STRIP_HEIGHT: f32 = 48.0;
const TAB_HEIGHT: f32 = 40.0;

/// The tab the editor shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorTab {
  Profile,
  Permissions,
  Usage,
}

impl lurq::app::component::DevtoolsInspectable for EditorTab {}

impl ProfileEditor {
  pub(super) fn tab_strip(&self, ctx: &Ctx) -> Element {
    let current = self.tab.get();
    let tab = |id: &'static str, key: &str, tab: EditorTab| {
      let select = self.tab.clone();
      Tab::new(id, ctx.t(key), Dimension::Px(TAB_HEIGHT))
        .selected(current == tab)
        .on_click(move || select.set(tab))
    };
    let summary = Row::new()
      .id("agent-permission-summary")
      .height(Dimension::Px(TAB_HEIGHT))
      .spacing(ExtraSpacing::Tight.role())
      .align_items(Alignment::Center)
      .child(icon(Icon::Shield, IconSize::Small, PaletteColor::TextMuted))
      .child(
        Text::new(&self.permission_summary(ctx))
          .variant(TypographyStyle::Caption)
          .nowrap(),
      );
    let row = Row::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Step.role())
      .align_items(Alignment::End)
      .child(tab(
        "agent-tab-profile",
        "agents.permissions.tabProfile",
        EditorTab::Profile,
      ))
      .child(tab(
        "agent-tab-permissions",
        "agents.permissions.tabPermissions",
        EditorTab::Permissions,
      ))
      .child(tab("agent-tab-usage", "agents.usedInPipelines", EditorTab::Usage).count(self.use_count()))
      .child(Column::new().flex(1.0))
      .child(summary);
    // The hairline is drawn under the tabs, so a selected tab's underline covers it.
    Stack::new()
      .width(Dimension::Pct(100.0))
      .height(Dimension::Px(STRIP_HEIGHT))
      .child(
        Column::new()
          .width(Dimension::Pct(100.0))
          .height(Dimension::Pct(100.0))
          .border_bottom(Border::inside(BorderSize::Sm, ExtraColor::Hairline.role())),
      )
      .child(row)
      .into()
  }

  /// Published pipeline steps that run the profile; none for a new profile.
  fn use_count(&self) -> Option<usize> {
    let Editing::Profile(entry) = &self.context.editing else {
      return None;
    };
    Some(
      self
        .context
        .agents
        .steps
        .iter()
        .filter(|step| step.profile == entry.id)
        .count(),
    )
  }

  /// How the chosen connection enforces the permissions, or why it cannot; `None` without a
  /// connection.
  pub(super) fn enforcement(&self) -> Option<Result<EnforcedPermissions, PermissionProblem>> {
    let connection = self.selected_connection()?;
    Some(
      self
        .permissions
        .get()
        .enforced_on(connection.provider(), Platform::current()),
    )
  }

  /// Why the permissions cannot be published on the chosen connection.
  pub(super) fn permission_problem(&self) -> Option<PermissionProblem> {
    self.enforcement()?.err()
  }

  /// The permissions in one line, as the connection runs them (`haXji`).
  fn permission_summary(&self, ctx: &Ctx) -> Arc<str> {
    let enforcement = self.enforcement();
    let enforced = enforcement.as_ref().and_then(|result| result.as_ref().ok());
    let summary = enforced_summary(ctx, &self.permissions.get(), enforced, Length::Long);
    if self.context.agents.agent_files.is_on() {
      return summary;
    }
    ctx.t_args(
      "agents.permissions.enforcement.repositoryFiles.summary",
      [("summary", &*summary)],
    )
  }

  /// What blocks publishing, and what the last connection change reset.
  pub(super) fn permission_notices(&self, ctx: &Ctx) -> Vec<Element> {
    let mut notices = Vec::new();
    if let (Some(problem), Some(connection)) = (self.permission_problem(), self.selected_connection()) {
      let mut notice = Notice::new(
        Icon::Lock,
        ctx.t("agents.permissions.blockedTitle"),
        self::problem(ctx, &problem, connection.provider()),
      )
      .emphasized()
      .id("agent-permissions-blocked");
      if self.tab.get() != EditorTab::Profile {
        let tab = self.tab.clone();
        notice = notice.action(
          ActionButton::new(ctx.t("agents.permissions.changeConnection"))
            .id("agent-change-connection")
            .in_form()
            .on_click(move || tab.set(EditorTab::Profile))
            .render(ctx),
        );
      }
      notices.push(notice.into());
    }
    let resets = self.resets.get();
    if let (false, Some(connection)) = (resets.0.is_empty(), self.selected_connection()) {
      let options: Vec<Arc<str>> = resets.0.iter().map(|option| ctx.t(option_title(*option))).collect();
      let separator = ctx.t("common.listSeparator");
      notices.push(
        Notice::new(
          Icon::Shield,
          ctx.t_args(
            "agents.permissions.resetTitle",
            [("connection", connection.name().as_str())],
          ),
          ctx.t_args(
            "agents.permissions.resetDetail",
            [("options", options.join(&separator).as_str())],
          ),
        )
        .id("agent-permissions-reset")
        .into(),
      );
    }
    notices
  }
}
