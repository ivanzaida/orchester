//! *Approved for every run* (`12` `wQy6o`), a page under Runs: the rules *Approve for every run of this project* made
//! (the person's decision of 2026-10-18), what each approves, how often it answered and when it was approved, and
//! *Revoke*, after which the tool asks again from its next request on. Revoking only narrows what runs may do, so it
//! takes any input. Only Orchester's `start_qa_app` makes such a rule: *Add rule* stays unavailable with why, and the
//! page says how a rule is added.
use lurq::{
  app::{
    component::Component,
    ctx::Ctx,
    theme::{BorderSize, PaletteColor, RadiusSize, SpacingSize},
  },
  components::{Column, Row, Text},
  core::Signal,
  layout::Alignment,
  node::{Element, border::Border, dimension::Dimension, padding::Padding},
};
use orchester_core::run::ProjectRule;
use orchester_workspace::{display, messages::Message, tasks::outside::Outside};

use super::{
  TabProps,
  kit::{Group, SettingRow, back, page_head},
  page,
  work::{self, Change, Read, Save},
};
use crate::{
  navigation::Navigation,
  routes::{ProjectPage, ProjectSettingsTab, Route},
  theme::{ExtraSpacing, ExtraTypography, IconSize},
  ui::{
    button::{ActionButton, ButtonKind},
    icon::{Icon, icon},
    message::message_text,
    unavailable::reason_line,
  },
};

pub(super) struct Rules {
  save: Save,
  stored: Signal<Option<Read>>,
  revision: Signal<u64>,
  own: work::OwnSaves,
}

impl Component for Rules {
  type Props = TabProps;

  fn create(ctx: &mut Ctx) -> Self {
    let tab = Self {
      save: work::action(ctx, ctx.props::<TabProps>().project.open.project.id()),
      stored: ctx.signal(None),
      revision: ctx.signal(0),
      own: work::OwnSaves::new(ctx, &[Outside::AutoMode]),
    };
    work::follow(
      ctx,
      &tab.save,
      ctx.props::<TabProps>().status.clone(),
      (tab.revision.clone(), tab.stored.clone(), tab.own.clone()),
    );
    tab
  }

  fn render(&self, ctx: &mut Ctx) -> impl Into<Element> {
    let props = ctx.props::<TabProps>().clone();
    let revision = self.revision.get() + self.own.count(&props.project.feed);
    let state = work::load(ctx, props.project.open.repo_root.clone(), revision, &self.own);
    let crumb = back(
      "settings-rules-crumb",
      (
        &ctx.t("project_settings.runs"),
        Some(&ctx.t("project_settings.projectRules")),
      ),
      Navigation::of(ctx).to(Route::Project(
        props.project.open.project.id(),
        ProjectPage::Settings(ProjectSettingsTab::Runs),
      )),
    );
    let head = page_head(
      "settings-rules-head",
      &ctx.t("project_settings.projectRules"),
      Some(&ctx.t("project_settings.projectRulesDetail")),
    );
    let content = Column::new()
      .width(Dimension::Pct(100.0))
      .spacing(ExtraSpacing::Roomy.role())
      .child(crumb)
      .child(head);
    let Some(read) = state.data.clone().or_else(|| self.stored.get()) else {
      return page::scroll(content.child(work::pending(ctx, &state, &self.revision)));
    };
    self.stored.set(Some(read.clone()));
    let rules = self.rules(ctx, &props, &read.0.approval_rules);
    page::scroll(content.child(rules).child(how_added(ctx)))
  }
}

impl Rules {
  /// The rules, each with Revoke, under their count and *Add rule*, unavailable; or that there is none yet.
  fn rules(&self, ctx: &mut Ctx, props: &TabProps, rules: &[ProjectRule]) -> Element {
    let busy = self.save.state().get().is_pending();
    let add = ActionButton::new(
      "settings-rules-add",
      ButtonKind::Ghost,
      ctx.t("project_settings.addRule"),
    )
    .icon(Icon::Plus)
    .compact()
    .unavailable(ctx.t("project_settings.addRuleUnavailable"));
    let count = Text::new(&rules.len().to_string())
      .id("settings-rules-count")
      .variant(crate::theme::ProjectSettingsText::Hint.role());
    let mut group = Group::new("settings-rules", ctx.t("project_settings.rules"))
      .end(count)
      .end(add);
    if rules.is_empty() {
      group = group.row(
        SettingRow::new("settings-rules-none", ctx.t("project_settings.noRules"))
          .hint(ctx.t("project_settings.projectRulesNone"))
          .render(ctx),
      );
    }
    for rule in rules {
      let row = self.rule(ctx, props, rule, busy);
      group = group.row(row);
    }
    if let Some(error) = self.save.state().get().error {
      group = group.after(crate::ui::field::field_error(
        "settings-save-error",
        &message_text(ctx, &error),
      ));
    }
    group.into()
  }

