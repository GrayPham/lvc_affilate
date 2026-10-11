//! Kiểu "công việc" cho host trait [`crate::GrokImageHost`] — `local_path` do phía impl
//! (`lvc-engine`) quyết, không phải provider.

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ImageJob {
    pub scene_index: u32,
    /// TÊN (label) API key trong kho — engine resolve.
    pub credential_name: String,
    pub prompt: String,
    pub model: String,
}

#[derive(Debug, Clone)]
pub struct ImageJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct ImageJobsReport {
    pub done: Vec<ImageJobDone>,
    pub failed: Vec<String>,
}
