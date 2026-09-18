//! Mô hình đồ thị node thuần (P3, spec §4): kiểu cổng (§4.1), bảng node (§4.2), map
//! ngầm + node rào chắn (§4.3) — chỉ dữ liệu + kiểm hợp lệ, không I/O. `Graph` là định
//! dạng file workflow JSON; Task 2 (`plan`) và Tauri app/React Flow UI dựng trên kiểu này.

mod edge;
mod node;
mod validate;

pub use edge::Edge;
pub use node::{Node, NodeKind, PortSpec, PortType};
pub use validate::{GraphError, topo_order, validate};

use serde::{Deserialize, Serialize};

/// Đồ thị hoàn chỉnh — định dạng file workflow JSON. Thứ tự trường khớp file: `version,
/// nodes, edges`. `version` cố định = 1 ở P3 (dành chỗ cho migrate định dạng về sau).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub version: u32,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogoMode, Mode, Model, Ratio};

    /// Đồ thị mẫu trong brief Task 1: Prompt → Tạo video → Lưu.
    fn fixture_graph() -> Graph {
        Graph {
            version: 1,
            nodes: vec![
                Node {
                    id: "n1".into(),
                    kind: NodeKind::Prompt { text: "…".into() },
                    x: 0.0,
                    y: 0.0,
                },
                Node {
                    id: "n2".into(),
                    kind: NodeKind::Video {
                        mode: Mode::Fast,
                        model: Model::Seedance20,
                        ratio: Ratio::R9x16,
                        duration: 10,
                        logo: LogoMode::Auto,
                        account: String::new(),
                    },
                    x: 300.0,
                    y: 0.0,
                },
                Node {
                    id: "n3".into(),
                    kind: NodeKind::Save {
                        out_dir: "downloads".into(),
                    },
                    x: 600.0,
                    y: 0.0,
                },
            ],
            edges: vec![
                Edge {
                    id: "e1".into(),
                    from: "n1".into(),
                    from_port: "out".into(),
                    to: "n2".into(),
                    to_port: "prompt".into(),
                },
                Edge {
                    id: "e2".into(),
                    from: "n2".into(),
                    from_port: "out".into(),
                    to: "n3".into(),
                    to_port: "video".into(),
                },
            ],
        }
    }

    const FIXTURE_JSON: &str = "{\"version\":1,\"nodes\":[{\"id\":\"n1\",\"kind\":{\"kind\":\"prompt\",\"text\":\"…\"},\"x\":0.0,\"y\":0.0},{\"id\":\"n2\",\"kind\":{\"kind\":\"video\",\"mode\":\"fast\",\"model\":\"seedance-2.0\",\"ratio\":\"9:16\",\"duration\":10,\"logo\":\"auto\",\"account\":\"\"},\"x\":300.0,\"y\":0.0},{\"id\":\"n3\",\"kind\":{\"kind\":\"save\",\"out_dir\":\"downloads\"},\"x\":600.0,\"y\":0.0}],\"edges\":[{\"id\":\"e1\",\"from\":\"n1\",\"from_port\":\"out\",\"to\":\"n2\",\"to_port\":\"prompt\"},{\"id\":\"e2\",\"from\":\"n2\",\"from_port\":\"out\",\"to\":\"n3\",\"to_port\":\"video\"}]}";

    #[test]
    fn json_khop_dinh_dang_ghim_trong_brief() {
        let g = fixture_graph();
        assert_eq!(serde_json::to_string(&g).unwrap(), FIXTURE_JSON);
    }

    #[test]
    fn json_doc_lai_ra_dung_graph_ban_dau() {
        let back: Graph = serde_json::from_str(FIXTURE_JSON).unwrap();
        assert_eq!(back, fixture_graph());
    }
}
