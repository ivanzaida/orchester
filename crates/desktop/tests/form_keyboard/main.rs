//! Keyboard use of the desktop: only form and dialog controls take focus. Tab and Shift+Tab
//! move through them in the order they appear, wrapping at either end and skipping what cannot
//! be used; navigation, tabs, lists, toolbars, page actions and the status bar are pointer-only.
//! Enter and Space press the focused control, and Enter in a single-line field submits only
//! when the form can be submitted. The mouse's side buttons and the history shortcuts go back
//! and forward through the screens.

mod agent_permissions;
mod agents;
mod changes;
mod claude_run;
mod dialogs;
mod draft_autosave;
mod github;
mod harness;
mod history;
mod instructions;
mod lifecycle;
mod new_task;
mod pipeline_editor;
mod project_settings;
mod project_setup;
mod pull_request_detail;
mod pull_requests;
mod run_permissions;
mod secrets;
mod selection_focus;
mod task_graph;
mod task_run;
mod tool_run;
mod tools;
mod window;
