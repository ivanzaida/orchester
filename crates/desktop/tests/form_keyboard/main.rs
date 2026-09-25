//! Keyboard use of the desktop: Tab and Shift+Tab move through the window's controls in the
//! order they appear, passing through a page form like a browser and skipping what cannot be
//! used, Enter and Space press the focused control, and Enter in a single-line field submits
//! only when the form can be submitted.

mod agents;
mod dialogs;
mod draft_autosave;
mod harness;
mod new_task;
mod pipeline_editor;
mod project_setup;
mod window;
