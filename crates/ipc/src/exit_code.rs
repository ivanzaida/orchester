//! Why a runner ended before the desktop could connect, told through its exit code: the runner
//! exits before its pipe exists, so the connection cannot carry the reason.

/// Another runner holds the project's runner lock (`.orchester/runner.lock`): another Orchester
/// app is running this project. Starting the runner again cannot help until that app closes.
/// `EX_TEMPFAIL` of `sysexits.h`; codes a runtime uses on its own (1, 3 for the C runtime's
/// `abort` on Windows, 101 for a Rust panic) would be mistaken for it.
pub const EXIT_PROJECT_LOCKED: u8 = 75;
