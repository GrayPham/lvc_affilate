//! Node `concat` — ghép nhiều video (cổng `in`, theo thứ tự cạnh) thành MỘT file bằng ffmpeg.
//!
//! ADR-R008: ưu tiên NỐI THẲNG (concat demuxer `-c copy`, nhanh, không giảm chất lượng) khi các
//! clip cùng codec/thông số; nếu ffmpeg từ chối (khác thông số) thì lùi về RE-ENCODE (libx264/aac).
//! ffmpeg giải quyết qua `lvc_core::ffmpeg` (ưu tiên bản đi kèm tool; env `LVC_FFMPEG`; PATH cuối). Không tải mạng, không tài khoản.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};

use async_trait::async_trait;
use lvc_node_api::{
    Asset, Category, GraphError, Inputs, NodeCtx, NodeError, NodeExecutor, NodeManifest,
    NodeResult, Outputs, Params, PortSpec, PortType, Preview, PreviewCtx, Value, meta,
};
use serde_json::json;

pub const KIND: &str = "concat";

static MANIFEST: LazyLock<NodeManifest> = LazyLock::new(|| NodeManifest {
    kind: KIND.to_string(),
    version: 1,
    category: Category::Video,
    label_vi: "Nối video (ghép cảnh)".to_string(),
    inputs: vec![
        PortSpec::new("in", PortType::Video)
            .many()
            .required()
            .collect(),
    ],
    outputs: vec![PortSpec::new("out", PortType::Video)],
    require_any: vec![],
    credential: None,
    default_params: json!({}),
    params_schema: json!({ "type": "object", "properties": {} }),
});

#[must_use]
pub fn node() -> Arc<dyn NodeExecutor> {
    Arc::new(ConcatNode)
}

struct ConcatNode;

fn ffmpeg_bin() -> String {
    lvc_core::ffmpeg::ffmpeg_path()
}

/// Nơi ghi file ghép: cạnh video đầu tiên (cùng thư mục `downloads`), tên `concat_<run>_<node>.mp4`.
fn output_path(first: &Path, run_id: &str, node_id: &str) -> PathBuf {
    let dir = first
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    dir.join(format!("concat_{run_id}_{node_id}.mp4"))
}

/// Nội dung file danh sách cho concat demuxer: mỗi dòng `file '<path>'` (đường dẫn tuyệt đối, `/`,
/// escape dấu nháy đơn).
fn concat_list(paths: &[PathBuf]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    for p in paths {
        let path = p
            .to_string_lossy()
            .replace('\\', "/")
            .replace('\'', "'\\''");
        let _ = writeln!(s, "file '{path}'");
    }
    s
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

/// Ghép `paths` -> `out`. Thử `-c copy` trước; lỗi thì re-encode.
async fn concat(paths: &[PathBuf], out: &Path, list_file: &Path) -> Result<(), String> {
    tokio::fs::write(list_file, concat_list(paths))
        .await
        .map_err(|e| format!("ghi file danh sach loi: {e}"))?;
    let list = list_file.to_string_lossy().to_string();
    let out_s = out.to_string_lossy().to_string();
    let copy_args = [
        "-y", "-f", "concat", "-safe", "0", "-i", &list, "-c", "copy", &out_s,
    ];
    if run_ffmpeg(&ffmpeg_bin(), &copy_args).await.is_ok() {
        return Ok(());
    }
    // Lui ve re-encode (thong so khac nhau giua cac clip).
    let enc_args = [
        "-y",
        "-f",
        "concat",
        "-safe",
        "0",
        "-i",
        &list,
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-c:a",
        "aac",
        "-movflags",
        "+faststart",
        &out_s,
    ];
    run_ffmpeg(&ffmpeg_bin(), &enc_args).await
}

#[async_trait]
impl NodeExecutor for ConcatNode {
    fn manifest(&self) -> &NodeManifest {
        &MANIFEST
    }

    fn preview(
        &self,
        cx: &PreviewCtx<'_>,
        _params: &Params,
        inputs: &Inputs,
    ) -> Result<Preview, Vec<GraphError>> {
        let n = inputs.assets("in").count();
        if n < 2 {
            return Err(vec![GraphError::Scene {
                node: cx.node_id.to_string(),
                line: None,
                message: "cần ít nhất 2 video để ghép".to_string(),
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
        _params: &Params,
        inputs: Inputs,
    ) -> Result<NodeResult, NodeError> {
        let paths: Vec<PathBuf> = inputs.assets("in").map(|a| a.path.clone()).collect();
        if paths.len() < 2 {
            return Err(NodeError::new(format!(
                "node {}: cần ít nhất 2 video để ghép (có {})",
                ctx.node_id,
                paths.len()
            )));
        }
        let out = output_path(&paths[0], &ctx.run_id, &ctx.node_id);
        let list_file = out.with_extension("txt");
        let res = concat(&paths, &out, &list_file).await;
        let _ = tokio::fs::remove_file(&list_file).await;
        res.map_err(|e| NodeError::new(format!("node {}: ghép video lỗi: {e}", ctx.node_id)))?;
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
    fn concat_list_dung_dinh_dang() {
        let l = concat_list(&[PathBuf::from("A:/x/a.mp4"), PathBuf::from(r"B:\y\b.mp4")]);
        assert_eq!(l, "file 'A:/x/a.mp4'\nfile 'B:/y/b.mp4'\n");
    }

    #[test]
    fn output_path_cung_thu_muc_input_dau() {
        let o = output_path(Path::new("A:/dl/run_x_vid_1.mp4"), "run_x", "cc");
        assert_eq!(
            o.to_string_lossy().replace('\\', "/"),
            "A:/dl/concat_run_x_cc.mp4"
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
            "manifest.json lech — chay lai voi LVC_WRITE_FIXTURES=1"
        );
    }
}
