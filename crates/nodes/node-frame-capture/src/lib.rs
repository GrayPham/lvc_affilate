//! Node `frame_capture` — chụp MỘT khung hình từ video (cổng `in`) tại giây `time` -> ảnh PNG
//! (cổng `out`, kiểu image; nối được vào node Lưu). Dùng ffmpeg (`-ss time -frames:v 1`). ffmpeg lấy
//! qua `lvc_core::ffmpeg` (ưu tiên bản đi kèm tool; env `LVC_FFMPEG`; PATH cuối). Không tải mạng, không tài khoản.

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

pub const KIND: &str = "frame_capture";

#[derive(Debug, Clone, Deserialize)]
struct FrameParams {
    #[serde(default)]
    time: f64,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Image,
    label_vi: "Chụp khung hình".to_string(),
    inputs: vec![PortSpec::new("in", PortType::Video).required()],
    outputs: vec![PortSpec::new("out", PortType::Image)],
    require_any: vec![],
    credential: None,
    default_params: json!({ "time": 0.0 }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "time": { "type": "number", "title": "Thời điểm (giây)", "minimum": 0 }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(FrameCaptureNode)
}

struct FrameCaptureNode;

fn ffmpeg_bin() -> String {
    lvc_core::ffmpeg::ffmpeg_path()
}

/// Nơi ghi ảnh: cạnh video vào (cùng thư mục), tên `frame_<run>_<node>.png`.
fn output_path(first: &Path, run_id: &str, node_id: &str) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("frame_{run_id}_{node_id}.png"))
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

/// Chụp 1 khung tại `time` giây (seek trước `-i` cho nhanh) -> PNG.
async fn capture(input: &Path, out: &Path, time: f64) -> Result<(), String> {
    let input_s = input.to_string_lossy().to_string();
    let out_s = out.to_string_lossy().to_string();
    let time_s = format!("{time}");
    let args = [
        "-y",
        "-ss",
        &time_s,
        "-i",
        &input_s,
        "-frames:v",
        "1",
        "-update",
        "1",
        &out_s,
    ];
    run_ffmpeg(&ffmpeg_bin(), &args).await
}

#[async_trait]
impl NodeExecutor for FrameCaptureNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let _p: FrameParams = parse_params(cx.node_id, params)?;
        if inputs.assets("in").next().is_none() {
            return Err(vec![GraphError::Scene {
                node: cx.node_id.to_string(),
                line: None,
                message: "cần một video ở cổng vào".to_string(),
            }]);
        }
        let placeholder = Value::Image(
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
        let p: FrameParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let input = inputs
            .assets("in")
            .next()
            .map(|a| a.path.clone())
            .ok_or_else(|| NodeError::new(format!("node {}: khong co video vao", ctx.node_id)))?;
        let out = output_path(&input, &ctx.run_id, &ctx.node_id);
        capture(&input, &out, p.time).await.map_err(|e| {
            NodeError::new(format!("node {}: chụp khung hình lỗi: {e}", ctx.node_id))
        })?;
        let asset = Asset::new(out)
            .with(meta::NODE, &ctx.node_id)
            .with(meta::INDEX, 1);
        Ok(NodeResult::ok(Outputs::from([(
            "out".to_string(),
            vec![Value::Image(asset)],
        )])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_cung_thu_muc_input_duoi_png() {
        let o = output_path(Path::new("A:/dl/run_x_vid_1.mp4"), "run_x", "fc");
        assert_eq!(
            o.to_string_lossy().replace('\\', "/"),
            "A:/dl/frame_run_x_fc.png"
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
