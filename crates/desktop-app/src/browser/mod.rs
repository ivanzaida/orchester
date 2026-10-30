//! The *Browser* window (`docs/design/browser.md`, "The *Browser* window"; board `Browser · redesign` `lcNez`): one
//! secondary lurq window for the whole app, named [`WINDOW_NAME`] for the app's MCP tools, that shows one project's
//! pages at a time from the screencast frames its runner sends (`orchester_workspace::browser`) and sends the person's
//! input back. Its title bar holds the mark, the project switcher (`switcher`), the tabs with their owners and *+*
//! (`strip`, `tab`, `overflow`); under it the toolbar (`toolbar`), the line that says what an agent did, and the page
//! (`page`) or what stands in its place (`notice`); its shortcuts are in `keys`. The main window opens it
//! ([`BrowserLink`]): the app menu's *Browser*, a run's *Browser* line on its Summary, the status bar's *Browser 3
//! tabs*, and by itself when an agent starts browsing ([`AutoOpen`]).

mod commands;
pub mod fit;
pub mod keys;
mod link;
pub mod model;
mod notice;
mod overflow;
mod page;
mod root;
mod state;
mod strip;
mod switcher;
mod tab;
mod title_bar;
mod toolbar;

pub use self::{
  link::{AutoOpen, BrowserLink, Show},
  root::BrowserRoot,
};

/// The window's name for the app's MCP tools (`lurq_windows`, a tool's `window`).
pub const WINDOW_NAME: &str = "browser";
/// The size it opens at (`Window/Browser` `lfxcu`).
pub const WINDOW_WIDTH: u32 = 1280;
pub const WINDOW_HEIGHT: u32 = 800;
