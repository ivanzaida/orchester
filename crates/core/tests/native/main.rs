//! The native agent's core: tool inputs, file confinement, command classification and the loop state
//! machine (`docs/architecture/native-agent.md` §8, layer 1).

mod command;
mod known_safe;
mod known_safe_options;
mod loop_resume;
mod loop_stop;
mod loop_turns;
mod scope;
#[cfg(windows)]
mod scope_windows;
mod support;
mod tools;
