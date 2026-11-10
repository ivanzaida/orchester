//! *Role* of a local profile's panel (`Role` in L17 `b7Tbxb`), the hub's section ([`crate::screens::hub::role`]) over
//! what the panel holds. What is typed shows at once and is kept as typed; once it can be saved it is a choice made on
//! what the panel holds ([`Edit::Role`]), as a select's is, so *Save* publishes it and *Cancel* puts back what is
//! stored. While it cannot be saved, the field says why and *Save* waits.

use std::sync::Arc;

use lurq::{app::ctx::Ctx, core::Signal, node::Element};
use orchester_workspace::agents::{
  local_edit::Edit,
  local_hub::LocalProfile,
  role::{RoleErrors, RoleText},
};

use crate::screens::hub::{
  profile_form::OnEdit,
  role::{self, Change},
};

/// The role as typed in the panel: `None` until something is typed, and again once *Cancel* puts back what is stored
/// or the hub lists what a save published.
pub(super) type Typed = Signal<Option<RoleText>>;

/// The section: what is typed, else the role of `shown`. `locked` while a save or a cancel is under way.
pub(super) fn section(
  ctx: &mut Ctx,
  typed: &Typed,
  shown: &LocalProfile,
  (on_edit, locked): (OnEdit, bool),
) -> Element {
  let text = typed.get().unwrap_or_else(|| shown.role.clone());
  let change = |set: fn(&mut RoleText, String)| -> Change {
    let (typed, base, edit) = (typed.clone(), text.clone(), on_edit.clone());
    Arc::new(move |value| {
      let mut next = typed.get_untracked().unwrap_or_else(|| base.clone());
      set(&mut next, value);
      if next.errors().is_none() {
        edit(Edit::Role(next.clone()));
      }
      typed.set(Some(next));
    })
  };
  let props = role::Props {
    id: "local-hub-profile-role".to_owned(),
    errors: text.errors(),
    locked,
    on_role: change(|role, value| role.role = value),
    on_responsibilities: change(|role, value| role.responsibilities = value),
    on_instructions: change(|role, value| role.instructions = value),
    name_said: None,
    first: None,
    role: text,
  };
  role::section(ctx, props)
}

/// Why the role as typed cannot be saved, if it cannot.
pub(super) fn refused(typed: &Typed) -> Option<RoleErrors> {
  typed.get().as_ref().and_then(RoleText::errors)
}
