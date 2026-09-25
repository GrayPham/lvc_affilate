//! Mô hình đồ thị node thuần (ADR-R014): `Graph`/`Node`/`Edge`, manifest + `Catalog`, kiểm hợp lệ
//! và thứ tự topo — chỉ dữ liệu, không I/O, KHÔNG biết danh sách loại node (mỗi node là một module
//! riêng ở `crates/nodes/*`, đăng ký vào `lvc_node_api::Registry`).

mod edge;
mod node;
#[cfg(test)]
pub(crate) mod test_catalog;
mod validate;

pub use edge::Edge;
pub use node::{
    Catalog, Category, CredentialReq, Node, NodeData, NodeManifest, PortSpec, PortType,
};
pub use validate::{GraphError, topo_order, validate};

use serde::{Deserialize, Serialize};

/// Phiên bản định dạng hiện tại. v1 (P3): kind `video`; v2 (P4a): kind `dola_video`.
pub const GRAPH_VERSION: u32 = 2;

/// Đồ thị hoàn chỉnh — định dạng file workflow JSON. Thứ tự trường khớp file: `version,
/// nodes, edges`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Graph {
    pub version: u32,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Đổi tên kind từ định dạng cũ sang hiện tại (v1 → v2). Kind giờ là chuỗi mở nên đồ thị v1 vẫn
/// parse được; hàm này chỉ đổi những kind đã đổi tên. Gọi ở MỌI cửa vào (đọc DB, IPC, template).
#[must_use]
pub fn migrate(mut g: Graph) -> Graph {
    if g.version < 2 {
        for n in &mut g.nodes {
            if n.kind.kind == "video" {
                n.kind.kind = "dola_video".to_string();
            }
        }
    }
    g.version = GRAPH_VERSION;
    g
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Đồ thị mẫu: Prompt → Tạo video Dola → Lưu.
    fn fixture_graph() -> Graph {
        Graph {
            version: 2,
            nodes: vec![
                Node {
                    id: "n1".into(),
                    kind: NodeData::new("prompt", json!({"text": "…"})),
                    x: 0.0,
                    y: 0.0,
                },
                Node {
                    id: "n2".into(),
                    kind: NodeData::new(
                        "dola_video",
                        json!({"mode":"fast","model":"seedance-2.0","ratio":"9:16","duration":10,"logo":"auto","account":""}),
                    ),
                    x: 300.0,
                    y: 0.0,
                },
                Node {
                    id: "n3".into(),
                    kind: NodeData::new("save", json!({"out_dir": "downloads"})),
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

    /// File workflow P3 (v1) — phải đọc được và migrate ra đúng đồ thị v2.
    const V1_JSON: &str = "{\"version\":1,\"nodes\":[{\"id\":\"n1\",\"kind\":{\"kind\":\"prompt\",\"text\":\"…\"},\"x\":0.0,\"y\":0.0},{\"id\":\"n2\",\"kind\":{\"kind\":\"video\",\"mode\":\"fast\",\"model\":\"seedance-2.0\",\"ratio\":\"9:16\",\"duration\":10,\"logo\":\"auto\",\"account\":\"\"},\"x\":300.0,\"y\":0.0},{\"id\":\"n3\",\"kind\":{\"kind\":\"save\",\"out_dir\":\"downloads\"},\"x\":600.0,\"y\":0.0}],\"edges\":[{\"id\":\"e1\",\"from\":\"n1\",\"from_port\":\"out\",\"to\":\"n2\",\"to_port\":\"prompt\"},{\"id\":\"e2\",\"from\":\"n2\",\"from_port\":\"out\",\"to\":\"n3\",\"to_port\":\"video\"}]}";

    #[test]
    fn v1_p3_doc_duoc_va_migrate_ra_v2() {
        let v1: Graph = serde_json::from_str(V1_JSON).unwrap();
        assert_eq!(migrate(v1), fixture_graph());
    }

    #[test]
    fn v2_round_trip_giu_hinh_dang_json() {
        let g = fixture_graph();
        let s = serde_json::to_string(&g).unwrap();
        assert_eq!(
            s,
            V1_JSON
                .replace("\"version\":1", "\"version\":2")
                .replace("\"video\",\"mode\"", "\"dola_video\",\"mode\"")
        );
        let back: Graph = serde_json::from_str(&s).unwrap();
        assert_eq!(back, g);
    }

    #[test]
    fn migrate_v2_khong_doi_gi() {
        let mut g = fixture_graph();
        g.nodes[1].kind.kind = "video".into(); // kind lạ ở v2 KHÔNG bị đổi tên
        assert_eq!(migrate(g.clone()), g);
    }
}
