//! The browser for agents' core rules (`docs/design/browser.md`, P1): the tool catalog and its inputs, the profile's
//! *Browser* permission with its allowed sites and their matching, the policy rows, and widening it; and how a step is
//! offered the tools and a call that asked is approved (P2b).
//! `cargo test -p orchester-core --test browser browser_`.

mod browser {
  mod calls;
  mod catalog;
  mod offer;
  mod policy;
  mod sites;
}
