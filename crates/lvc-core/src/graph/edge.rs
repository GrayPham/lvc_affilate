//! Cạnh nối cổng ra của node này tới cổng vào của node khác — spec §4 (thuần dữ liệu,
//! không I/O; `validate`/`topo_order` ở `super::validate` diễn giải các cạnh này).

use serde::{Deserialize, Serialize};

/// Cạnh có hướng: `(from, from_port)` → `(to, to_port)`. Thứ tự trường khớp định dạng
/// file workflow JSON (Task 1 brief): `id, from, from_port, to, to_port`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub from_port: String,
    pub to: String,
    pub to_port: String,
}
