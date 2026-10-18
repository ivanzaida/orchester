//! The project's approval rules under Auto mode (the person's decision of 2026-10-18): each rule *Approve for every
//! run of this project* made, what it approves and how often it did, and *Revoke*, after which the tool asks again
//! from its next request on. Revoking only narrows what runs may do, so it takes any input.
use lurq::{
  app::{ctx::Ctx, theme::SpacingSize},
  components::{Column, Row},
  layout::Alignment,
  node::{Element, dimension::Dimension},
};
use orchester_core::run::ProjectRule;
use orchester_workspace::project_settings::Settings;

use super::Props;
use crate::{
  screens::project_settings::{
    page,
    work::{self, Change, Save},
  },
  ui::{
    button::{ActionButton, ButtonKind},
    form_section::Place,
    icon::Icon,
  },
};

pub fn row(ctx: &Ctx, props: &Props, settings: &Settings, save: &Save) -> Element {
  let busy = save.state().get().is_pending();
  let mut control = Column::new()
    .id("settings-auto-rules")
    .width(Dimension::Px(page::CONTROL_WIDTH))
    .spacing(SpacingSize::Sm);
  if settings.approval_rules.is_empty() {
    control = control.child(page::note(
      "settings-auto-rules-none",
      &ctx.t("project_settings.projectRulesNone"),
    ));
  }
  for rule in &settings.approval_rules {
    control = control.child(entry(ctx, props, rule, (save, busy)));
  }
  page::row(
    ctx,
    "settings-auto-rules-row",
    ("project_settings.projectRules", "project_settings.projectRulesDetail"),
    control.into(),
    Place::Last,
  )
}

/// One rule: what it approves, and its *Revoke*.
fn entry(ctx: &Ctx, props: &Props, rule: &ProjectRule, (save, busy): (&Save, bool)) -> Element {
  let id = format!("settings-auto-rule-{}", rule.id.get());
  let answered = rule.answered.to_string();
  // lurq's i18n has no plural rules: one answer has words of its own.
  let key = if rule.answered == 1 {
    "project_settings.projectRuleOnce"
  } else {
    "project_settings.projectRule"
  };
  let words = ctx.t_args(
    key,
    [
      ("server", rule.scope.server()),
      ("tool", rule.scope.tool()),
      ("script", rule.scope.short_script()),
      ("answered", answered.as_str()),
    ],
  );
  let revoke = ActionButton::new(
    format!("{id}-revoke"),
    ButtonKind::Secondary,
    ctx.t("project_settings.revokeRule"),
  )
  .icon(Icon::Ban)
  .in_form();
  let revoke = if busy {
    revoke.disabled()
  } else {
    let start = work::start(save, props.tab.project.open.repo_root.clone(), Change::Revoke(rule.id));
    revoke.on_click(move || start())
  };
  Row::new()
    .id(id.clone())
    .width(Dimension::Pct(100.0))
    .spacing(SpacingSize::Sm)
    .align_items(Alignment::Center)
    .child(
      Column::new()
        .flex(1.0)
        .child(page::note(&format!("{id}-words"), &words)),
    )
    .child(revoke)
    .into()
}
