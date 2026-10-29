//! The Browser window (ORC-379, `docs/design/browser.md`, "The *Browser* window"; board `Browser · redesign`
//! `lcNez`): how its tabs share the title bar (`fit`), the strip's order and dots and the tab it shows (`tabs`), the
//! window itself headless on a session's browser state as a runner reports it (`window`, on `support`), and the
//! person's input on the page as it sends it to the tab (`input`). Run with
//! `cargo test -p orchester-app --test browser_window browser_`.

mod fit;
mod input;
mod support;
mod tabs;
mod window;
