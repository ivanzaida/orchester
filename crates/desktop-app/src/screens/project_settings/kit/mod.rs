//! The patterns every tab of Project settings is built from (`Project settings · redesign` `XGtkg`, and its rule for
//! undrawn states in `What changed` `k8s96Q`): a group's heading over a bordered box of rows ([`group`]), a row with
//! its label, one short hint line in its tone and its control at the end ([`row`]), an empty row, the in-place question
//! box, a sub-page's back crumb and head, and a warning line with a terminal command and Copy ([`parts`]). Connections
//! and Secrets, which are tabs of the page, build from them too.

mod form;
mod group;
mod parts;
mod row;

pub(crate) use self::{
  form::{
    FormField, form_actions, form_actions_noted, form_group, form_link, form_note, form_row, on_enter, warning_line,
  },
  group::Group,
  parts::{EmptyRow, Glyph, back, command_line, entity_head, outlined_tile, page_head, question, tile},
  row::{SettingRow, Tone, value},
};
/// A tab's column of groups, scrolling under the page's tabs.
pub(crate) use super::page::scroll;
