//! Node `trim` — cắt MỘT video (cổng `in`) theo khoảng thời gian `[start, end]` giây bằng ffmpeg.
//!
//! Ưu tiên CẮT NHANH (`-c copy`, không giải mã lại); nếu ffmpeg từ chối (khác keyframe/thông số) thì
//! lùi về RE-ENCODE (libx264/aac). ffmpeg lấy từ env `LVC_FFMPEG` hoặc `ffmpeg` trong PATH. Không tải
//! mạng, không tài khoản. `end <= start` (hoặc `end = 0`) = cắt tới hết video.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest,
    NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value, meta,
    parse_params,
};
use serde::Deserialize;
use serde_json::json;

pub const KIND: &str = "trim";

#[derive(Debug, Clone, Deserialize)]
struct TrimParams {
    #[serde(default)]
    start: f64,
    #[serde(default)]
    end: f64,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Video,
    label_vi: "Cắt video".to_string(),
    inputs: vec![PortSpec::new("in", PortType::Video).required()],
    outputs: vec![PortSpec::new("out", PortType::Video)],
    require_any: vec![],
    credential: None,
    default_params: json!({ "start": 0.0, "end": 0.0 }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "start": { "type": "number", "title": "Bắt đầu (giây)", "minimum": 0 },
            "end": { "type": "number", "title": "Kết thúc (giây, 0 = hết)", "minimum": 0 }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(TrimNode)
}

struct TrimNode;

fn ffmpeg_bin() -> String {
    std::env::var("LVC_FFMPEG").unwrap_or_else(|_| "ffmpeg".to_string())
}

/// Nơi ghi file cắt: cạnh video vào (cùng thư mục), tên `trim_<run>_<node>.mp4`.
fn output_path(first: &Path, run_id: &str, node_id: &str) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("trim_{run_id}_{node_id}.mp4"))
}

/// `[start, end]` -> tham số thời gian ffmpeg (đặt SAU `-i`: seek chính xác theo khung hình).
/// `end <= start` -> chỉ `-ss` (cắt tới hết).
fn time_args(start: f64, end: f64) -> Vec<String> {
    let mut v = Vec::new();
    if start > 0.0 {
        v.push("-ss".to_string());
        v.push(format!("{start}"));
    }
    if end > start {
        v.push("-to".to_string());
        v.push(format!("{end}"));
    }
    v
}

async fn run_ffmpeg(bin: &str, args: &[&str]) -> Result<(), String> {
    let out = tokio::process::Command::new(bin)
        .args(args)
        .output()
        .await
        .map_err(|e| format!("khong chay duoc ffmpeg ({bin}): {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let tail: String = err
        .lines()
        .rev()
        .take(4)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    Err(tail)
}

/// Cắt `input` -> `out` theo `[start, end]`. Thử `-c copy` trước; lỗi thì re-encode.
async fn trim(input: &Path, out: &Path, start: f64, end: f64) -> Result<(), String> {
    let input_s = input.to_string_lossy().to_string();
    let out_s = out.to_string_lossy().to_string();
    let t = time_args(start, end);
    let t_ref: Vec<&str> = t.iter().map(String::as_str).collect();

    let mut copy_args = vec!["-y", "-i", &input_s];
    copy_args.extend_from_slice(&t_ref);
    copy_args.extend_from_slice(&["-c", "copy", &out_s]);
    if run_ffmpeg(&ffmpeg_bin(), &copy_args).await.is_ok() {
        return Ok(());
    }
    let mut enc_args = vec!["-y", "-i", &input_s];
    enc_args.extend_from_slice(&t_ref);
    enc_args.extend_from_slice(&[
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        &out_s,
    ]);
    run_ffmpeg(&ffmpeg_bin(), &enc_args).await
}

#[async_trait]
impl NodeExecutor for TrimNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let p: TrimParams = parse_params(cx.node_id, params)?;
        if p.end > 0.0 && p.end <= p.start {
            return Err(vec![GraphError::Scene {
                node: cx.node_id.to_string(),
                line: None,
                message: "kết thúc phải lớn hơn bắt đầu (hoặc để 0 = hết video)".to_string(),
            }]);
        }
        if inputs.assets("in").next().is_none() {
            return Err(vec![GraphError::Scene {
                node: cx.node_id.to_string(),
                line: None,
                message: "cần một video ở cổng vào".to_string(),
            }]);
        }
        let placeholder = Value::Video(
            Asset::default()
                .with(meta::NODE, cx.node_id)
                .with(meta::INDEX, 1),
        );
        Ok(Preview::single(Outputs::from([(
            "out".to_string(),
            vec![placeholder],
        )])))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let p: TrimParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let input = inputs
            .assets("in")
            .next()
            .map(|a| a.path.clone())
            .ok_or_else(|| NodeError::new(format!("node {}: khong co video vao", ctx.node_id)))?;
        let out = output_path(&input, &ctx.run_id, &ctx.node_id);
        trim(&input, &out, p.start, p.end)
            .await
            .map_err(|e| NodeError::new(format!("node {}: cắt video lỗi: {e}", ctx.node_id)))?;
        let asset = Asset::new(out)
            .with(meta::NODE, &ctx.node_id)
            .with(meta::INDEX, 1);
        Ok(NodeResult::ok(Outputs::from([(
            "out".to_string(),
            vec![Value::Video(asset)],
        )])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_args_bo_ss_khi_start_0_va_bo_to_khi_end_le_start() {
        assert_eq!(time_args(0.0, 0.0), Vec::<String>::new());
        assert_eq!(time_args(0.0, 5.0), vec!["-to", "5"]);
        assert_eq!(time_args(2.0, 0.0), vec!["-ss", "2"]);
        assert_eq!(time_args(2.0, 7.0), vec!["-ss", "2", "-to", "7"]);
        assert_eq!(time_args(5.0, 3.0), vec!["-ss", "5"]);
    }

    #[test]
    fn output_path_cung_thu_muc_input() {
        let o = output_path(Path::new("A:/dl/run_x_vid_1.mp4"), "run_x", "tr");
        assert_eq!(
            o.to_string_lossy().replace('\\', "/"),
            "A:/dl/trim_run_x_tr.mp4"
        );
    }

    #[test]
    fn manifest_khop_file_manifest_json() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/manifest.json");
        let got = serde_json::to_string_pretty(node().manifest()).unwrap() + "\n";
        if std::env::var_os("LVC_WRITE_FIXTURES").is_some() {
            std::fs::write(path, &got).unwrap();
        }
        let want = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        assert_eq!(
            got, want,
            "manifest.json lech - chay lai voi LVC_WRITE_FIXTURES=1"
        );
    }
}
