//! The Browser window (ORC-379, `docs/design/browser.md`, "The *Browser* window"; board `Browser · redesign`
//! `lcNez`): how its tabs share the title bar (`fit`), the strip's order and dots and the tab it shows (`tabs`), and
//! the window itself headless on a session's browser state as a runner reports it (`window`). Run with
//! `cargo test -p orchester-app --test browser_window browser_`.

mod fit;
mod tabs;
mod window;
