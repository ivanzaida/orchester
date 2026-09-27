//! A change as a list of changed files beside the selected file's diff, shared by the pull
//! request screen (08.02 `Rrmhf`) and the task page's Changes tab: the file list (`h8PZJa`, files
//! `j9NDM`) and the diff of one file (`rz27k`, lines `vEZ42`). Each screen adds its own toolbar
//! above the diff with what the diff compares.

mod files;
mod line;
mod rows;
mod view;

pub use self::{
  files::{FileEntry, FileGroup, FileList, FileListProps},
  view::{DiffView, DiffViewProps},
};
