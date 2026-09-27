//! Node `audio_split` — tách MỘT video (cổng `in`) thành HAI nhánh song song bằng ffmpeg:
//! `video` (video không tiếng, `-an -c:v copy`, nhanh — không giải mã lại hình) và `audio`
//! (chỉ tiếng, `-vn`, giữ nguyên codec gốc bằng `-c:a copy` khi có thể). ffmpeg qua
//! `lvc_core::ffmpeg` (ưu tiên bản đi kèm tool; env `LVC_FFMPEG`; PATH cuối). Không tải mạng,
//! không tài khoản.

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

pub const KIND: &str = "audio_split";

#[derive(Debug, Deserialize)]
#[allow(
    dead_code,
    reason = "node khong co tham so - giu struct rong cho parse_params dong nhat"
)]
struct AudioSplitParams {}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Video,
    label_vi: "Tách audio↔video".to_string(),
    inputs: vec![PortSpec::new("in", PortType::Video).required()],
    outputs: vec![
        PortSpec::new("video", PortType::Video),
        PortSpec::new("audio", PortType::Audio),
    ],
    require_any: vec![],
    credential: None,
    default_params: json!({}),
    params_schema: json!({ "type": "object", "properties": {} }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(AudioSplitNode)
}

struct AudioSplitNode;

fn ffmpeg_bin() -> String {
    lvc_core::ffmpeg::ffmpeg_path()
}

/// Nơi ghi video câm: cạnh video vào (cùng thư mục), tên `split_video_<run>_<node>.mp4`.
fn video_output_path(first: &Path, run_id: &str, node_id: &str) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("split_video_{run_id}_{node_id}.mp4"))
}

/// Nơi ghi audio: cạnh video vào (cùng thư mục), tên `split_audio_<run>_<node>.m4a`.
fn audio_output_path(first: &Path, run_id: &str, node_id: &str) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("split_audio_{run_id}_{node_id}.m4a"))
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

/// `input` -> video không tiếng: `-an -c:v copy` (không giải mã lại hình).
fn video_args<'a>(input: &'a str, out: &'a str) -> Vec<&'a str> {
    vec!["-y", "-i", input, "-an", "-c:v", "copy", out]
}

/// `input` -> chỉ audio: `-vn`. Ưu tiên `-c:a copy` (nhanh); node gọi lùi re-encode aac nếu lỗi.
fn audio_copy_args<'a>(input: &'a str, out: &'a str) -> Vec<&'a str> {
    vec!["-y", "-i", input, "-vn", "-c:a", "copy", out]
}

fn audio_encode_args<'a>(input: &'a str, out: &'a str) -> Vec<&'a str> {
    vec!["-y", "-i", input, "-vn", "-acodec", "aac", out]
}

async fn split_video(input: &Path, out: &Path) -> Result<(), String> {
    let input_s = input.to_string_lossy().to_string();
    let out_s = out.to_string_lossy().to_string();
    run_ffmpeg(&ffmpeg_bin(), &video_args(&input_s, &out_s)).await
}

/// Tách audio: thử `-c:a copy` trước (giữ nguyên codec, nhanh); lỗi (vd container m4a không nhận
/// codec gốc) thì lùi về re-encode AAC.
async fn split_audio(input: &Path, out: &Path) -> Result<(), String> {
    let input_s = input.to_string_lossy().to_string();
    let out_s = out.to_string_lossy().to_string();
    if run_ffmpeg(&ffmpeg_bin(), &audio_copy_args(&input_s, &out_s))
        .await
        .is_ok()
    {
        return Ok(());
    }
    run_ffmpeg(&ffmpeg_bin(), &audio_encode_args(&input_s, &out_s)).await
}

#[async_trait]
impl NodeExecutor for AudioSplitNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let _p: AudioSplitParams = parse_params(cx.node_id, params)?;
        if inputs.assets("in").next().is_none() {
            return Err(vec![GraphError::Scene {
                node: cx.node_id.to_string(),
                line: None,
                message: "cần một video ở cổng vào".to_string(),
            }]);
        }
        let video_placeholder = Value::Video(
            Asset::default()
                .with(meta::NODE, cx.node_id)
                .with(meta::INDEX, 1),
        );
        let audio_placeholder = Value::Audio(
            Asset::default()
                .with(meta::NODE, cx.node_id)
                .with(meta::INDEX, 1),
        );
        Ok(Preview::single(Outputs::from([
            ("video".to_string(), vec![video_placeholder]),
            ("audio".to_string(), vec![audio_placeholder]),
        ])))
    }

    async fn run(
        &self,
        ctx: &NodeCtx,
        params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let _p: AudioSplitParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let input = inputs
            .assets("in")
            .next()
            .map(|a| a.path.clone())
            .ok_or_else(|| NodeError::new(format!("node {}: khong co video vao", ctx.node_id)))?;

        let video_out = video_output_path(&input, &ctx.run_id, &ctx.node_id);
        split_video(&input, &video_out)
            .await
            .map_err(|e| NodeError::new(format!("node {}: tách video lỗi: {e}", ctx.node_id)))?;

        let audio_out = audio_output_path(&input, &ctx.run_id, &ctx.node_id);
        split_audio(&input, &audio_out)
            .await
            .map_err(|e| NodeError::new(format!("node {}: tách audio lỗi: {e}", ctx.node_id)))?;

        let video_asset = Asset::new(video_out)
            .with(meta::NODE, &ctx.node_id)
            .with(meta::INDEX, 1);
        let audio_asset = Asset::new(audio_out)
            .with(meta::NODE, &ctx.node_id)
            .with(meta::INDEX, 1);
        Ok(NodeResult::ok(Outputs::from([
            ("video".to_string(), vec![Value::Video(video_asset)]),
            ("audio".to_string(), vec![Value::Audio(audio_asset)]),
        ])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_cung_thu_muc_input() {
        let v = video_output_path(Path::new("A:/dl/run_x_vid_1.mp4"), "run_x", "sp");
        assert_eq!(
            v.to_string_lossy().replace('\\', "/"),
            "A:/dl/split_video_run_x_sp.mp4"
        );
        let a = audio_output_path(Path::new("A:/dl/run_x_vid_1.mp4"), "run_x", "sp");
        assert_eq!(
            a.to_string_lossy().replace('\\', "/"),
            "A:/dl/split_audio_run_x_sp.m4a"
        );
    }

    #[test]
    fn video_args_bo_tieng_giu_nguyen_hinh() {
        assert_eq!(
            video_args("in.mp4", "out.mp4"),
            vec!["-y", "-i", "in.mp4", "-an", "-c:v", "copy", "out.mp4"]
        );
    }

    #[test]
    fn audio_args_bo_hinh_uu_tien_copy_roi_encode() {
        assert_eq!(
            audio_copy_args("in.mp4", "out.m4a"),
            vec!["-y", "-i", "in.mp4", "-vn", "-c:a", "copy", "out.m4a"]
        );
        assert_eq!(
            audio_encode_args("in.mp4", "out.m4a"),
            vec!["-y", "-i", "in.mp4", "-vn", "-acodec", "aac", "out.m4a"]
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
