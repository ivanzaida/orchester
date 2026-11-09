//! The forms that make a new resource in the hub's panel (A7–A12: [`panel`]), which the head's *New <kind>* opens
//! ([`crate::screens::hub::new_button`]). A form has only Create and Cancel: no draft, Create makes v1
//! (`docs/design/team-configuration.md`, decision 6).

mod fields;
mod instruction;
mod kinds;
mod local_mcp;
mod mcp;
mod mode;
mod notices;
mod panel;
mod profile;
mod skill;
mod tool;
mod work;

pub(super) use self::mcp::{EditView, Fields as McpFields};
pub(in crate::screens) use self::{
  kinds::new_row,
  mode::Mode,
  panel::{Props, panel},
};
