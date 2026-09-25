//! `lvc-node-api` — hợp đồng node độc lập (ADR-R014, spec `2026-09-25-node-modules-design.md`).
//!
//! Một node module (`crates/nodes/<ten>`) chỉ cần:
//! 1. khai [`NodeManifest`] (cổng vào/ra, nhãn, tham số mặc định, loại tài khoản cần);
//! 2. impl [`NodeExecutor`]: `preview` (THUẦN — kiểm tham số, dự đoán đầu ra/số job, chạy lúc
//!    validate) và `run` (async — làm việc thật, chỉ chạm thế giới ngoài qua [`NodeCtx`]);
//! 3. được `lvc-app` đăng ký một dòng vào [`Registry`].
//!
//! Node KHÔNG biết node khác: đầu vào là [`Value`] chung theo cổng ([`Inputs`]), đầu ra là
//! [`Outputs`]. Engine chỉ gọi `registry.get(kind)`, không `match` theo kind.

mod ctx;
mod registry;
mod value;

pub use ctx::{Counts, NodeCtx, NodeError, NodeResult, Services};
pub use registry::{Prepared, Registry, gather_inputs};
pub use value::{Asset, EdgeInput, Inputs, Outputs, TextItem, Value, meta};

pub use lvc_core::graph::{
    Catalog, Category, CredentialReq, GraphError, NodeManifest, PortSpec, PortType,
};
pub use serde_json::{Map, Value as Json};

use async_trait::async_trait;
use lvc_core::Limits;

/// Tham số node đã parse sẵn từ JSON đồ thị (`NodeData::params`).
pub type Params = Map<String, Json>;

/// Ngữ cảnh THUẦN cho [`NodeExecutor::preview`].
#[derive(Debug, Clone, Copy)]
pub struct PreviewCtx<'a> {
    pub node_id: &'a str,
    pub limits: &'a Limits,
}

/// Kết quả `preview`: đầu ra dự đoán (có thể là giá trị giữ chỗ — vd video chưa tạo) và số đơn vị
/// công việc (`jobs`) để UI/engine tính tiến độ (`NodeState.total`). Node thường = 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Preview {
    pub outputs: Outputs,
    pub jobs: u32,
}

impl Preview {
    #[must_use]
    pub fn single(outputs: Outputs) -> Self {
        Self { outputs, jobs: 1 }
    }
}

/// Parse `params` JSON thành struct tham số của node; lỗi → `GraphError::Param` (field `"*"`).
///
/// # Errors
/// JSON không khớp struct `T`.
pub fn parse_params<T: serde::de::DeserializeOwned>(
    node_id: &str,
    params: &Params,
) -> Result<T, Vec<GraphError>> {
    serde_json::from_value(Json::Object(params.clone())).map_err(|e| {
        vec![GraphError::Param {
            node: node_id.to_string(),
            field: "*".to_string(),
            message: e.to_string(),
        }]
    })
}

/// Một loại node. `Send + Sync` vì registry dùng chung giữa các run.
#[async_trait]
pub trait NodeExecutor: Send + Sync {
    fn manifest(&self) -> &NodeManifest;

    /// THUẦN, không I/O: kiểm tham số + đầu vào dự đoán, trả đầu ra dự đoán. Chạy trong
    /// `Registry::prepare` theo thứ tự topo (cả khi `workflow_validate` lẫn trước khi chạy).
    ///
    /// # Errors
    /// Lỗi đồ thị/tham số của CHÍNH node này (`GraphError::Scene`, `Param`, ...).
    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>>;

    /// Làm việc thật. Không panic khi lỗi — trả [`NodeError`]; thất bại một phần (vd 1/3 cảnh
    /// hỏng) trả `Ok` với `NodeResult.failure`/`counts.failed`.
    ///
    /// # Errors
    /// [`NodeError`] — `fatal = true` dừng cả run (tương đương `hard_stop` P3).
    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError>;
}
