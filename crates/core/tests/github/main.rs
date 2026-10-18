//! GitHub through the GitHub CLI: connection checks, repository detection from git remotes and
//! linking, pull requests and their diffs, against output captured from gh 2.97.0 and git 2.46 (`../fixtures/github`).
//! Files named in the tests as synthetic follow the captured shapes for cases that cannot be produced
//! on demand (several accounts, rate limits, credentials in a remote URL).

mod connection;
mod detect;
mod link;
mod pull_request_create;
mod pull_request_detail;
mod pull_request_diff;
mod pull_requests;
mod remote;
mod replay;
mod state;
mod view;
