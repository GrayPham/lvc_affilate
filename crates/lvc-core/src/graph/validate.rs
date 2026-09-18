//! Kiểm hợp lệ đồ thị và thứ tự chạy — spec §4. `validate` gom HẾT lỗi tìm được (không
//! dừng ở lỗi đầu tiên) theo thứ tự tất định: theo thứ tự node rồi theo thứ tự cạnh.
//! `topo_order` dùng Kahn, ổn định theo chỉ số node; gặp chu trình trả về node id trên
//! chu trình đó (không nhất thiết là chu trình ngắn nhất).

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::{Graph, Node, NodeKind, PortType};

/// (id node, tên cổng vào) → số cạnh trỏ tới cổng đó (bước 3: bắt `many=false` bị nối 2 lần).
type InCount<'g> = HashMap<(&'g str, &'g str), usize>;
/// Tập (id node, tên cổng vào) đã có ít nhất 1 cạnh nối tới (bước 4: cổng bắt buộc).
type ConnectedInputs<'g> = HashSet<(&'g str, &'g str)>;

/// Lỗi đồ thị. `Serialize` để lộ ra UI (Tauri command trả `Vec<GraphError>` cho React Flow).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GraphError {
    #[error("trung id node: {0}")]
    DuplicateNodeId(String),
    #[error("canh tham chieu node khong ton tai: {0}")]
    UnknownNode(String),
    #[error("node {node} khong co cong {port}")]
    UnknownPort { node: String, port: String },
    #[error("canh {edge} sai kieu: {from:?} -> {to:?}")]
    TypeMismatch {
        edge: String,
        from: PortType,
        to: PortType,
    },
    #[error("node {node} thieu dau vao bat buoc o cong {port}")]
    MissingRequiredInput { node: String, port: String },
    #[error("chu trinh: {0:?}")]
    Cycle(Vec<String>),
    #[error("node {node} (kind={kind}) chua ho tro o P3")]
    Unsupported { node: String, kind: String },
    #[error("do thi rong")]
    Empty,
    #[error("cong don {port} tren node {node} nhan qua 1 canh")]
    MultipleInputsOnSinglePort { node: String, port: String },
    // Hai bien duoi day KHONG do validate() sinh ra — plan() (Task 2, spec §4.3) dung
    // chung enum GraphError de UI xu ly mot kieu loi duy nhat cho ca kiem hop le lan lap
    // ke hoach chay.
    /// `SceneError` tu `bulk_scenes`/dòng TXT khi map Prompt → cảnh (message = `SceneError` Display).
    #[error("node {node}: {message}")]
    Scene {
        node: String,
        line: Option<usize>,
        message: String,
    },
    /// Tổng ảnh tham chiếu (cổng `refs` ∪ TXT `refs=`, đã khử trùng) của một cảnh vượt
    /// `Limits.reference_image_max_count`.
    #[error("node {node} canh {scene_index}: qua nhieu anh tham chieu ({count} > {max})")]
    TooManyRefs {
        node: String,
        scene_index: u32,
        count: usize,
        max: usize,
    },
}

