//! The page's save: one *Save*, which publishes the edit as the next version (a pipeline holds no draft, D-032), and
//! asks first when that would be weaker (*Saving needs your approval*, as `widening` decides), in place (`02e`
//! `T46Jq`). While it cannot save, it says why at the button, as the Pipelines hub's bar does. Every write carries the
//! exact body reviewed.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::{
  messages::Message,
  pipelines::{
    problems::rule_message,
    widening::save_reasons,
    write::{Allowed, refusals},
  },
};

use super::{
  state::{Editing, State, View},
  work::{Action, Work},
};
use crate::{
  screens::connections::widen::{NoticeProps, WidenLook, WidenNotice, WidenProps},
  ui::{
    button::{ActionButton, ButtonKind},
    field::field_error,
    icon::Icon,
    message::message_text,
    tooltip::Tooltip,
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

/// Whether the save question shows in the column rather than under the header: on the Steps tab of the edit.
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
  /// *Save* while the edit or the name is not saved, a field's error among them (`02g` `PexNQ`), else *Version
  /// history*. While the edit cannot be saved, *Save* is unavailable and its tooltip says why ([`refused`]); a viewed
  /// version, which is read only, has nothing to save.
  pub fn save_button(&self, ctx: &mut Ctx, editing: &Editing, busy: bool) -> Element {
    if !editing.unsaved() {
      let views = editing.views.clone();
      return ActionButton::new(
        "pipeline-header-history",
        ButtonKind::Secondary,
        ctx.t("pipelines.history"),
      )
      .icon(Icon::History)
      .on_click(move || views.tab(View::Versions))
      .into();
    }
    let writes = self.clone();
    let save = ActionButton::new("pipeline-save", ButtonKind::Primary, ctx.t("pipelines.save"))
      .icon(Icon::Save)
      .on_click(move || writes.save_edit());
    if busy || editing.viewing.is_some() {
      return save.disabled().into();
    }
    match refused(editing) {
      Some(why) => {
        let why = message_text(ctx, &why);
        Tooltip::new("pipeline-save-why", why.clone()).wrap(ctx, save.unavailable(why))
      }
      None => save.into(),
    }
  }

  /// Saves the edit as it stands, asking first when the save would be weaker.
  fn save_edit(&self) {
    let current = self.state.get();
    if refused(&current).is_some() {
      return;
    }
    let Ok(body) = current.edit.validate() else {
      return;
    };
    if !reasons(&current, &body).is_empty() {
      self.asking.set(true);
      return;
    }
    self.save(current, body, Allowed::default());
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

  /// *Saving needs your approval*, with what makes the save weaker.
  pub fn save_question(&self, ctx: &mut Ctx) -> Element {
    let editing = self.state.get();
    let Ok(body) = editing.edit.validate() else {
      return field_error("pipeline-invalid", &ctx.t("pipelines.error")).into();
    };
    let reasons = reasons(&editing, &body);
    let words = reasons.iter().map(|m| message_text(ctx, m)).collect();
    let (writes, keep) = (self.clone(), self.asking.clone());
    ctx.mount_keyed::<WidenNotice>(
      "pipeline-save-question",
      NoticeProps {
        question: WidenProps {
          id: "pipeline-save-question".into(),
          look: WidenLook::Band,
          affirming: false,
          automation: crate::mcp::input_gate::Guarded::PermissionWidening,
          title: ctx.t("pipelines.saveWeaker"),
          reasons: words,
          confirm: ctx.t("pipelines.save"),
          keep: ctx.t("pipelines.keep"),
          on_confirm: Arc::new(move || {
            let current = writes.state.get();
            if current.edit.validate().as_ref() != Ok(&body) || refused(&current).is_some() {
              return;
            }
            writes.asking.set(false);
            writes.save(current, body.clone(), Allowed(reasons.clone()));
          }),
          on_keep: Arc::new(move || keep.set(false)),
        },
        confirm_icon: Icon::Save,
      },
    )
  }

  /// The review of what *Save* publishes, in the column.
  pub fn publish_column(&self, ctx: &mut Ctx) -> Element {
    let editing = self.state.get();
    super::publication::review(ctx, &editing).into()
  }
}

/// Why the edit cannot be saved as it stands, the first reason: a field's error, a rule the steps break, or what the
/// save check refuses ([`refusals`]: a step not ready, the flow, the version limit, a team pipeline's rename); `None`
/// when it can be saved.
pub fn refused(editing: &Editing) -> Option<Message> {
  if let Some(error) = editing.errors.values().next() {
    return Some(error.clone());
  }
  let body = match editing.edit.validate() {
    Ok(body) => body,
    Err(error) => return Some(rule_message(&editing.edit, &error)),
  };
  refusals(&editing.opened, &body, &editing.name).into_iter().next()
}

/// What saving `body` over the latest version needs a person to allow.
fn reasons(editing: &Editing, body: &orchester_core::resource::Pipeline) -> Vec<Message> {
  save_reasons(Some(&editing.opened.body), body, &editing.opened.assignables)
}
