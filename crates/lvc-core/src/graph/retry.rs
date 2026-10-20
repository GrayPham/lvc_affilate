//! Phần C (run-robustness) — chọn tập node CHẠY LẠI khi "chạy lại node lỗi". THUẦN (không I/O):
//! cho đồ thị + tập node lỗi (`seeds`), trả tập node phải chạy lại = `seeds` ∪ toàn bộ HẬU DUỆ
//! (node phụ thuộc, trực/gián tiếp) — vì output của chúng đổi khi node lỗi chạy lại. Node đã xong
//! NGOÀI tập này được seed output (engine/lvc-app lo), không chạy lại (không tốn credit).

use std::collections::{BTreeSet, VecDeque};

use super::Graph;

/// Tập node phải chạy lại: `seeds` cùng mọi node với tới được từ chúng theo cạnh `from -> to`.
/// Trả GỒM cả `seeds` để caller dùng thẳng làm "rerun set". Đồ thị đã validate không chu trình nên
/// không lặp vô hạn, nhưng vẫn phòng chu trình bằng tập đã thăm.
#[must_use]
pub fn descendants(graph: &Graph, seeds: &BTreeSet<String>) -> BTreeSet<String> {
    let mut seen: BTreeSet<String> = seeds.clone();
    let mut queue: VecDeque<String> = seeds.iter().cloned().collect();
    while let Some(cur) = queue.pop_front() {
        for e in &graph.edges {
            if e.from == cur && !seen.contains(&e.to) {
                seen.insert(e.to.clone());
                queue.push_back(e.to.clone());
            }
        }
    }
    seen
}

#[cfg(test)]
mod tests {
    use super::descendants;
    use crate::graph::{Edge, Graph};
    use std::collections::BTreeSet;

    fn edge(from: &str, to: &str) -> Edge {
        Edge {
            id: format!("{from}-{to}"),
            from: from.to_string(),
            from_port: "out".to_string(),
            to: to.to_string(),
            to_port: "in".to_string(),
        }
    }

    fn graph(edges: Vec<Edge>) -> Graph {
        Graph {
            version: crate::graph::GRAPH_VERSION,
            nodes: Vec::new(),
            edges,
        }
    }

    fn set(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn descendants_gom_hau_due_va_seed() {
        // A->B->C, D rời.
        let g = graph(vec![edge("A", "B"), edge("B", "C")]);
        assert_eq!(descendants(&g, &set(&["A"])), set(&["A", "B", "C"]));
    }

    #[test]
    fn descendants_nhieu_nhanh_vao_mot_node() {
        // A->C, B->C.
        let g = graph(vec![edge("A", "C"), edge("B", "C")]);
        assert_eq!(descendants(&g, &set(&["B"])), set(&["B", "C"]));
    }

    #[test]
    fn descendants_seed_la_la_chi_tra_chinh_no() {
        let g = graph(vec![edge("A", "B")]);
        assert_eq!(descendants(&g, &set(&["B"])), set(&["B"]));
    }
}
