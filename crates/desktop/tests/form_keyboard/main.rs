//! Keyboard use of the desktop: only form and dialog controls take focus. Tab and Shift+Tab
//! move through them in the order they appear, wrapping at either end and skipping what cannot
//! be used; navigation, tabs, lists, toolbars, page actions and the status bar are pointer-only.
//! Enter and Space press the focused control, and Enter in a single-line field submits only
//! when the form can be submitted. The mouse's side buttons and the history shortcuts go back
//! and forward through the screens.

mod agent_permissions;
mod agents;
mod all_projects;
mod canvas_zoom;
mod changes;
mod claude_run;
mod code_index;
mod command_approvals;
mod conversation;
mod dialogs;
mod draft_autosave;
mod edit_task;
mod first_edit;
mod github;
mod harness;
mod history;
mod import;
mod instructions;
mod integration;
mod lifecycle;
mod mcp_exempt_tools;
mod mcp_servers;
mod modal_layer;
mod native_connections;
mod new_task;
mod pipeline_editor;
mod pipeline_history;
mod plan_review;
mod project_settings;
mod project_setup;
mod pull_request_detail;
mod pull_requests;
mod run_again;
mod run_permissions;
mod secrets;
mod selection_focus;
mod sidebar;
mod skills;
mod tab_links;
mod tabs;
mod tabs_live;
mod tabs_window;
mod task_graph;
mod task_run;
mod tool_run;
mod tools;
mod unreadable_run;
mod window;
mod writable_folders;
