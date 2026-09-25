//! Bảng manifest giả CHỈ cho test lõi — mô phỏng cổng của các node module thật
//! (`crates/nodes/*`) để test `validate`/`topo_order` không phải phụ thuộc crate node.

use std::collections::HashMap;

use serde_json::json;

use super::{Catalog, Category, Node, NodeData, NodeManifest, PortSpec, PortType};

pub(crate) struct TestCatalog(HashMap<String, NodeManifest>);

fn m(kind: &str, inputs: Vec<PortSpec>, outputs: Vec<PortSpec>) -> NodeManifest {
    NodeManifest {
        kind: kind.to_string(),
        version: 1,
        category: Category::Input,
        label_vi: kind.to_string(),
        inputs,
        outputs,
        require_any: Vec::new(),
        credential: None,
        default_params: json!({}),
        params_schema: json!({}),
    }
}

impl TestCatalog {
    pub(crate) fn new() -> Self {
        let mut save = m(
            "save",
            vec![
                PortSpec::new("video", PortType::Video).many(),
                PortSpec::new("image", PortType::Image).many(),
            ],
            vec![],
        );
        save.require_any = vec![vec!["video".into(), "image".into()]];
        let all = [
            m(
                "prompt",
                vec![],
                vec![PortSpec::new("out", PortType::Text).many()],
            ),
            m(
                "image_upload",
                vec![],
                vec![PortSpec::new("out", PortType::Image).many()],
            ),
            m(
                "dola_video",
                vec![
                    PortSpec::new("prompt", PortType::Text).many().required(),
                    PortSpec::new("refs", PortType::Image).many(),
                ],
                vec![PortSpec::new("out", PortType::Video).many()],
            ),
            save,
            // Hậu kỳ 1 vào/1 ra (many=false) — dùng test cổng đơn nhận 2 cạnh.
            m(
                "post",
                vec![PortSpec::new("in", PortType::Video).required()],
                vec![PortSpec::new("out", PortType::Video)],
            ),
        ];
        Self(all.into_iter().map(|x| (x.kind.clone(), x)).collect())
    }
}

impl Catalog for TestCatalog {
    fn manifest(&self, kind: &str) -> Option<&NodeManifest> {
        self.0.get(kind)
    }
}

pub(crate) fn node(id: &str, kind: &str) -> Node {
    Node {
        id: id.to_string(),
        kind: NodeData::new(kind, json!({})),
        x: 0.0,
        y: 0.0,
    }
}
