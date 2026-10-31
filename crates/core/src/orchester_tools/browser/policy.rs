//! Which steps get the browser tools and whether a call asks (`docs/design/browser.md`, "Permissions and policy";
//! the rows join `docs/interactive-tool-approvals.md`'s table):
//!
//! | Browser tools | Step that may write the worktree | Read-only step |
//! | --- | --- | --- |
//! | Browser off, or network off | not offered | not offered |
//! | Reading tools (`browser_tabs`, `browser_read_page`, `browser_page_text`, `browser_screenshot`, `browser_console`, `browser_network`, `browser_wait`) and `browser_close` | runs | runs |
//! | Opening a URL or acting on a page of an allowed site or a local development host | runs | runs |
//! | Opening a URL or acting on a page of any other site | asks | asks |
//! | `browser_eval` | asks, at every call | asks, at every call |
//!
//! A read-only step gets the browser too, as the person decided (2026-10-29): browsing does not write the worktree,
//! and acting on a site the profile does not allow asks. Auto-approval never answers a browser call that asks
//! ([`crate::orchester_tools::auto_approvable`]): a click on another site can send anything anywhere.

use super::{BrowserReach, BrowserTool};
use crate::{
  orchester_tools::ToolPolicy,
  permission::{
    AgentPermissions, NetworkAccess,
    browser::{BrowserAccess, SiteHost},
  },
};

/// Whether a step with `permissions` is offered the browser tools: its profile's *Browser* is on and its network is
/// on, whatever its repository access. Network to listed hosts only gives no browser: the browser's own allowed
/// sites are a different list, and a page loads from hosts neither names.
pub fn offered(permissions: &AgentPermissions) -> bool {
  permissions.browser().access() == BrowserAccess::On && matches!(permissions.network(), NetworkAccess::On)
}

/// The policy of a call of `tool` in a step with `permissions`; `None` when the step is not offered the browser.
/// `site` is the host the call goes to or acts on: the URL `browser_open` opens, or the page of the tab any other
/// call acts on, `None` for a page without a web host, which asks.
pub fn decide(tool: BrowserTool, site: Option<&SiteHost>, permissions: &AgentPermissions) -> Option<ToolPolicy> {
  if !offered(permissions) {
    return None;
  }
  Some(match tool.reach() {
    BrowserReach::Reads => ToolPolicy::Runs,
    BrowserReach::Site if site.is_some_and(|host| permissions.browser().allows_site(host)) => ToolPolicy::Runs,
    BrowserReach::Site | BrowserReach::Script => ToolPolicy::Asks,
  })
}
