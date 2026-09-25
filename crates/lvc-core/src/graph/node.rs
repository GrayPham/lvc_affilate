//! Node, cổng và manifest — ADR-R014 (spec `2026-09-25-node-modules-design.md` §3).
//!
//! Lõi KHÔNG còn biết danh sách loại node (bỏ enum đóng `NodeKind` của P3). Mỗi node module
//! (`crates/nodes/*`) tự khai [`NodeManifest`] (cổng vào/ra, nhãn, loại tài khoản cần); lõi chỉ
//! tra manifest qua trait [`Catalog`] để kiểm hợp lệ đồ thị. Tham số riêng của node là JSON tự do
//! ([`NodeData::params`]) — chính node đó parse/kiểm.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Kiểu dữ liệu chạy qua một cổng. Thêm biến thể mới = thêm kiểu dây mới cho mọi node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortType {
    Text,
    Image,
    Video,
    Audio,
}

/// Mô tả một cổng (vào hoặc ra) trong manifest của một loại node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: PortType,
    /// Cổng vào nhận nhiều cạnh; `false` → cạnh thứ 2 trở đi là
    /// `GraphError::MultipleInputsOnSinglePort`.
    pub many: bool,
    /// Cổng vào phải có ít nhất 1 cạnh, thiếu → `GraphError::MissingRequiredInput`.
    pub required: bool,
    /// Cổng rào chắn (spec §4.3): chờ toàn bộ nhánh thượng nguồn. Chỉ mô tả.
    pub collect: bool,
}

impl PortSpec {
    /// Cổng mặc định: 1 cạnh, không bắt buộc, không rào chắn.
    #[must_use]
    pub fn new(name: &str, ty: PortType) -> Self {
        Self {
            name: name.to_string(),
            ty,
            many: false,
            required: false,
            collect: false,
        }
    }

    #[must_use]
    pub fn many(mut self) -> Self {
        self.many = true;
        self
    }

    #[must_use]
    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    #[must_use]
    pub fn collect(mut self) -> Self {
        self.collect = true;
        self
    }
}

/// Nhóm node trên menu "Thêm node" / palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Input,
    Text,
    Image,
    Video,
    Audio,
    Output,
}

/// Loại tài khoản node cần lấy từ kho tài khoản (spec §7). `provider` ví dụ `"dola"`,
/// `"openai"`; `kinds` ví dụ `["session"]`, `["api_key"]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialReq {
    pub provider: String,
    pub kinds: Vec<String>,
}

/// Hợp đồng tĩnh của một loại node — nguồn sự thật duy nhất cho cả engine lẫn UI
/// (`node_catalog` IPC, fixture `catalog.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeManifest {
    /// Khoá duy nhất, không bao giờ đổi (`"dola_video"`).
    pub kind: String,
    /// Tăng khi đổi hình dạng `params` (node tự migrate).
    pub version: u32,
    pub category: Category,
    pub label_vi: String,
    pub inputs: Vec<PortSpec>,
    pub outputs: Vec<PortSpec>,
    /// Mỗi nhóm: phải có cạnh vào ÍT NHẤT MỘT cổng trong nhóm (vd Save: `[["video","image"]]`).
    /// Thiếu → `MissingRequiredInput { port: "video|image" }`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub require_any: Vec<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<CredentialReq>,
    /// Tham số mặc định khi thả node mới lên canvas.
    pub default_params: Value,
    /// JSON Schema (tập con) của `params` — UI dựng form khi node không có panel riêng.
    pub params_schema: Value,
}

impl NodeManifest {
    #[must_use]
    pub fn input(&self, name: &str) -> Option<&PortSpec> {
        self.inputs.iter().find(|p| p.name == name)
    }

    #[must_use]
    pub fn output(&self, name: &str) -> Option<&PortSpec> {
        self.outputs.iter().find(|p| p.name == name)
    }
}

/// Tra manifest theo `kind` — `lvc_node_api::Registry` impl trait này; test lõi dùng bảng giả.
pub trait Catalog {
    fn manifest(&self, kind: &str) -> Option<&NodeManifest>;
}

/// Loại + tham số của một node. JSON: `{"kind":"dola_video", ...params}` — cùng hình dạng file
/// workflow P3 (`#[serde(tag = "kind")]` cũ) nên UI/DB không phải đổi cấu trúc.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeData {
    pub kind: String,
    #[serde(flatten)]
    pub params: Map<String, Value>,
}

impl NodeData {
    /// Dựng từ `kind` + object tham số (không phải object → params rỗng).
    #[must_use]
    pub fn new(kind: &str, params: Value) -> Self {
        let params = match params {
            Value::Object(m) => m,
            _ => Map::new(),
        };
        Self {
            kind: kind.to_string(),
            params,
        }
    }
}

/// Một node trong đồ thị: định danh, loại + tham số, toạ độ vẽ (React Flow).
/// Thứ tự trường khớp định dạng file workflow JSON: `id, kind, x, y`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    pub kind: NodeData,
    pub x: f32,
    pub y: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn node_data_serde_phang_nhu_dinh_dang_p3() {
        let d = NodeData::new("prompt", json!({"text": "a"}));
        assert_eq!(
            serde_json::to_string(&d).unwrap(),
            r#"{"kind":"prompt","text":"a"}"#
        );
        let back: NodeData = serde_json::from_str(r#"{"kind":"save","out_dir":"x"}"#).unwrap();
        assert_eq!(back.kind, "save");
        assert_eq!(back.params.get("out_dir"), Some(&json!("x")));
    }

    #[test]
    fn port_spec_json_dung_ten_type() {
        let p = PortSpec::new("out", PortType::Video).many();
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["type"], "video");
        assert_eq!(v["many"], true);
    }
}
