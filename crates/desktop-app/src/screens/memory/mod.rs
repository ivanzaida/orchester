//! Memory: the Agents hub's Memory filter ([`hub`], `Section Y · Memory` `Xxl1u`) and the task's Memory tab
//! (`Artifacts & Memory` `g5dmWI`), which share their parts: a record's row, a proposal, the record editor and the
//! delete confirmation are one set; every change happens in place, through the workspace's memory rules
//! (`orchester_workspace::memory`), off the UI thread.

mod centered;
mod delete;
mod editor;
pub(in crate::screens) mod hub;
mod list;
mod no_match;
mod opened;
mod parts;
mod proposal;
mod row;
pub(crate) mod state;
mod tab_no_match;
mod tabs;
mod toolbar;
pub(crate) mod words;
mod work;

pub(crate) use self::{
  centered::Centered,
  list::{ListInput, groups, new_record, row_failure},
  no_match::no_match,
  parts::{group_heading, settings_link, unreadable},
  toolbar::FilterBar,
};
