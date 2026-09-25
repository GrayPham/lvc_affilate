//! Registry node + `prepare` (validate + preview theo topo) — thay `lvc_core::graph::plan` P3.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use lvc_core::Limits;
use lvc_core::graph::{
    Catalog, Category, Graph, GraphError, Node, NodeManifest, topo_order, validate,
};

use crate::{EdgeInput, Inputs, NodeExecutor, Outputs, PreviewCtx};

/// Mọi loại node đã cài. Dựng MỘT lần ở composition root (`lvc-app`), dùng chung qua `Arc`.
#[derive(Default, Clone)]
pub struct Registry {
    nodes: BTreeMap<String, Arc<dyn NodeExecutor>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.nodes.keys()).finish()
    }
}

/// Kết quả `prepare`: thứ tự chạy + số job từng node + tổng video sẽ tạo.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Prepared {
    pub order: Vec<String>,
    pub jobs: HashMap<String, u32>,
    /// Tổng job của các node nhóm `Category::Video` (UI: "sẽ tạo N video").
    pub video_total: u32,
}

impl Registry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Đăng ký một loại node.
    ///
    /// # Panics
    /// Khi `kind` đã đăng ký — lỗi lập trình ở composition root, phải lộ ngay khi khởi động.
    pub fn register(&mut self, node: Arc<dyn NodeExecutor>) {
        let kind = node.manifest().kind.clone();
        assert!(
            !self.nodes.contains_key(&kind),
            "node kind trung lap trong registry: {kind}"
        );
        self.nodes.insert(kind, node);
    }

    #[must_use]
    pub fn with(mut self, node: Arc<dyn NodeExecutor>) -> Self {
        self.register(node);
        self
    }

    #[must_use]
    pub fn get(&self, kind: &str) -> Option<&Arc<dyn NodeExecutor>> {
        self.nodes.get(kind)
    }

    /// Manifest mọi node, sắp theo `kind` (ổn định cho fixture/UI).
    #[must_use]
    pub fn manifests(&self) -> Vec<NodeManifest> {
        self.nodes.values().map(|n| n.manifest().clone()).collect()
    }

    /// Kiểm hợp lệ cấu trúc (`lvc_core::graph::validate`) rồi `preview` từng node theo topo, gom
    /// HẾT lỗi. Node có cạnh vào từ node nguồn preview lỗi → bỏ qua (không báo lỗi kép), đầu ra
    /// của nó coi như không có.
    ///
    /// # Errors
    /// Lỗi cấu trúc (trả nguyên, không trộn lỗi preview) hoặc lỗi preview của các node.
    pub fn prepare(&self, g: &Graph, limits: &Limits) -> Result<Prepared, Vec<GraphError>> {
        validate(g, self)?;
        let order = topo_order(g).map_err(|e| vec![e])?;
        let by_id: HashMap<&str, &Node> = g.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

        let mut outputs: HashMap<String, Outputs> = HashMap::new();
        let mut errors = Vec::new();
        let mut prepared = Prepared {
            order: order.clone(),
            ..Prepared::default()
        };

        for node_id in &order {
            let Some(node) = by_id.get(node_id.as_str()) else {
                continue;
            };
            let Some(exec) = self.get(&node.kind.kind) else {
                continue; // validate() da bao Unsupported
            };
            let inputs = gather_inputs(g, node_id, &outputs);
            if inputs.any_missing() {
                continue;
            }
            let cx = PreviewCtx { node_id, limits };
            match exec.preview(&cx, &node.kind.params, &inputs) {
                Ok(p) => {
                    prepared.jobs.insert(node_id.clone(), p.jobs);
                    if exec.manifest().category == Category::Video {
                        prepared.video_total = prepared.video_total.saturating_add(p.jobs);
                    }
                    outputs.insert(node_id.clone(), p.outputs);
                }
                Err(mut e) => errors.append(&mut e),
            }
        }

        if errors.is_empty() {
            Ok(prepared)
        } else {
            Err(errors)
        }
    }
}

impl Catalog for Registry {
    fn manifest(&self, kind: &str) -> Option<&NodeManifest> {
        self.nodes.get(kind).map(|n| n.manifest())
    }
}

/// Gom đầu vào cho `node_id` từ đầu ra đã có (theo thứ tự `g.edges`). Nguồn chưa có đầu ra →
/// `EdgeInput.values = None`. Engine dùng chung hàm này khi chạy thật.
#[must_use]
pub fn gather_inputs<S: std::hash::BuildHasher>(
    g: &Graph,
    node_id: &str,
    outputs: &HashMap<String, Outputs, S>,
) -> Inputs {
    let mut inputs = Inputs::new();
    for e in g.edges.iter().filter(|e| e.to == node_id) {
        let values = outputs
            .get(&e.from)
            .map(|o| o.get(&e.from_port).cloned().unwrap_or_default());
        inputs.push(
            &e.to_port,
            EdgeInput {
                from_node: e.from.clone(),
                from_port: e.from_port.clone(),
                values,
            },
        );
    }
    inputs
}
