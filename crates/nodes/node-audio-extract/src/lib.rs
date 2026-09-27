//! Node `audio_extract` — tách audio từ MỘT video (cổng `in`) bằng ffmpeg (`-vn`) -> file audio
//! (cổng `out`, kiểu audio). Định dạng ra chọn qua tham số `format` (`mp3`|`aac`|`wav`, mặc định
//! `mp3`). ffmpeg qua `lvc_core::ffmpeg` (ưu tiên bản đi kèm tool; env `LVC_FFMPEG`; PATH cuối).
//! Không tải mạng, không tài khoản.

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

pub const KIND: &str = "audio_extract";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AudioFormat {
    #[default]
    Mp3,
    Aac,
    Wav,
}

impl AudioFormat {
    /// Đuôi file ra (khớp tên biến thể).
    const fn ext(self) -> &'static str {
        match self {
            Self::Mp3 => "mp3",
            Self::Aac => "aac",
            Self::Wav => "wav",
        }
    }

    /// Codec ffmpeg (`-acodec`) tương ứng.
    const fn codec(self) -> &'static str {
        match self {
            Self::Mp3 => "libmp3lame",
            Self::Aac => "aac",
            Self::Wav => "pcm_s16le",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AudioExtractParams {
    #[serde(default)]
    format: AudioFormat,
}

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Audio,
    label_vi: "Tách audio".to_string(),
    inputs: vec![PortSpec::new("in", PortType::Video).required()],
    outputs: vec![PortSpec::new("out", PortType::Audio)],
    require_any: vec![],
    credential: None,
    default_params: json!({ "format": "mp3" }),
    params_schema: json!({
        "type": "object",
        "properties": {
            "format": {
                "type": "string",
                "title": "Định dạng",
                "enum": ["mp3", "aac", "wav"]
            }
        }
    }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(AudioExtractNode)
}

struct AudioExtractNode;

fn ffmpeg_bin() -> String {
    lvc_core::ffmpeg::ffmpeg_path()
}

/// Nơi ghi audio: cạnh video vào (cùng thư mục), tên `audio_<run>_<node>.<ext>`.
fn output_path(first: &Path, run_id: &str, node_id: &str, format: AudioFormat) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("audio_{run_id}_{node_id}.{}", format.ext()))
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

/// `input` (video) -> `out` (audio) — bỏ hình (`-vn`), encode audio theo `format`.
fn extract_args<'a>(input: &'a str, out: &'a str, format: AudioFormat) -> Vec<&'a str> {
    vec!["-y", "-i", input, "-vn", "-acodec", format.codec(), out]
}

async fn extract(input: &Path, out: &Path, format: AudioFormat) -> Result<(), String> {
    let input_s = input.to_string_lossy().to_string();
    let out_s = out.to_string_lossy().to_string();
    let args = extract_args(&input_s, &out_s, format);
    run_ffmpeg(&ffmpeg_bin(), &args).await
}

#[async_trait]
impl NodeExecutor for AudioExtractNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let _p: AudioExtractParams = parse_params(cx.node_id, params)?;
        if inputs.assets("in").next().is_none() {
            return Err(vec![GraphError::Scene {
                node: cx.node_id.to_string(),
                line: None,
                message: "cần một video ở cổng vào".to_string(),
            }]);
        }
        let placeholder = Value::Audio(
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
        let p: AudioExtractParams =
            parse_params(&ctx.node_id, params).map_err(|e| NodeError::new(e[0].to_string()))?;
        let input = inputs
            .assets("in")
            .next()
            .map(|a| a.path.clone())
            .ok_or_else(|| NodeError::new(format!("node {}: khong co video vao", ctx.node_id)))?;
        let out = output_path(&input, &ctx.run_id, &ctx.node_id, p.format);
        extract(&input, &out, p.format)
            .await
            .map_err(|e| NodeError::new(format!("node {}: tách audio lỗi: {e}", ctx.node_id)))?;
        let asset = Asset::new(out)
            .with(meta::NODE, &ctx.node_id)
            .with(meta::INDEX, 1);
        Ok(NodeResult::ok(Outputs::from([(
            "out".to_string(),
            vec![Value::Audio(asset)],
        )])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_path_cung_thu_muc_input_theo_dinh_dang() {
        let o = output_path(
            Path::new("A:/dl/run_x_vid_1.mp4"),
            "run_x",
            "ax",
            AudioFormat::Mp3,
        );
        assert_eq!(
            o.to_string_lossy().replace('\\', "/"),
            "A:/dl/audio_run_x_ax.mp3"
        );
        let o = output_path(
            Path::new("A:/dl/run_x_vid_1.mp4"),
            "run_x",
            "ax",
            AudioFormat::Wav,
        );
        assert_eq!(
            o.to_string_lossy().replace('\\', "/"),
            "A:/dl/audio_run_x_ax.wav"
        );
    }

    #[test]
    fn extract_args_bo_hinh_dung_codec_theo_dinh_dang() {
        assert_eq!(
            extract_args("in.mp4", "out.mp3", AudioFormat::Mp3),
            vec![
                "-y",
                "-i",
                "in.mp4",
                "-vn",
                "-acodec",
                "libmp3lame",
                "out.mp3"
            ]
        );
        assert_eq!(
            extract_args("in.mp4", "out.aac", AudioFormat::Aac),
            vec!["-y", "-i", "in.mp4", "-vn", "-acodec", "aac", "out.aac"]
        );
        assert_eq!(
            extract_args("in.mp4", "out.wav", AudioFormat::Wav),
            vec![
                "-y",
                "-i",
                "in.mp4",
                "-vn",
                "-acodec",
                "pcm_s16le",
                "out.wav"
            ]
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
