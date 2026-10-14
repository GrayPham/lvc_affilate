//! Dựng danh sách [`VideoJob`] (THUẦN, không I/O). Mỗi `TextItem` trên cổng `prompt` là MỘT job.
//! `mode="t2v"` bỏ qua cổng `ref`; `mode="i2v"` lấy ảnh ĐẦU TIÊN trên cổng `ref` làm ảnh gốc (mặc
//! định dùng chung cho mọi job, trừ dòng có override TXT `refs=`). Veo official nhận 1 ảnh gốc cho
//! image-to-video.
//!
//! Override TXT theo dòng (`attrs`, khoá trùng `node_prompt::attrs` — không import để giữ node
//! độc lập, giống `node_dola_video::jobs`): `ratio` (chuyển sang `aspect`, giá trị không nhận diện
//! được thì IM LẶNG bỏ qua), `refs` (ẢNH ĐẦU TIÊN sau khi tách `\n` thay cho ảnh gốc của cổng `ref`
//! — giữ đúng ngữ nghĩa "chỉ 1 ảnh" đã có, KHÔNG âm thầm đổi sang nhiều ảnh), `account` (ghi đè
//! `credential` khi dòng có giá trị khác rỗng). KHÔNG đọc `duration`/`model`/`mode` theo dòng.

use std::path::PathBuf;

use lvc_google::job::VideoJob;
use lvc_google::req::Aspect;
use lvc_node_api::{GraphError, Inputs, TextItem};

use crate::VideoParams;

/// Khoá `TextItem.attrs` dùng cho override theo dòng.
const RATIO: &str = "ratio";
const REFS: &str = "refs";
const ACCOUNT: &str = "account";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModeKind {
    T2v,
    I2v,
}

impl ModeKind {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "t2v" => Some(Self::T2v),
            "i2v" => Some(Self::I2v),
            _ => None,
        }
    }
}

fn param_error_single(node: &str, field: &str, message: impl Into<String>) -> GraphError {
    GraphError::Param {
        node: node.to_string(),
        field: field.to_string(),
        message: message.into(),
    }
}

fn param_error(node: &str, field: &str, message: impl Into<String>) -> Vec<GraphError> {
    vec![param_error_single(node, field, message)]
}

fn parse_aspect(node: &str, raw: &str) -> Result<Aspect, Vec<GraphError>> {
    Aspect::from_tag(raw).ok_or_else(|| {
        param_error(
            node,
            "aspect",
            format!("aspect khong hop le: '{raw}' (chi nhan portrait|landscape|square)"),
        )
    })
}

/// `"16:9"`/`"9:16"`/`"1:1"` (cú pháp `ratio=` của Dola/`node-prompt`) → [`Aspect`]. Giá trị không
/// nhận diện được → `None` (im lặng bỏ qua, giữ `aspect` tĩnh của node).
fn aspect_from_ratio_attr(raw: &str) -> Option<Aspect> {
    match raw.trim() {
        "16:9" => Aspect::from_tag("landscape"),
        "9:16" => Aspect::from_tag("portrait"),
        "1:1" => Aspect::from_tag("square"),
        _ => None,
    }
}

/// Ảnh ĐẦU TIÊN sau khi tách `refs=` TXT bằng `\n` (bỏ dòng rỗng) — giữ đúng ngữ nghĩa "chỉ 1 ảnh"
/// của node này (khác `veo3_video`/`dola_video` nhận nhiều ảnh).
fn first_txt_ref(raw: &str) -> Option<PathBuf> {
    raw.split('\n')
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// # Errors
/// `GraphError::Scene` (không có cảnh) / `GraphError::Param` (`mode`/`aspect` sai, `credential`
/// rỗng, hoặc `i2v` nhưng cổng `ref` không có ảnh).
pub fn build_jobs(
    node_id: &str,
    p: &VideoParams,
    inputs: &Inputs,
) -> Result<Vec<VideoJob>, Vec<GraphError>> {
    let items: Vec<&TextItem> = inputs.texts("prompt").collect();
    if items.is_empty() {
        return Err(vec![GraphError::Scene {
            node: node_id.to_string(),
            line: None,
            message: "khong co canh nao".to_string(),
        }]);
    }
    let mode = ModeKind::parse(&p.mode).ok_or_else(|| {
        param_error(
            node_id,
            "mode",
            format!("mode khong hop le: '{}' (chi nhan t2v|i2v)", p.mode),
        )
    })?;
    let aspect = parse_aspect(node_id, &p.aspect)?;
    if p.credential.trim().is_empty() {
        return Err(param_error(
            node_id,
            "credential",
            "credential (ten/label API key Google) khong duoc rong",
        ));
    }

    let port_ref: Option<PathBuf> = if mode == ModeKind::I2v {
        inputs.assets("ref").map(|a| a.path.clone()).next()
    } else {
        None
    };

    let mut errors = Vec::new();
    let mut jobs = Vec::with_capacity(items.len());
    for (idx, item) in items.into_iter().enumerate() {
        let scene_index = u32::try_from(idx + 1).unwrap_or(u32::MAX);

        let item_aspect = item
            .attrs
            .get(RATIO)
            .and_then(|r| aspect_from_ratio_attr(r))
            .unwrap_or(aspect);

        let item_credential = item
            .attrs
            .get(ACCOUNT)
            .map(|a| a.trim())
            .filter(|a| !a.is_empty())
            .map_or_else(|| p.credential.clone(), str::to_string);

        let ref_image_path = if mode == ModeKind::I2v {
            let txt_ref = item.attrs.get(REFS).and_then(|r| first_txt_ref(r));
            let resolved = txt_ref.or_else(|| port_ref.clone());
            if resolved.is_none() {
                errors.push(param_error_single(
                    node_id,
                    "ref",
                    format!("mode i2v nhung canh {scene_index} khong co anh goc (cong 'ref' rong, khong co refs= TXT)"),
                ));
                continue;
            }
            resolved
        } else {
            None
        };

        jobs.push(VideoJob {
            scene_index,
            credential_name: item_credential,
            prompt: item.text.clone(),
            model: p.model.clone(),
            aspect: item_aspect,
            ref_image_path,
        });
    }
    if errors.is_empty() {
        Ok(jobs)
    } else {
        Err(errors)
    }
}
