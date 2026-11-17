//! spec11 core: deterministic spec-driven-development evidence. No CLI, HTTP or MCP here.
pub mod codegraph;
pub mod config;
pub mod coverage;
pub mod detect;
pub mod modules;
pub mod report;
pub mod scan;

pub use report::{Analysis, ChangeOpts, Changes, Report, Status, StatusOpts, Trace};
