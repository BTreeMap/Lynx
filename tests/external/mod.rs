// Each integration-test binary links only the scenarios it executes. The
// shared module tree deliberately remains available to both the functional and
// benchmark binaries, so per-binary dead-code diagnostics are not actionable.
#![allow(dead_code)]

pub(crate) mod benchmark;
pub(crate) mod client;
pub(crate) mod config;
pub(crate) mod lifecycle;
pub(crate) mod report;
pub(crate) mod scenarios;
pub(crate) mod traffic;
