use serde::{Deserialize, Serialize};

/// Đúng 4 trạng thái. Huỷ = `Failed` + `failure_code = "cancelled"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Processing,
    Completed,
    Failed,
}

impl TaskStatus {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "queued" => Some(Self::Queued),
            "processing" => Some(Self::Processing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BatchProgress {
    pub total: usize,
    pub queued: usize,
    pub processing: usize,
    pub completed: usize,
    pub failed: usize,
    pub percent: u8,
}

/// `percent = round(100 * (completed + failed) / total)`.
#[must_use]
pub fn batch_progress(statuses: &[TaskStatus]) -> BatchProgress {
    let mut p = BatchProgress { total: statuses.len(), ..BatchProgress::default() };
    for s in statuses {
        match s {
            TaskStatus::Queued => p.queued += 1,
            TaskStatus::Processing => p.processing += 1,
            TaskStatus::Completed => p.completed += 1,
            TaskStatus::Failed => p.failed += 1,
        }
    }
    if p.total > 0 {
        let done = p.completed + p.failed;
        // dung chuyen doi chinh xac theo cau hoi
        #[allow(clippy::cast_precision_loss)]
        let pct = (100.0 * done as f64 / p.total as f64).round();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let percent = u8::try_from(pct as u64).unwrap_or(100);
        p.percent = percent;
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_la_hoan_tat_cong_that_bai() {
        let tasks = [
            TaskStatus::Completed,
            TaskStatus::Failed,
            TaskStatus::Queued,
            TaskStatus::Processing,
        ];
        let p = batch_progress(&tasks);
        assert_eq!(p.total, 4);
        assert_eq!(p.completed, 1);
        assert_eq!(p.failed, 1);
        assert_eq!(p.queued, 1);
        assert_eq!(p.processing, 1);
        assert_eq!(p.percent, 50);
    }

    #[test]
    fn rong_thi_percent_0() {
        let p = batch_progress(&[]);
        assert_eq!(p.total, 0);
        assert_eq!(p.percent, 0);
    }

    #[test]
    fn trang_thai_parse_dung_4_gia_tri() {
        assert_eq!(TaskStatus::parse("queued"), Some(TaskStatus::Queued));
        assert_eq!(TaskStatus::parse("processing"), Some(TaskStatus::Processing));
        assert_eq!(TaskStatus::parse("completed"), Some(TaskStatus::Completed));
        assert_eq!(TaskStatus::parse("failed"), Some(TaskStatus::Failed));
        assert_eq!(TaskStatus::parse("cancelled"), None, "huy = failed + failure_code");
    }
}
