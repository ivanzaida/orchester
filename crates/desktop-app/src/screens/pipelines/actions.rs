//! The page's saves and publishes: *Save draft*, which asks first when the draft would be weaker (*Saving needs your
//! approval*, as `widening` decides), *Publish vN* and the publish review's own question (*Publishing needs your
//! approval*), both in place (`02b` `fXnX2`, `02e` `T46Jq`). Every write carries the exact body reviewed.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::pipelines::{
  widening::{draft_reasons, publish_reasons},
  write::{Allowed, next_version},
};

use super::{
  state::{Editing, State, View},
  work::{Action, Work},
};
use crate::{
  screens::connections::widen::{WidenNotice, WidenProps},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
  },
};

/// What a write needs: the page's state, its action, the project's repository, and whether the save question shows.
#[derive(Clone)]
pub struct Writes {
  pub state: State,
  pub action: Action,
  pub root: std::path::PathBuf,
  pub asking: Signal<bool>,
}

/// The page's writes, provided to the Steps tab's column, which asks the save question in place (`02e` `T46Jq`).
#[derive(Clone)]
pub struct ColumnWrites(pub Writes);

impl lurq::app::component::DevtoolsInspectable for ColumnWrites {}

/// Whether the save question shows in the column rather than under the header: on the Steps tab of the draft.
pub fn asks_in_column(editing: &Editing) -> bool {
  editing.view.tab() == super::state::Tab::Steps && editing.viewing.is_none() && editing.view != View::Publish
}

/// *Saving needs your approval* where the column asks it: under its title.
pub fn column_question(ctx: &mut Ctx) -> Option<Element> {
  let writes = ctx.use_context::<ColumnWrites>()?.0;
  if !writes.asking.get() || !asks_in_column(&writes.state.get()) {
    return None;
  }
  Some(writes.save_question(ctx))
}

impl Writes {
  /// *Save draft* while edits are unsaved; it asks first when the draft would be weaker.
  pub fn save_button(&self, ctx: &Ctx, editing: &Editing, busy: bool) -> Option<ActionButton> {
    if editing.viewing.is_some() || !editing.dirty() {
      return None;
    }
    let writes = self.clone();
    let save = ActionButton::new("pipeline-save", ButtonKind::Secondary, ctx.t("pipelines.save"))
      .icon(Icon::Save)
      .on_click(move || {
        let current = writes.state.get();
        let Ok(body) = current.edit.validate() else {
          return;
        };
        if !draft_reasons(&current.opened, &body).is_empty() {
          writes.asking.set(true);
          return;
        }
        writes.save(current, body, Allowed::default());
      });
    Some(
      if busy || !editing.errors.is_empty() || editing.edit.validate().is_err() {
        save.disabled()
      } else {
        save
      },
    )
  }

  fn save(&self, current: Editing, body: orchester_core::resource::Pipeline, allowed: Allowed) {
    self.action.run(Work::Save {
      root: self.root.clone(),
      name: current.name,
      before: current.opened,
      body,
      allowed,
    });
  }

  /// *Publish vN*, or *Version history* when the published version has no draft.
  pub fn publish_button(&self, ctx: &Ctx, editing: &Editing, busy: bool) -> ActionButton {
    let views = editing.views.clone();
    if editing.opened.latest.is_some() && editing.opened.draft_saved_at.is_none() && !editing.dirty() {
      return ActionButton::new(
        "pipeline-header-history",
        ButtonKind::Secondary,
        ctx.t("pipelines.history"),
      )
      .icon(Icon::History)
      .on_click(move || views.tab(View::Versions));
    }
    let next = next_version(&editing.opened)
      .map(|v| v.get().to_string())
      .unwrap_or_default();
    let publish = ActionButton::new(
      "pipeline-publish",
      ButtonKind::Primary,
      ctx.t_args("pipelines.publish", [("version", next)]),
    )
    .icon(Icon::ArrowUp)
    .on_click(move || views.choose(View::Publish));
    if busy
      || editing.viewing.is_some()
      || editing.view == View::Publish
      || editing.unsaved()
      || editing.opened.draft_saved_at.is_none()
      || !super::publication::valid(editing)
    {
      publish.disabled()
    } else {
      publish
    }
  }

