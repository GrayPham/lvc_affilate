//! Dữ liệu chạy trên dây giữa các node — kiểu CHUNG, không node nào biết kind của node nguồn.

use std::collections::BTreeMap;
use std::path::PathBuf;

use lvc_core::graph::PortType;
use serde::{Deserialize, Serialize};

/// Khoá chuẩn trong [`Asset::meta`] — dùng hằng thay chuỗi rải rác.
pub mod meta {
    /// id node đã sinh ra asset (node sinh tự ghi, engine không đóng dấu).
    pub const NODE: &str = "node";
    /// Chỉ số 1-based của asset trong đầu ra node sinh (vd `scene_index`).
    pub const INDEX: &str = "index";
    /// Nhãn hiển thị (chip tham chiếu "character", "cloths"...).
    pub const LABEL: &str = "label";
}

/// Một dòng văn bản (vd 1 dòng TXT = 1 cảnh). `attrs` là override thô theo dòng (`ratio`,
/// `duration`, `model`, `mode`, `logo`, `refs`, `account`) — node tiêu thụ tự chọn khoá cần dùng.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TextItem {
    pub text: String,
    pub source_node: String,
    /// Số dòng 1-based trong node nguồn (báo lỗi đúng chỗ).
    pub line: Option<usize>,
    pub attrs: BTreeMap<String, String>,
}

/// File media (ảnh/video/âm thanh) trên đĩa + metadata tự do.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Asset {
    pub path: PathBuf,
    pub meta: BTreeMap<String, String>,
}

impl Asset {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            meta: BTreeMap::new(),
        }
    }

    #[must_use]
    #[allow(
        clippy::needless_pass_by_value,
        reason = "builder nhan so/chuoi/bool truc tiep (`.with(INDEX, 3)`) — tham chieu lam call site xau"
    )]
    pub fn with(mut self, key: &str, value: impl ToString) -> Self {
        self.meta.insert(key.to_string(), value.to_string());
        self
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.meta.get(key).map(String::as_str)
    }
}

/// Một giá trị trên dây. Biến thể khớp 1-1 với [`PortType`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Value {
    Text(TextItem),
    Image(Asset),
    Video(Asset),
    Audio(Asset),
}

impl Value {
    #[must_use]
    pub fn port_type(&self) -> PortType {
        match self {
            Self::Text(_) => PortType::Text,
            Self::Image(_) => PortType::Image,
            Self::Video(_) => PortType::Video,
            Self::Audio(_) => PortType::Audio,
        }
    }

    #[must_use]
    pub fn as_text(&self) -> Option<&TextItem> {
        match self {
            Self::Text(t) => Some(t),
            _ => None,
        }
    }

    #[must_use]
    pub fn as_asset(&self) -> Option<&Asset> {
        match self {
            Self::Image(a) | Self::Video(a) | Self::Audio(a) => Some(a),
            Self::Text(_) => None,
        }
    }
}

/// Đầu ra của một node: tên cổng ra → danh sách giá trị (theo thứ tự).
pub type Outputs = BTreeMap<String, Vec<Value>>;

/// Dữ liệu tới qua MỘT cạnh. `values = None`: node nguồn không chạy/không có đầu ra (lỗi, bị bỏ
/// qua) — khác với `Some(vec![])` (chạy xong nhưng rỗng).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInput {
    pub from_node: String,
    pub from_port: String,
    pub values: Option<Vec<Value>>,
}

/// Đầu vào của một node: tên cổng vào → các cạnh, ĐÚNG thứ tự khai báo trong `Graph.edges`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Inputs {
    ports: BTreeMap<String, Vec<EdgeInput>>,
}

impl Inputs {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, port: &str, edge: EdgeInput) {
        self.ports.entry(port.to_string()).or_default().push(edge);
    }

    /// Các cạnh vào cổng `port` (rỗng nếu không nối).
    #[must_use]
    pub fn edges(&self, port: &str) -> &[EdgeInput] {
        self.ports.get(port).map_or(&[], Vec::as_slice)
    }

    /// Có cạnh nào (mọi cổng) mà nguồn KHÔNG có đầu ra (`values = None`)?
    #[must_use]
    pub fn any_missing(&self) -> bool {
        self.ports.values().flatten().any(|e| e.values.is_none())
    }

    /// Mọi giá trị tới cổng `port`, gộp theo thứ tự cạnh (bỏ cạnh `None`).
    pub fn values<'a>(&'a self, port: &str) -> impl Iterator<Item = &'a Value> + 'a {
        self.edges(port)
            .iter()
            .filter_map(|e| e.values.as_deref())
            .flatten()
    }

    pub fn texts<'a>(&'a self, port: &str) -> impl Iterator<Item = &'a TextItem> + 'a {
        self.values(port).filter_map(Value::as_text)
    }

    pub fn assets<'a>(&'a self, port: &str) -> impl Iterator<Item = &'a Asset> + 'a {
        self.values(port).filter_map(Value::as_asset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Value {
        Value::Text(TextItem {
            text: s.into(),
            ..TextItem::default()
        })
    }

    fn edge(from: &str, values: Option<Vec<Value>>) -> EdgeInput {
        EdgeInput {
            from_node: from.into(),
            from_port: "out".into(),
            values,
        }
    }

    #[test]
    fn values_gop_theo_thu_tu_canh_bo_canh_none() {
        let mut i = Inputs::new();
        i.push("p", edge("a", Some(vec![t("1"), t("2")])));
        i.push("p", edge("b", None));
        i.push("p", edge("c", Some(vec![t("3")])));
        let got: Vec<&str> = i.texts("p").map(|x| x.text.as_str()).collect();
        assert_eq!(got, ["1", "2", "3"]);
        assert!(i.any_missing());
        assert!(i.edges("khac").is_empty());
    }

    #[test]
    fn value_json_co_tag_type() {
        let v = Value::Video(Asset::new("a.mp4").with(meta::INDEX, 2));
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["type"], "video");
        assert_eq!(j["meta"]["index"], "2");
        assert_eq!(v.port_type(), PortType::Video);
    }
}
