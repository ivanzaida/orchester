//! The Orchestrator chat's git tools (`docs/design/orchestrator-toolset.md`, decision 9), with real git in folders of
//! the tests' own and a bare remote: every read; writes refused while the person's switch is off and, with it on,
//! commit, branch, switch, merge (clean and conflicted), pull, push, revert, tag and fetch; what can lose work only
//! proposed and applied as shown; a run's worktree never written while the run works; Orchester's claims on the
//! checkout respected; no refspec, URL or outside path the chat names reaching git; and no wait on a remote that
//! wants credentials. `cargo test -p orchester-workspace --test orchestrator_git orchestrator_git`.

#[path = "orchestrator_git/claims.rs"]
mod claims;
#[path = "orchestrator_git/injection.rs"]
mod injection;
#[path = "orchestrator_git/proposals.rs"]
mod proposals;
#[path = "orchestrator_git/reads.rs"]
mod reads;
#[path = "orchestrator_git/runs.rs"]
mod runs;
#[path = "orchestrator_git/scratch.rs"]
mod scratch;
#[path = "orchestrator_git/writes.rs"]
mod writes;