  /// *Saving needs your approval*, with what makes the draft weaker.
  pub fn save_question(&self, ctx: &mut Ctx) -> Element {
    let editing = self.state.get();
    let Ok(body) = editing.edit.validate() else {
      return field_error("pipeline-invalid", &ctx.t("pipelines.error")).into();
    };
    let reasons = draft_reasons(&editing.opened, &body);
    let words = reasons.iter().map(|m| message_text(ctx, m)).collect();
    let (writes, keep) = (self.clone(), self.asking.clone());
    ctx.mount_keyed::<WidenNotice>(
      "pipeline-save-question",
      WidenProps {
        id: "pipeline-save-question".into(),
        card: false,
        affirming: false,
        automation: crate::mcp::input_gate::Guarded::PermissionWidening,
        title: ctx.t("pipelines.saveWeaker"),
        reasons: words,
        confirm: ctx.t("pipelines.save"),
        keep: ctx.t("pipelines.keep"),
        on_confirm: Arc::new(move || {
          let current = writes.state.get();
          if current.edit.validate().as_ref() != Ok(&body) || !current.errors.is_empty() {
            return;
          }
          writes.asking.set(false);
          writes.save(current, body.clone(), Allowed(reasons.clone()));
        }),
        on_keep: Arc::new(move || keep.set(false)),
      },
    )
  }

  /// The publish review in the column, with *Publish vN* and *Cancel*, or the question a weaker version asks.
  pub fn publish_column(&self, ctx: &mut Ctx, busy: bool) -> Element {
    let editing = self.state.get();
    let review = super::publication::review(ctx, &editing);
    let valid = super::publication::valid(&editing);
    if !valid || busy {
      return review.into();
    }
    let version = next_version(&editing.opened).map_or_else(String::new, |v| v.get().to_string());
    let reasons = publish_reasons(editing.opened.latest.as_ref().map(|v| &v.body), &editing.opened.body);
    let before = editing.opened.clone();
    if reasons.is_empty() {
      let writes = self.clone();
      let publish = ActionButton::new(
        "pipeline-publish-confirm",
        ButtonKind::Primary,
        ctx.t_args("pipelines.publish", [("version", version)]),
      )
      .icon(Icon::ArrowUp)
      .on_click(move || writes.publish(before.clone(), Allowed::default()));
      return review
        .child(super::publication::footer(ctx, &self.state, publish))
        .into();
    }
    let words = reasons.iter().map(|m| message_text(ctx, m)).collect();
    let (writes, keep) = (self.clone(), editing.views);
    let question = ctx.mount_keyed::<WidenNotice>(
      "pipeline-publish-question",
      WidenProps {
        id: "pipeline-publish-question".into(),
        card: false,
        affirming: false,
        automation: crate::mcp::input_gate::Guarded::PermissionWidening,
        title: ctx.t("pipelines.publishWeaker"),
        reasons: words,
        confirm: ctx.t("pipelines.publishNow"),
        keep: ctx.t("pipelines.keep"),
        on_confirm: Arc::new(move || {
          let current = writes.state.get();
          if current.opened != before || current.unsaved() {
            return;
          }
          writes.publish(before.clone(), Allowed(reasons.clone()));
        }),
        on_keep: Arc::new(move || keep.close()),
      },
    );
    review.child(question).into()
  }

  fn publish(&self, before: orchester_workspace::pipelines::data::OpenedPipeline, allowed: Allowed) {
    self.action.run(Work::Publish {
      root: self.root.clone(),
      before,
      allowed,
    });
  }
}
