//! The page's saves: *Save draft* and *Publish vN*, both the one save that publishes the next version (a pipeline
//! holds no draft, D-032), which asks first when it would be weaker (*Saving needs your approval*, as `widening`
//! decides), in place (`02e` `T46Jq`). Every write carries the exact body reviewed.
use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::pipelines::{
  widening::save_reasons,
  write::{Allowed, next_version},
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
  },
};

/// What a write needs: the page's state, its action, the project's repository, and whether the save question shows.
#[derive(Clone)]
pub struct Writes {
  pub state: State,
  pub action: Action,
  pub root: std::path::PathBuf,
  pub asking: Signal<bool>,
  /// The project is shared: Publish reads *Publish for the team* (02).
  pub shared: bool,
}

impl Writes {
  /// *Publish vN*, or *Publish for the team* in a shared project.
  fn publish_label(&self, ctx: &Ctx, version: String) -> std::sync::Arc<str> {
    if self.shared {
      crate::screens::hub::team::publish_label(ctx)
    } else {
      ctx.t_args("pipelines.publish", [("version", version)])
    }
  }
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
  /// *Save draft* while edits are unsaved, a field's error among them (`02g` `PexNQ`), disabled until they are
  /// valid; it asks first when the save would be weaker. A pipeline holds no draft (D-032): the save publishes the
  /// next version, or is refused and stores nothing.
  pub fn save_button(&self, ctx: &Ctx, editing: &Editing, busy: bool) -> Option<ActionButton> {
    if editing.viewing.is_some() || !editing.unsaved() {
      return None;
    }
    let writes = self.clone();
    let save = ActionButton::new("pipeline-save", ButtonKind::Secondary, ctx.t("pipelines.save"))
      .icon(Icon::Save)
      .on_click(move || writes.save_edit());
    Some(if busy || !saveable(editing) {
      save.disabled()
    } else {
      save
    })
  }

  /// Saves the edit as it stands, asking first when the save would be weaker.
  fn save_edit(&self) {
    let current = self.state.get();
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

  /// *Publish vN* while edits are unsaved, which saves them as *Save draft* does (one save publishes, D-032), else
  /// *Version history*.
  pub fn publish_button(&self, ctx: &Ctx, editing: &Editing, busy: bool) -> ActionButton {
    let views = editing.views.clone();
    if !editing.dirty() {
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
    let writes = self.clone();
    let publish = ActionButton::new("pipeline-publish", ButtonKind::Primary, self.publish_label(ctx, next))
      .icon(Icon::ArrowUp)
      .on_click(move || writes.save_edit());
    if busy || editing.viewing.is_some() || !saveable(editing) {
      publish.disabled()
    } else {
      publish
    }
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
            if current.edit.validate().as_ref() != Ok(&body) || !current.errors.is_empty() {
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

  /// The publish review in the column. A pipeline holds no draft (D-032), so it has nothing of its own to publish:
  /// *Publish vN* in the header saves the edit.
  pub fn publish_column(&self, ctx: &mut Ctx, _busy: bool) -> Element {
    let editing = self.state.get();
    super::publication::review(ctx, &editing).into()
  }
}

/// Whether the edit can be saved as it stands: every field valid, and the steps keep the pipeline's rules.
fn saveable(editing: &Editing) -> bool {
  editing.errors.is_empty() && editing.edit.validate().is_ok()
}

/// What saving `body` over the latest version needs a person to allow.
fn reasons(
  editing: &Editing,
  body: &orchester_core::resource::Pipeline,
) -> Vec<orchester_workspace::messages::Message> {
  save_reasons(Some(&editing.opened.body), body, &editing.opened.assignables)
}