/// Kiểm hợp lệ toàn bộ đồ thị: id node không trùng, cạnh trỏ tới node/cổng tồn tại,
/// kiểu cổng khớp, cổng vào bắt buộc phải có cạnh, cổng `many=false` chỉ nhận 1 cạnh,
/// loại node được hỗ trợ ở P3, và không có chu trình. Gom hết lỗi vào một `Vec` thay vì
/// dừng ở lỗi đầu, theo thứ tự tất định: node rồi cạnh (chia nhỏ thành các bước
/// `check_*` bên dưới để giữ mỗi hàm ngắn, dễ đọc).
///
/// # Errors
/// `Vec<GraphError>` không rỗng khi đồ thị vi phạm ít nhất một ràng buộc trên.
pub fn validate(g: &Graph) -> Result<(), Vec<GraphError>> {
    if g.nodes.is_empty() {
        // Đồ thị rỗng không còn gì khác để kiểm — trả đúng 1 lỗi, không lẫn lỗi khác.
        return Err(vec![GraphError::Empty]);
    }

    let mut errors = Vec::new();

    check_nodes(g, &mut errors);

    // Tra node theo id (id trùng đã báo lỗi riêng ở trên; bản ghi đầu tiên đủ để tra cứu).
    let mut by_id: HashMap<&str, &Node> = HashMap::new();
    for node in &g.nodes {
        by_id.entry(node.id.as_str()).or_insert(node);
    }

    let (in_count, connected_inputs) = check_edges(g, &by_id, &mut errors);
    check_multiple_inputs(g, &in_count, &mut errors);
    check_missing_required(g, &connected_inputs, &mut errors);

    if let Err(GraphError::Cycle(ids)) = topo_order(g) {
        errors.push(GraphError::Cycle(ids));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Bước 1 — id trùng + loại chưa hỗ trợ ở P3 (theo thứ tự node).
fn check_nodes(g: &Graph, errors: &mut Vec<GraphError>) {
    let mut seen_ids: HashSet<&str> = HashSet::new();
    for node in &g.nodes {
        if !seen_ids.insert(node.id.as_str()) {
            errors.push(GraphError::DuplicateNodeId(node.id.clone()));
        }
        if !node.kind.supported_in_p3() {
            errors.push(GraphError::Unsupported {
                node: node.id.clone(),
                kind: node.kind.tag().to_string(),
            });
        }
    }
}

/// Bước 2 — cạnh: node/cổng tồn tại, kiểu khớp (theo thứ tự cạnh). Trả về bảng đếm số
/// cạnh vào mỗi cổng vào (dùng ở bước 3) và tập cổng vào "đã nối" (dùng ở bước 4) — một
/// cổng được coi là "đã nối" khi tồn tại (bất kể phía nguồn có khớp kiểu hay không).
fn check_edges<'g>(
    g: &'g Graph,
    by_id: &HashMap<&'g str, &'g Node>,
    errors: &mut Vec<GraphError>,
) -> (InCount<'g>, ConnectedInputs<'g>) {
    let mut in_count: InCount<'g> = HashMap::new();
    let mut connected_inputs: ConnectedInputs<'g> = HashSet::new();

    for edge in &g.edges {
        let from_node = by_id.get(edge.from.as_str()).copied();
        let to_node = by_id.get(edge.to.as_str()).copied();

        if from_node.is_none() {
            errors.push(GraphError::UnknownNode(edge.from.clone()));
        }
        if to_node.is_none() {
            errors.push(GraphError::UnknownNode(edge.to.clone()));
        }

        let from_port =
            from_node.and_then(|n| n.kind.outputs().iter().find(|p| p.name == edge.from_port));
        if from_node.is_some() && from_port.is_none() {
            errors.push(GraphError::UnknownPort {
                node: edge.from.clone(),
                port: edge.from_port.clone(),
            });
        }

        let to_port = to_node.and_then(|n| n.kind.inputs().iter().find(|p| p.name == edge.to_port));
        if to_node.is_some() && to_port.is_none() {
            errors.push(GraphError::UnknownPort {
                node: edge.to.clone(),
                port: edge.to_port.clone(),
            });
        }

        if to_port.is_some() {
            let key = (edge.to.as_str(), edge.to_port.as_str());
            *in_count.entry(key).or_insert(0) += 1;
            connected_inputs.insert(key);
        }

        if let (Some(fp), Some(tp)) = (from_port, to_port) {
            if fp.ty != tp.ty {
                errors.push(GraphError::TypeMismatch {
                    edge: edge.id.clone(),
                    from: fp.ty,
                    to: tp.ty,
                });
            }
        }
    }

    (in_count, connected_inputs)
}

/// Bước 3 — cổng `many=false` nhận quá 1 cạnh (theo thứ tự node rồi thứ tự cổng khai báo).
fn check_multiple_inputs(g: &Graph, in_count: &InCount<'_>, errors: &mut Vec<GraphError>) {
    for node in &g.nodes {
        for spec in node.kind.inputs() {
            if !spec.many {
                let count = in_count
                    .get(&(node.id.as_str(), spec.name))
                    .copied()
                    .unwrap_or(0);
                if count > 1 {
                    errors.push(GraphError::MultipleInputsOnSinglePort {
                        node: node.id.clone(),
                        port: spec.name.to_string(),
                    });
                }
            }
        }
    }
}

/// Bước 4 — thiếu đầu vào bắt buộc (theo thứ tự node rồi thứ tự cổng khai báo). `Save`
/// nhận `Video[]` hoặc `Image[]` qua 2 cổng riêng — chỉ cần ≥1 trong 2 (không dùng cờ
/// `required` của từng `PortSpec` vì đây là ràng buộc "một trong hai", không phải "cả hai").
fn check_missing_required(
    g: &Graph,
    connected_inputs: &ConnectedInputs<'_>,
    errors: &mut Vec<GraphError>,
) {
    for node in &g.nodes {
        if let NodeKind::Save { .. } = node.kind {
            let has_video = connected_inputs.contains(&(node.id.as_str(), "video"));
            let has_image = connected_inputs.contains(&(node.id.as_str(), "image"));
            if !has_video && !has_image {
                errors.push(GraphError::MissingRequiredInput {
                    node: node.id.clone(),
                    port: "video|image".to_string(),
                });
            }
        } else {
            for spec in node.kind.inputs() {
                if spec.required && !connected_inputs.contains(&(node.id.as_str(), spec.name)) {
                    errors.push(GraphError::MissingRequiredInput {
                        node: node.id.clone(),
                        port: spec.name.to_string(),
                    });
                }
            }
        }
    }
}

/// Thứ tự chạy topo (Kahn), ổn định theo chỉ số node gốc: ở mỗi bước, chọn node đầu
/// tiên (theo thứ tự khai báo) còn lại có bậc vào 0. Cạnh trỏ tới node không tồn tại bị
/// bỏ qua (đã báo lỗi riêng ở `validate`) để hàm này dùng được kể cả trên đồ thị chưa
/// hợp lệ hoàn toàn.
///
/// # Errors
/// `GraphError::Cycle` kèm danh sách id node trên một chu trình tìm được (không nhất
/// thiết là chu trình ngắn nhất) khi đồ thị không phải DAG.
pub fn topo_order(g: &Graph) -> Result<Vec<String>, GraphError> {
    let n = g.nodes.len();
    let index_of: HashMap<&str, usize> = g
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indegree: Vec<usize> = vec![0; n];
    for edge in &g.edges {
        if let (Some(&fi), Some(&ti)) = (
            index_of.get(edge.from.as_str()),
            index_of.get(edge.to.as_str()),
        ) {
            adj[fi].push(ti);
            indegree[ti] += 1;
        }
    }

    let mut removed = vec![false; n];
    let mut order = Vec::with_capacity(n);
    while let Some(i) = (0..n).find(|&i| !removed[i] && indegree[i] == 0) {
        removed[i] = true;
        order.push(g.nodes[i].id.clone());
        for &j in &adj[i] {
            if !removed[j] {
                indegree[j] -= 1;
            }
        }
    }

    if order.len() == n {
        return Ok(order);
    }

    let remaining: Vec<usize> = (0..n).filter(|&i| !removed[i]).collect();
    let cycle_indices = find_one_cycle(&remaining, &adj, &removed);
    Err(GraphError::Cycle(
        cycle_indices
            .into_iter()
            .map(|i| g.nodes[i].id.clone())
            .collect(),
    ))
}

/// Tìm một chu trình trong đồ thị con còn lại sau Kahn (`remaining` chắc chắn khác rỗng
/// và mỗi node trong đó có bậc vào > 0 trong đồ thị con này, nên chu trình luôn tồn tại).
fn find_one_cycle(remaining: &[usize], adj: &[Vec<usize>], removed: &[bool]) -> Vec<usize> {
    let n = adj.len();
    let mut visited = vec![false; n];
    let mut on_stack = vec![false; n];
    let mut path: Vec<usize> = Vec::new();

    for &start in remaining {
        if !visited[start] {
            if let Some(cycle) =
                dfs_find_cycle(start, adj, removed, &mut visited, &mut on_stack, &mut path)
            {
                return cycle;
            }
        }
    }
    // Ly thuyet khong toi day (Kahn con sot node => luon co chu trinh); tra ve nguyen
    // phan con lai thay vi panic.
    remaining.to_vec()
}

fn dfs_find_cycle(
    u: usize,
    adj: &[Vec<usize>],
    removed: &[bool],
    visited: &mut [bool],
    on_stack: &mut [bool],
    path: &mut Vec<usize>,
) -> Option<Vec<usize>> {
    visited[u] = true;
    on_stack[u] = true;
    path.push(u);

    for &v in &adj[u] {
        if removed[v] {
            continue;
        }
        if on_stack[v] {
            if let Some(start) = path.iter().position(|&x| x == v) {
                return Some(path[start..].to_vec());
            }
        } else if !visited[v] {
            if let Some(cycle) = dfs_find_cycle(v, adj, removed, visited, on_stack, path) {
                return Some(cycle);
            }
        }
    }

    path.pop();
    on_stack[u] = false;
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Node};
    use crate::{LogoMode, Mode, Model, Ratio};

    fn prompt(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Prompt {
                text: "canh 1".to_string(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn video(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Video {
                mode: Mode::Fast,
                model: Model::Seedance20,
                ratio: Ratio::R9x16,
                duration: 10,
                logo: LogoMode::Off,
                account: String::new(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn save(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Save {
                out_dir: "downloads".to_string(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn image_upload(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::ImageUpload { paths: vec![] },
            x: 0.0,
            y: 0.0,
        }
    }

    fn edge(id: &str, from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
        Edge {
            id: id.to_string(),
            from: from.to_string(),
            from_port: from_port.to_string(),
            to: to.to_string(),
            to_port: to_port.to_string(),
        }
    }

    #[test]
    fn do_thi_hop_le_3_node_ok() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1"), video("n2"), save("n3")],
            edges: vec![
                edge("e1", "n1", "out", "n2", "prompt"),
                edge("e2", "n2", "out", "n3", "video"),
            ],
        };
        assert_eq!(validate(&g), Ok(()));
    }

    #[test]
    fn do_thi_rong_chi_tra_1_loi_empty() {
        let g = Graph {
            version: 1,
            nodes: vec![],
            edges: vec![],
        };
        assert_eq!(validate(&g), Err(vec![GraphError::Empty]));
    }

    #[test]
    fn id_trung_bao_loi() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1"), prompt("n1")],
            edges: vec![],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::DuplicateNodeId("n1".to_string())));
    }

    #[test]
    fn canh_toi_node_la_bao_loi() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1")],
            edges: vec![edge("e1", "n1", "out", "n2", "prompt")],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::UnknownNode("n2".to_string())));
    }

    #[test]
    fn cong_la_bao_loi() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1"), video("n2")],
            edges: vec![edge("e1", "n1", "out", "n2", "bogus")],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::UnknownPort {
            node: "n2".to_string(),
            port: "bogus".to_string(),
        }));
    }

    #[test]
    fn text_sang_image_sai_kieu_bao_loi() {
        let g = Graph {
            version: 1,
            nodes: vec![image_upload("n1"), video("n2")],
            edges: vec![edge("e1", "n1", "out", "n2", "prompt")],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::TypeMismatch {
            edge: "e1".to_string(),
            from: PortType::Image,
            to: PortType::Text,
        }));
    }

    #[test]
    fn video_thieu_prompt_bao_loi() {
        let g = Graph {
            version: 1,
            nodes: vec![video("n1")],
            edges: vec![],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::MissingRequiredInput {
            node: "n1".to_string(),
            port: "prompt".to_string(),
        }));
    }

    #[test]
    fn save_khong_noi_video_lan_image_bao_loi_gop() {
        let g = Graph {
            version: 1,
            nodes: vec![save("n1")],
            edges: vec![],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::MissingRequiredInput {
            node: "n1".to_string(),
            port: "video|image".to_string(),
        }));
    }

    #[test]
    fn save_chi_noi_image_thi_du_khong_bao_loi_thieu_dau_vao() {
        let g = Graph {
            version: 1,
            nodes: vec![image_upload("n1"), save("n2")],
            edges: vec![edge("e1", "n1", "out", "n2", "image")],
        };
        // Chi noi "image" (khong noi "video") van du vi Save chi can >=1 trong 2 cong.
        assert_eq!(validate(&g), Ok(()));
    }

    #[test]
    fn chu_trinh_a_b_a_bao_loi() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1"), prompt("n2")],
            edges: vec![
                edge("e1", "n1", "out", "n2", "in"),
                edge("e2", "n2", "out", "n1", "in"),
            ],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::Cycle(vec!["n1".to_string(), "n2".to_string()])));
    }

    #[test]
    fn concat_khong_ho_tro_o_p3_bao_loi_unsupported() {
        let g = Graph {
            version: 1,
            nodes: vec![
                video("n1"),
                video("n2"),
                Node {
                    id: "n3".to_string(),
                    kind: NodeKind::Concat,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            edges: vec![
                edge("e1", "n1", "out", "n3", "in"),
                edge("e2", "n2", "out", "n3", "in"),
            ],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::Unsupported {
            node: "n3".to_string(),
            kind: "concat".to_string(),
        }));
    }

    #[test]
    fn hai_canh_vao_cong_many_false_bao_loi() {
        // EnsureH264."in" la many=false — 2 canh Video vao cung cong phai bi bat.
        let g = Graph {
            version: 1,
            nodes: vec![
                video("n1"),
                video("n2"),
                Node {
                    id: "n3".to_string(),
                    kind: NodeKind::EnsureH264,
                    x: 0.0,
                    y: 0.0,
                },
            ],
            edges: vec![
                edge("e1", "n1", "out", "n3", "in"),
                edge("e2", "n2", "out", "n3", "in"),
            ],
        };
        let errs = validate(&g).unwrap_err();
        assert!(errs.contains(&GraphError::MultipleInputsOnSinglePort {
            node: "n3".to_string(),
            port: "in".to_string(),
        }));
    }

    #[test]
    fn topo_order_hinh_kim_cuong_on_dinh_theo_chi_so_node() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1"), prompt("n2"), prompt("n3"), prompt("n4")],
            edges: vec![
                edge("e1", "n1", "out", "n2", "in"),
                edge("e2", "n1", "out", "n3", "in"),
                edge("e3", "n2", "out", "n4", "in"),
                edge("e4", "n3", "out", "n4", "in"),
            ],
        };
        assert_eq!(
            topo_order(&g).unwrap(),
            vec![
                "n1".to_string(),
                "n2".to_string(),
                "n3".to_string(),
                "n4".to_string()
            ]
        );
    }

    #[test]
    fn topo_order_chu_trinh_tra_ve_cycle() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt("n1"), prompt("n2")],
            edges: vec![
                edge("e1", "n1", "out", "n2", "in"),
                edge("e2", "n2", "out", "n1", "in"),
            ],
        };
        assert_eq!(
            topo_order(&g),
            Err(GraphError::Cycle(vec!["n1".to_string(), "n2".to_string()]))
        );
    }
}
