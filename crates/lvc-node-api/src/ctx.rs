//! Ngữ cảnh chạy của node + kết quả/lỗi.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use lvc_core::Limits;
use tokio_util::sync::CancellationToken;

use crate::Outputs;

/// Kho dịch vụ do host (engine/app) cấp cho node, tra theo KIỂU. Node khai kiểu dịch vụ nó cần
/// (vd `Arc<dyn lvc_dola::DolaVideoHost>`) trong crate provider, KHÔNG phụ thuộc engine.
#[derive(Clone, Default)]
pub struct Services {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl std::fmt::Debug for Services {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Services")
            .field("len", &self.map.len())
            .finish()
    }
}

impl Services {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Chèn (ghi đè) dịch vụ kiểu `T`. `T` thường là `Arc<dyn Trait>`.
    pub fn insert<T: Any + Send + Sync>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Arc::new(value));
    }

    /// Bản sao dịch vụ kiểu `T` (rẻ khi `T = Arc<..>`).
    #[must_use]
    pub fn get<T: Any + Send + Sync + Clone>(&self) -> Option<T> {
        self.map
            .get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
            .cloned()
    }
}

/// Mọi thứ node được chạm vào khi chạy.
#[derive(Debug, Clone)]
pub struct NodeCtx {
    pub run_id: String,
    pub workflow_id: String,
    pub node_id: String,
    /// Token của RUN — node dùng làm cha cho token con của từng job.
    pub cancel: CancellationToken,
    pub limits: Arc<Limits>,
    pub services: Services,
}

/// Bộ đếm tiến độ cuối của node (`EngineEvent::NodeState`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub done: u32,
    pub total: u32,
    pub ok: u32,
    pub failed: u32,
}

impl Counts {
    /// Một đơn vị công việc thành công.
    pub const ONE_OK: Self = Self {
        done: 1,
        total: 1,
        ok: 1,
        failed: 0,
    };
    /// Một đơn vị công việc thất bại.
    pub const ONE_FAILED: Self = Self {
        done: 1,
        total: 1,
        ok: 0,
        failed: 1,
    };
}

/// Kết quả node chạy xong (có thể thất bại MỘT PHẦN).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NodeResult {
    pub outputs: Outputs,
    pub counts: Counts,
    /// Thông điệp khi thất bại một phần (vào `runs.error`), vd `"node vid: canh 1,3 that bai"`.
    pub failure: Option<String>,
}

impl NodeResult {
    #[must_use]
    pub fn ok(outputs: Outputs) -> Self {
        Self {
            outputs,
            counts: Counts::ONE_OK,
            failure: None,
        }
    }

    #[must_use]
    pub fn is_failed(&self) -> bool {
        self.failure.is_some() || self.counts.failed > 0
    }
}

/// Node thất bại hoàn toàn (không có đầu ra).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct NodeError {
    pub message: String,
    /// `true` = dừng CẢ run (node sau không chạy) — vd ảnh đầu vào thiếu file.
    pub fatal: bool,
}

impl NodeError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fatal: false,
        }
    }

    #[must_use]
    pub fn fatal(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            fatal: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    trait Hello: Send + Sync {
        fn hi(&self) -> &'static str;
    }
    struct H;
    impl Hello for H {
        fn hi(&self) -> &'static str {
            "xin chao"
        }
    }

    #[test]
    fn services_tra_theo_kieu_trait_object() {
        let mut s = Services::new();
        assert!(s.get::<Arc<dyn Hello>>().is_none());
        s.insert::<Arc<dyn Hello>>(Arc::new(H));
        assert_eq!(s.get::<Arc<dyn Hello>>().unwrap().hi(), "xin chao");
    }
}
