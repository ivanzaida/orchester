//! The colors of the *Needs you* count Windows shows over the app's taskbar button (ORC-56). No design frame draws it:
//! it takes the dark theme's `danger` (#F87171) as its disc and `canvas` (#101010) as its figures, the colors the
//! design gives what needs the person's attention. The system draws it, so these are raw 0xRRGGBB values rather than
//! lurq roles; macOS draws the Dock's badge in its own colors.

/// The disc, `danger` #F87171.
pub const DISC: u32 = 0x00F8_7171;
/// The figures, `canvas` #101010.
pub const FIGURES: u32 = 0x0010_1010;
