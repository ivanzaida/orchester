//! The Browser window (ORC-379, `docs/design/browser.md`, "The *Browser* window"; board `Browser · redesign`
//! `lcNez`): how its tabs share the title bar (`fit`), the strip's order and dots and the tab it shows (`tabs`), the
//! window itself headless on a session's browser state as a runner reports it (`window`, on `support`), the person's
//! input on the page as it sends it to the tab (`input`), what the address line shows and whether it holds the
//! focus (`address`), and what it does while a restart waits for its answer (`restart`). Run with
//! `cargo test -p orchester-app --test browser_window browser_`.

mod address;
mod fit;
mod input;
mod restart;
mod support;
mod tabs;
mod window;