  /// One rule: what it approves, how often it answered and when it was approved, and its *Revoke*.
  fn rule(&self, ctx: &mut Ctx, props: &TabProps, rule: &ProjectRule, busy: bool) -> Element {
    let id = format!("settings-auto-rule-{}", rule.id.get());
    let (month, day) = display::month_day(rule.created_at);
    let month = message_text(ctx, &Message::new(display::MONTH_KEYS[month])).to_string();
    let answered = rule.answered.to_string();
    // lurq's i18n has no plural rules: one answer has words of its own.
    let key = if rule.answered == 1 {
      "project_settings.ruleDetailOnce"
    } else {
      "project_settings.ruleDetail"
    };
    let detail = ctx.t_args(
      key,
      [
        ("script", rule.scope.short_script()),
        ("answered", answered.as_str()),
        ("month", month.as_str()),
        ("day", day.to_string().as_str()),
      ],
    );
    let revoke = ActionButton::new(
      format!("{id}-revoke"),
      ButtonKind::Ghost,
      ctx.t("project_settings.revokeRule"),
    )
    .icon(Icon::Ban)
    .compact()
    .in_form();
    let revoke = if busy {
      revoke.disabled()
    } else {
      let start = work::start(
        &self.save,
        props.project.open.repo_root.clone(),
        Change::Revoke(rule.id),
      );
      revoke.on_click(move || start())
    };
    SettingRow::new(
      id,
      ctx.t_args(
        "project_settings.ruleLabel",
        [("server", rule.scope.server()), ("tool", rule.scope.tool())],
      ),
    )
    .item()
    .small_label()
    .lead(icon(Icon::ShieldCheck, IconSize::Md, PaletteColor::Success))
    .hint(detail)
    .control(revoke)
    .render(ctx)
  }
}

/// How a rule is added (`KzZHm`): the answer a `start_qa_app` request offers in a task's Conversation, as it looks
/// there, and why rules for other tools cannot be added yet.
fn how_added(ctx: &Ctx) -> Element {
  let tile = Column::new()
    .id("settings-rules-answer")
    .width(Dimension::Pct(50.0))
    .padding(Padding::symmetric(
      ExtraSpacing::Loose.role(),
      ExtraSpacing::Inline.role(),
    ))
    .spacing(ExtraSpacing::Fine.role())
    .background(PaletteColor::SurfacePanel)
    .border(Border::inside(BorderSize::Sm, PaletteColor::Border))
    .rounded(RadiusSize::Md)
    .child(
      Row::new()
        .spacing(ExtraSpacing::TextArea.role())
        .align_items(Alignment::Center)
        .child(icon(Icon::ShieldCheck, IconSize::Base, PaletteColor::TextSecondary))
        .child(Text::new(&ctx.t("project_settings.approveForProject")).variant(ExtraTypography::Strong.role())),
    )
    .child(Text::new(&ctx.t("project_settings.approveForProjectCovers")).variant(ExtraTypography::Note.role()));
  Column::new()
    .id("settings-rules-how")
    .width(Dimension::Pct(100.0))
    .padding(Padding::symmetric(SpacingSize::Lg, ExtraSpacing::Inset.role()))
    .spacing(ExtraSpacing::Loose.role())
    .background(PaletteColor::SurfaceRaised)
    .rounded(RadiusSize::Lg)
    .child(Text::new(&ctx.t("project_settings.howRuleAdded")).variant(ExtraTypography::OptionTitle.role()))
    .child(
      Text::new(&ctx.t("project_settings.howRuleAddedBody"))
        .variant(ExtraTypography::Prose.role())
        .width(Dimension::Pct(100.0)),
    )
    .child(tile)
    .child(reason_line(
      "settings-rules-how-reason",
      &ctx.t("project_settings.addRuleUnavailable"),
    ))
    .into()
}
