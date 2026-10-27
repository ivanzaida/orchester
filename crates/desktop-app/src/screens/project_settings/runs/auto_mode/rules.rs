//! *Approved for every run* (`AJzh2` in `02 · Runs`): how many rules *Approve for every run of this project* made, what
//! they approve, and the row opens their page with Revoke (`12` `wQy6o`, [`crate::screens::project_settings::rules`]).
use lurq::{
  app::{ctx::Ctx, theme::PaletteColor},
  components::Row,
  layout::Alignment,
  node::Element,
};
use orchester_workspace::project_settings::Settings;

use super::Props;
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, Route},
  screens::project_settings::kit::{SettingRow, value},
  theme::{ExtraSpacing, IconSize},
  ui::icon::{Icon, icon},
};

pub fn row(ctx: &mut Ctx, props: &Props, settings: &Settings) -> Element {
  let count = settings.approval_rules.len();
  let (hint, counted) = match count {
    0 => (
      ctx.t("project_settings.projectRulesNone"),
      ctx.t("project_settings.noRules"),
    ),
    1 => (
      ctx.t("project_settings.projectRulesHint"),
      ctx.t("project_settings.ruleCount.one"),
    ),
    count => (
      ctx.t("project_settings.projectRulesHint"),
      ctx.t_args("project_settings.ruleCount.other", [("count", count.to_string())]),
    ),
  };
  let open = Navigation::of(ctx).to(Route::Project(
    props.tab.project.open.project.id(),
    ProjectPage::ProjectRules,
  ));
  let shown = Row::new()
    .spacing(ExtraSpacing::Snug.role())
    .align_items(Alignment::Center)
    .child(value("settings-auto-rules-count", &counted))
    .child(icon(Icon::ChevronRight, IconSize::Md, PaletteColor::TextMuted));
  SettingRow::new("settings-auto-rules", ctx.t("project_settings.projectRules"))
    .info(ctx.t("project_settings.projectRulesDetail"))
    .hint(hint)
    .control(shown)
    .opens(open)
    .render(ctx)
}
