//! A hosted reviewer is chosen only after the person answers the destination question (04 b, B7tgr).
use std::sync::Arc;

use lurq::{app::ctx::Ctx, node::Element};
use orchester_core::run::review::AutoModeSettings;
use orchester_workspace::project_settings::{Settings, decision_reviewer};

use super::{Asked, AutoMode, Props};
use crate::{
  mcp::input_gate::Guarded,
  screens::connections::widen::{WidenConfirm, WidenLook, WidenProps},
};

impl AutoMode {
  pub(super) fn needs_hosted_question(&self, settings: &Settings, asked: &AutoModeSettings) -> bool {
    decision_reviewer::choosing_hosted(&settings.auto_mode, asked, &settings.decision_connections).is_some()
      && self.hosted_confirmed.get_untracked().0.as_ref() != Some(asked)
  }

  pub(super) fn hosted_question(&self, ctx: &mut Ctx, props: &Props) -> Option<Element> {
    let settings = self.stored.get().0;
    // Invalid drafts are explained by their field, without a destination question.
    let asked = self.draft()?;
    if !self.needs_hosted_question(&settings, &asked) {
      return None;
    }
    let destination = decision_reviewer::choosing_hosted(&settings.auto_mode, &asked, &settings.decision_connections)?;
    let connection = self.selected_decision(&settings)?;
    let old = decision_reviewer::row(
      &settings.auto_mode,
      &settings.connections,
      &settings.decision_connections,
    );
    let keep_name = old
      .connection
      .unwrap_or_else(|| ctx.t("project_settings.default").to_string());
    let name = connection.name().as_str();
    let displayed = connection.clone();
    let (confirm, keep, props) = (self.clone(), self.clone(), props.clone());
    let threshold = format!("{:.2}", asked.reviewer.as_ref()?.threshold?.value());
    Some(
      ctx.mount_keyed::<WidenConfirm>(
        "settings-reviewer-hosted-question",
        WidenProps {
          id: "settings-reviewer-hosted".into(),
          look: WidenLook::Reviewer,
          affirming: false,
          automation: Guarded::WideningQuestion,
          title: ctx.t_args(
            "project_settings.decision.question",
            [("host", destination.host.as_str())],
          ),
          reasons: vec![
            ctx.t_args(
              "project_settings.decision.sends",
              [("name", name), ("threshold", threshold.as_str())],
            ),
            ctx.t("project_settings.decision.personOnly"),
          ]
          .into(),
          confirm: ctx.t_args("project_settings.decision.use", [("name", name)]),
          keep: ctx.t_args("project_settings.decision.keep", [("name", keep_name.as_str())]),
          on_confirm: Arc::new(move || {
            if lurq::app::events::is_synthetic_input() || confirm.outside() {
              return;
            }
            // Bind consent to exactly the destination, model and threshold that the question showed.
            if confirm.draft().is_some_and(|draft| draft == asked)
              && confirm.selected_decision(&confirm.stored.get_untracked().0) == Some(&displayed)
            {
              confirm.note_input();
              confirm.hosted_confirmed.set(Asked(Some(asked.clone())));
              confirm.request(&props, &confirm.save, asked.clone());
            }
          }),
          on_keep: Arc::new(move || {
            if lurq::app::events::is_synthetic_input() {
              return;
            }
            keep.hosted_confirmed.set(Asked(None));
            keep.attempted.set(Asked(None));
            keep.fill(keep.stored.get_untracked().0);
          }),
        },
      ),
    )
  }
}
