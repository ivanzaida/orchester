//! Keyboard use of the desktop: only form and dialog controls take focus. Tab and Shift+Tab
//! move through them in the order they appear, wrapping at either end and skipping what cannot
//! be used; navigation, tabs, lists, toolbars, page actions and the status bar are pointer-only.
//! Enter and Space press the focused control, and Enter in a single-line field submits only
//! when the form can be submitted.

mod agents;
mod dialogs;
mod draft_autosave;
mod harness;
mod new_task;
mod pipeline_editor;
mod project_setup;
mod selection_focus;
mod window;
