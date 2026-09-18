//! Kế hoạch chạy — biến `Graph` đã kiểm hợp lệ thành dãy job cụ thể: map NGẦM (spec §4.3)
//! "mỗi dòng Prompt = 1 cảnh, mỗi cảnh = 1 video job". Thuần dữ liệu, không I/O — Task 7
//! (`lvc-engine`) đọc `RunPlan` này để chạy DAG thật.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::{Edge, Graph, GraphError, Node, NodeKind, topo_order, validate};
use crate::{
    CoreError, Limits, LogoMode, Mode, Model, Ratio, RawScene, Scene, SceneError, parse_txt,
    scene_txt,
};

/// (id node Prompt nguồn, số dòng 1-based trong node đó, `RawScene` CHƯA chuẩn hoá) — một
/// "dòng thô" trong danh sách đã gộp từ nhiều node Prompt, TRƯỚC khi đánh lại `scene_index`.
type RawPiece = (String, Option<usize>, RawScene);

/// Một job tạo video: đúng 1 cảnh của node Tạo video `node_id` — đã gộp mặc định node +
/// override TXT theo dòng + ảnh tham chiếu (cổng `refs` ∪ TXT `refs=`, spec §4.3).
/// `rename_all = "camelCase"` — cùng quy ước với `GraphError` (fix round 1 Task 1): camelCase
/// ngay tại nguồn để Task 9 (Tauri IPC) không cần thêm lớp DTO chỉ để đổi cách viết hoa.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoJob {
    pub node_id: String,
    /// 1-based; đánh lại LIÊN TỤC theo thứ tự gộp khi 1 node Tạo video nhận nhiều node
    /// Prompt thượng nguồn (many=true trên cổng `prompt`).
    pub scene_index: u32,
    pub scene: Scene,
}

/// Một job lưu — khớp đúng 1 cạnh nối vào cổng `video`/`image` của node Lưu.
/// `rename_all = "camelCase"` — xem lý do ở `VideoJob`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveJob {
    pub node_id: String,
    pub out_dir: String,
    pub from_node: String,
}

/// Một bước của kế hoạch chạy, theo đúng thứ tự topo của node sinh ra nó (spec §4.3).
/// `rename_all_fields = "camelCase"` áp cho field bên trong CÁC STRUCT-VARIANT (`Prompt`,
/// `ImageUpload`, `Video`) — cùng quy ước `GraphError` (fix round 1 Task 1). Biến thể
/// newtype `Save(SaveJob)` không có field riêng để đổi ở đây (không phải struct-variant),
/// nhưng field bên trong `SaveJob` vẫn ra camelCase nhờ `#[serde(rename_all = "camelCase")]`
/// khai báo NGAY TRÊN `SaveJob` — không phụ thuộc cách nó được bọc ở đây.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Step {
    Prompt {
        node_id: String,
        scenes: Vec<Scene>,
    },
    ImageUpload {
        node_id: String,
        paths: Vec<String>,
    },
    Video {
        node_id: String,
        jobs: Vec<VideoJob>,
    },
    Save(SaveJob),
}

/// Kế hoạch chạy đầy đủ: dãy bước theo topo + tổng số video job (Task 7 dùng tính %).
/// `rename_all = "camelCase"` — xem lý do ở `VideoJob`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunPlan {
    pub steps: Vec<Step>,
    pub video_total: u32,
}

/// Biến `Graph` đã kiểm hợp lệ thành `RunPlan` — spec §4.3 "map ngầm": mỗi dòng văn bản
/// của node Prompt là 1 cảnh; mỗi cảnh thuộc node Tạo video là 1 `VideoJob` (`scene_index`
/// 1-based); ảnh từ node Tải ảnh lên thượng nguồn (cổng `refs`) gắn vào MỌI cảnh của node
/// Tạo video đó; node Lưu sinh 1 `SaveJob` cho mỗi cạnh nối vào.
///
/// Bất biến (ghim ở test `so_canh_prompt_bang_so_job_video_khi_co_dong_comment_va_dong_trong`,
/// review Task 2 vòng 1, Important #2): với một node Prompt nối THẲNG vào một node Tạo
/// video (không lỗi), `Step::Prompt.scenes.len()` của node Prompt đó LUÔN bằng số
/// `VideoJob` mà nó đóng góp cho `Step::Video`. Đúng vì `step_prompt` (dùng `parse_txt`)
/// và `collect_prompt_pieces` (dùng `parse_lines_raw`) cùng gọi chung `scene_txt::parse_line`
/// cho từng dòng và bỏ dòng trống/`#` theo ĐÚNG MỘT quy tắc — hai hàm chỉ khác nhau ở việc
/// `parse_txt` chuẩn hoá luôn bằng mặc định chung, còn `parse_lines_raw` hoãn chuẩn hoá tới
/// khi biết mặc định của node Tạo video. Bất biến này SẼ VỠ nếu quy tắc bỏ dòng của một
/// trong hai hàm đổi mà hàm kia không đổi theo — không có gì trong kiểu dữ liệu ép hai bên
/// phải khớp, chỉ có test này canh.
///
/// # Errors
/// `validate(g)` không đạt → trả nguyên vẹn `Vec<GraphError>` của nó (không trộn lỗi lập kế
/// hoạch vào). Sau khi đồ thị hợp lệ, còn gom thêm lỗi cảnh (`GraphError::Scene`, từ cú pháp
/// TXT hay chuẩn hoá `Scene` sai) và vượt trần ảnh tham chiếu (`GraphError::TooManyRefs`).
pub fn plan(g: &Graph, limits: &Limits) -> Result<RunPlan, Vec<GraphError>> {
    validate(g)?;
    // validate() da soat chu trinh o tren nen topo_order khong the Err o day; van xu ly
    // Err thay vi unwrap/expect de khong panic neu validate() noi long rang buoc sau nay.
    let order = topo_order(g).map_err(|e| vec![e])?;

    let by_id: HashMap<&str, &Node> = g.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let mut errors: Vec<GraphError> = Vec::new();
    let mut steps: Vec<Step> = Vec::new();
    let mut video_total: u32 = 0;

    for node_id in &order {
        // topo_order() tra id tu chinh g.nodes da dung de dung by_id nen luon tra Some —
        // van .get() + bao loi thay vi index [] panic, de khong sup do neu bat bien nay
        // lech trong tuong lai (vi du validate()/topo_order() sua sai).
        let Some(node) = by_id.get(node_id.as_str()).copied() else {
            errors.push(GraphError::UnknownNode {
                id: node_id.clone(),
            });
            continue;
        };
        match &node.kind {
            NodeKind::Prompt { text } => step_prompt(node, text, limits, &mut steps, &mut errors),
            NodeKind::ImageUpload { paths } => steps.push(Step::ImageUpload {
                node_id: node.id.clone(),
                paths: paths.clone(),
            }),
            NodeKind::Video { .. } => step_video(
                node,
                g,
                &by_id,
                limits,
                &mut steps,
                &mut errors,
                &mut video_total,
            ),
            NodeKind::Save { .. } => step_save(node, g, &mut steps),
            // validate() da bao loi Unsupported va chan cac kind P4 nay truoc khi toi day.
            NodeKind::ImageFromUrl { .. }
            | NodeKind::ImageGen { .. }
            | NodeKind::RemoveLogo
            | NodeKind::EnsureH264
            | NodeKind::Concat => {}
        }
    }

    if errors.is_empty() {
        Ok(RunPlan { steps, video_total })
    } else {
        Err(errors)
    }
}

/// Gói 1 `SceneError` thành `GraphError::Scene` — `message` = `SceneError` Display
/// (đã có sẵn tiền tố "line N: " khi `line` là `Some`).
fn scene_error(node: &str, e: &SceneError) -> GraphError {
    GraphError::Scene {
        node: node.to_string(),
        line: e.line,
        message: e.to_string(),
    }
}

/// Bước `Prompt`: mỗi dòng TXT (`parse_txt`, mặc định CHUNG của `Scene::normalize` — bước
/// này CHƯA biết node Tạo video nào sẽ tiêu thụ) là 1 cảnh.
fn step_prompt(
    node: &Node,
    text: &str,
    limits: &Limits,
    steps: &mut Vec<Step>,
    errors: &mut Vec<GraphError>,
) {
    match parse_txt(text, limits) {
        Ok(scenes) => steps.push(Step::Prompt {
            node_id: node.id.clone(),
            scenes,
        }),
        Err(e) => errors.push(scene_error(&node.id, &e)),
    }
}

/// Bước `Save`: 1 `SaveJob` cho MỖI cạnh nối vào cổng `video` hoặc `image` (spec §4.3).
fn step_save(node: &Node, g: &Graph, steps: &mut Vec<Step>) {
    let NodeKind::Save { out_dir } = &node.kind else {
        return;
    };
    for edge in incoming(g, &node.id, &["video", "image"]) {
        steps.push(Step::Save(SaveJob {
            node_id: node.id.clone(),
            out_dir: out_dir.clone(),
            from_node: edge.from.clone(),
        }));
    }
}

/// Bước `Video`: gộp cảnh từ mọi node Prompt thượng nguồn (cổng `prompt`, many=true) theo
/// đúng thứ tự cạnh, đánh lại `scene_index` 1..N liên tục; gộp ảnh từ mọi node Tải ảnh lên
/// thượng nguồn (cổng `refs`) vào MỌI cảnh; mặc định lấy từ cấu hình node, TXT trên dòng
/// thắng mặc định (riêng `account`: node thắng nếu khác "", còn không thì lấy TXT `acc`).
fn step_video(
    node: &Node,
    g: &Graph,
    by_id: &HashMap<&str, &Node>,
    limits: &Limits,
    steps: &mut Vec<Step>,
    errors: &mut Vec<GraphError>,
    video_total: &mut u32,
) {
    let NodeKind::Video {
        mode,
        model,
        ratio,
        duration,
        logo,
        account,
    } = &node.kind
    else {
        return;
    };

    let port_refs = collect_port_refs(g, by_id, &node.id);
    let defaults = video_defaults(*mode, *model, *ratio, *duration, *logo, account);

    // Gop danh sach dong THO (chua chuan hoa) tu MOI node Prompt thuong nguon, dung thu
    // tu cạnh — day chinh la "map ngam": N dong -> N canh, roi danh lai chi so lien tuc.
    let Some(pieces) = collect_prompt_pieces(g, by_id, &node.id, errors) else {
        return; // loi cu phap TXT o mot node Prompt thuong nguon da duoc gom vao errors.
    };
    if pieces.is_empty() {
        errors.push(GraphError::Scene {
            node: node.id.clone(),
            line: None,
            message: "khong co canh nao".to_string(),
        });
        return;
    }
    if pieces.len() > limits.batch_max_scenes {
        errors.push(GraphError::Scene {
            node: node.id.clone(),
            line: None,
            message: CoreError::TooManyScenes(limits.batch_max_scenes).to_string(),
        });
        return;
    }

    let jobs = build_jobs(&node.id, pieces, &defaults, &port_refs, limits, errors);
    *video_total = video_total.saturating_add(u32::try_from(jobs.len()).unwrap_or(u32::MAX));
    steps.push(Step::Video {
        node_id: node.id.clone(),
        jobs,
    });
}

/// Ảnh tham chiếu từ cổng `refs` (mọi node Tải ảnh lên thượng nguồn), giữ thứ tự cạnh rồi
/// thứ tự nội bộ của từng node (ruling task-2: cổng TRƯỚC, TXT SAU khi gộp với `refs=`).
fn collect_port_refs(g: &Graph, by_id: &HashMap<&str, &Node>, video_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    for edge in incoming(g, video_id, &["refs"]) {
        if let Some(src) = by_id.get(edge.from.as_str()) {
            if let NodeKind::ImageUpload { paths } = &src.kind {
                out.extend(paths.iter().cloned());
            }
        }
    }
    out
}

/// Mặc định của node Tạo video, dạng `RawScene` — chỉ dùng cho trường nào dòng TXT KHÔNG
/// tự ghi đè (spec §4.3).
fn video_defaults(
    mode: Mode,
    model: Model,
    ratio: Ratio,
    duration: u16,
    logo: LogoMode,
    account: &str,
) -> RawScene {
    RawScene {
        prompt: None,
        ratio: Some(ratio.as_str().to_string()),
        duration: Some(duration.to_string()),
        model: Some(model.as_str().to_string()),
        mode: Some(mode.as_str().to_string()),
        logo: Some(logo.as_str().to_string()),
        reference_images: None,
        account: if account.is_empty() {
            None
        } else {
            Some(account.to_string())
        },
    }
}

/// Gộp các dòng thô từ mọi node Prompt nối vào cổng `prompt` của node Tạo video, đúng thứ
/// tự cạnh. Trả `None` khi có lỗi cú pháp TXT ở BẤT KỲ node nguồn nào (đã đẩy vào `errors`
/// — không lập kế hoạch tiếp cho node Tạo video này nữa vì thứ tự gộp đã hỏng).
fn collect_prompt_pieces(
    g: &Graph,
    by_id: &HashMap<&str, &Node>,
    video_id: &str,
    errors: &mut Vec<GraphError>,
) -> Option<Vec<RawPiece>> {
    let mut pieces = Vec::new();
    let mut ok = true;
    for edge in incoming(g, video_id, &["prompt"]) {
        let Some(src) = by_id.get(edge.from.as_str()) else {
            continue;
        };
        let NodeKind::Prompt { text } = &src.kind else {
            continue;
        };
        match scene_txt::parse_lines_raw(text) {
            Ok(lines) => pieces.extend(
                lines
                    .into_iter()
                    .map(|(line, raw)| (src.id.clone(), Some(line), raw)),
            ),
            Err(e) => {
                errors.push(scene_error(&src.id, &e));
                ok = false;
            }
        }
    }
    ok.then_some(pieces)
}

/// Với mỗi dòng thô đã gộp: đánh `scene_index` 1..N liên tục, gộp ảnh tham chiếu (cổng ∪
/// TXT, kiểm trần), gộp mặc định node + override TXT rồi `Scene::normalize`. Lỗi gom vào
/// `errors`, KHÔNG dừng giữa chừng — mỗi dòng độc lập với các dòng còn lại.
fn build_jobs(
    video_id: &str,
    pieces: Vec<RawPiece>,
    defaults: &RawScene,
    port_refs: &[String],
    limits: &Limits,
    errors: &mut Vec<GraphError>,
) -> Vec<VideoJob> {
    let mut jobs = Vec::with_capacity(pieces.len());
    for (idx, (src_id, line, raw)) in pieces.into_iter().enumerate() {
        // pieces.len() da kiem <= limits.batch_max_scenes (usize nho) o tren nen khong tran.
        let scene_index = u32::try_from(idx + 1).unwrap_or(u32::MAX);

        let txt_refs = raw.reference_images.clone().unwrap_or_default();
        let merged_refs = merge_refs(port_refs, &txt_refs);
        if merged_refs.len() > limits.reference_image_max_count {
            errors.push(GraphError::TooManyRefs {
                node: video_id.to_string(),
                scene_index,
                count: merged_refs.len(),
                max: limits.reference_image_max_count,
            });
            continue;
        }

        // TXT tren dong thang mac dinh node — TRU account: node thang neu khac "", con
        // khong thi lay TXT `acc` (nguoc chieu voi cac truong con lai, dung ruling task-2).
        let merged = RawScene {
            prompt: raw.prompt,
            ratio: raw.ratio.or_else(|| defaults.ratio.clone()),
            duration: raw.duration.or_else(|| defaults.duration.clone()),
            model: raw.model.or_else(|| defaults.model.clone()),
            mode: raw.mode.or_else(|| defaults.mode.clone()),
            logo: raw.logo.or_else(|| defaults.logo.clone()),
            reference_images: None,
            account: defaults.account.clone().or(raw.account),
        };

        match Scene::normalize(&merged, limits, line) {
            Ok(mut scene) => {
                scene.reference_images = merged_refs;
                jobs.push(VideoJob {
                    node_id: video_id.to_string(),
                    scene_index,
                    scene,
                });
            }
            Err(e) => errors.push(scene_error(&src_id, &e)),
        }
    }
    jobs
}

/// Cạnh nối vào node `to`, cổng thuộc `ports` — GIỮ NGUYÊN thứ tự khai báo trong `g.edges`.
fn incoming<'g>(g: &'g Graph, to: &str, ports: &[&str]) -> impl Iterator<Item = &'g Edge> {
    g.edges
        .iter()
        .filter(move |e| e.to == to && ports.contains(&e.to_port.as_str()))
}

/// Gộp ảnh tham chiếu: cổng `refs` TRƯỚC, `refs=` trong TXT SAU; loại phần tử trùng/rỗng,
/// giữ đúng lần xuất hiện ĐẦU TIÊN (ruling task-2, xem "Refs merge" trong báo cáo).
fn merge_refs(port_refs: &[String], txt_refs: &[String]) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for r in port_refs.iter().chain(txt_refs.iter()) {
        let r = r.trim();
        if r.is_empty() {
            continue;
        }
        if seen.insert(r.to_string()) {
            out.push(r.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LogoMode, Mode, Model, Ratio};

    fn prompt_node(id: &str, text: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Prompt {
                text: text.to_string(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn image_upload_node(id: &str, paths: &[&str]) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::ImageUpload {
                paths: paths.iter().map(|p| (*p).to_string()).collect(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn video_node(id: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Video {
                mode: Mode::Fast,
                model: Model::Seedance20,
                ratio: Ratio::R9x16,
                duration: 10,
                logo: LogoMode::Off,
                account: String::new(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn save_node(id: &str, out_dir: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Save {
                out_dir: out_dir.to_string(),
            },
            x: 0.0,
            y: 0.0,
        }
    }

    fn edge(id: &str, from: &str, from_port: &str, to: &str, to_port: &str) -> Edge {
        Edge {
            id: id.to_string(),
            from: from.to_string(),
            from_port: from_port.to_string(),
            to: to.to_string(),
            to_port: to_port.to_string(),
        }
    }

    /// Đồ thị mẫu Prompt(text) → Tạo video → Lưu, dùng lại ở nhiều test.
    fn fixture(text: &str) -> Graph {
        Graph {
            version: 1,
            nodes: vec![
                prompt_node("n1", text),
                video_node("n2"),
                save_node("n3", "downloads"),
            ],
            edges: vec![
                edge("e1", "n1", "out", "n2", "prompt"),
                edge("e2", "n2", "out", "n3", "video"),
            ],
        }
    }

    #[test]
    fn prompt_3_dong_ra_video_3_job_danh_so_1_den_3() {
        let g = fixture("canh 1\ncanh 2\ncanh 3");
        let rp = plan(&g, &Limits::default()).unwrap();
        let Step::Video { jobs, .. } = rp
            .steps
            .iter()
            .find(|s| matches!(s, Step::Video { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(jobs.len(), 3);
        assert_eq!(
            jobs.iter().map(|j| j.scene_index).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(jobs[0].scene.prompt, "canh 1");
        assert_eq!(jobs[2].scene.prompt, "canh 3");
        assert_eq!(rp.video_total, 3);
    }

    #[test]
    fn txt_ratio_thang_mac_dinh_node() {
        // Node co ratio mac dinh KHAC voi Ratio::default() (R9x16) de phan biet ro voi
        // mac dinh cung cua Scene::normalize.
        let mut g = fixture("canh 1 | ratio=16:9\ncanh 2");
        if let NodeKind::Video { ratio, .. } = &mut g.nodes[1].kind {
            *ratio = Ratio::R1x1;
        }
        let rp = plan(&g, &Limits::default()).unwrap();
        let Step::Video { jobs, .. } = rp
            .steps
            .iter()
            .find(|s| matches!(s, Step::Video { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(jobs[0].scene.ratio, Ratio::R16x9, "TXT ratio=16:9 thang");
        assert_eq!(
            jobs[1].scene.ratio,
            Ratio::R1x1,
            "khong co TXT -> mac dinh node"
        );
    }

    #[test]
    fn refs_tu_image_upload_gan_vao_moi_canh_va_gop_voi_txt_refs() {
        let mut g = fixture("canh 1\ncanh 2 | refs=p3.jpg,p1.jpg");
        g.nodes.push(image_upload_node("n4", &["p1.jpg", "p2.jpg"]));
        g.edges.push(edge("e3", "n4", "out", "n2", "refs"));

        let rp = plan(&g, &Limits::default()).unwrap();
        let Step::Video { jobs, .. } = rp
            .steps
            .iter()
            .find(|s| matches!(s, Step::Video { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(jobs[0].scene.reference_images, vec!["p1.jpg", "p2.jpg"]);
        // p1.jpg da co tu cong -> khong lap lai; p3.jpg moi duoc noi sau, giu thu tu.
        assert_eq!(
            jobs[1].scene.reference_images,
            vec!["p1.jpg", "p2.jpg", "p3.jpg"]
        );
    }

    #[test]
    fn vuot_qua_tran_anh_tham_chieu_bao_loi_too_many_refs() {
        let mut g = fixture("canh 1");
        g.nodes.push(image_upload_node("n4", &["p1.jpg", "p2.jpg"]));
        g.edges.push(edge("e3", "n4", "out", "n2", "refs"));

        let limits = Limits {
            reference_image_max_count: 1,
            ..Limits::default()
        };
        let errs = plan(&g, &limits).unwrap_err();
        assert!(errs.contains(&GraphError::TooManyRefs {
            node: "n2".to_string(),
            scene_index: 1,
            count: 2,
            max: 1,
        }));
    }

    #[test]
    fn hai_node_prompt_vao_1_video_noi_chi_so_gop() {
        let mut g = Graph {
            version: 1,
            nodes: vec![
                prompt_node("n1", "a\nb"),
                prompt_node("n1b", "c"),
                video_node("n2"),
                save_node("n3", "downloads"),
            ],
            edges: vec![
                edge("e1", "n1", "out", "n2", "prompt"),
                edge("e1b", "n1b", "out", "n2", "prompt"),
                edge("e2", "n2", "out", "n3", "video"),
            ],
        };
        // Sap xep node de dam bao thu tu topo khong tinh co lam dung "thu tu canh":
        // e1 (n1) khai bao truoc e1b (n1b) trong g.edges nen "a","b" phai truoc "c".
        g.edges.sort_by_key(|e| e.id.clone());

        let rp = plan(&g, &Limits::default()).unwrap();
        let Step::Video { jobs, .. } = rp
            .steps
            .iter()
            .find(|s| matches!(s, Step::Video { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(
            jobs.iter()
                .map(|j| j.scene.prompt.clone())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c"]
        );
        assert_eq!(
            jobs.iter().map(|j| j.scene_index).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn save_noi_video_ra_savejob_co_from_node() {
        let g = fixture("canh 1");
        let rp = plan(&g, &Limits::default()).unwrap();
        let save_job = rp
            .steps
            .iter()
            .find_map(|s| match s {
                Step::Save(job) => Some(job),
                _ => None,
            })
            .unwrap();
        assert_eq!(save_job.node_id, "n3");
        assert_eq!(save_job.out_dir, "downloads");
        assert_eq!(save_job.from_node, "n2");
    }

    #[test]
    fn save_mot_savejob_cho_moi_canh_vao() {
        let mut g = Graph {
            version: 1,
            nodes: vec![
                prompt_node("n1", "a"),
                video_node("n2"),
                prompt_node("n1b", "b"),
                video_node("n2b"),
                save_node("n3", "downloads"),
            ],
            edges: vec![
                edge("e1", "n1", "out", "n2", "prompt"),
                edge("e1b", "n1b", "out", "n2b", "prompt"),
                edge("e2", "n2", "out", "n3", "video"),
                edge("e2b", "n2b", "out", "n3", "video"),
            ],
        };
        g.edges.sort_by_key(|e| e.id.clone());
        let rp = plan(&g, &Limits::default()).unwrap();
        let save_jobs: Vec<&SaveJob> = rp
            .steps
            .iter()
            .filter_map(|s| match s {
                Step::Save(job) => Some(job),
                _ => None,
            })
            .collect();
        assert_eq!(save_jobs.len(), 2);
        assert_eq!(
            save_jobs
                .iter()
                .map(|j| j.from_node.clone())
                .collect::<Vec<_>>(),
            vec!["n2".to_string(), "n2b".to_string()]
        );
    }

    #[test]
    fn video_total_la_tong_job_qua_nhieu_node_video() {
        let g = Graph {
            version: 1,
            nodes: vec![
                prompt_node("n1", "a\nb"),
                video_node("n2"),
                prompt_node("n1b", "c"),
                video_node("n2b"),
                save_node("n3", "downloads"),
            ],
            edges: vec![
                edge("e1", "n1", "out", "n2", "prompt"),
                edge("e1b", "n1b", "out", "n2b", "prompt"),
                edge("e2", "n2", "out", "n3", "video"),
                edge("e2b", "n2b", "out", "n3", "video"),
            ],
        };
        let rp = plan(&g, &Limits::default()).unwrap();
        assert_eq!(rp.video_total, 3);
    }

    #[test]
    fn do_thi_khong_hop_le_thi_tra_nguyen_ven_loi_validate() {
        let g = Graph {
            version: 1,
            nodes: vec![],
            edges: vec![],
        };
        assert_eq!(plan(&g, &Limits::default()), Err(validate(&g).unwrap_err()));
    }

    #[test]
    fn video_prompt_rong_thi_loi_scene_khong_co_canh_nao() {
        let g = fixture("   \n\n  ");
        let errs = plan(&g, &Limits::default()).unwrap_err();
        assert!(errs.contains(&GraphError::Scene {
            node: "n2".to_string(),
            line: None,
            message: "khong co canh nao".to_string(),
        }));
    }

    #[test]
    fn prompt_don_doc_khong_noi_edge_van_ra_step_prompt() {
        let g = Graph {
            version: 1,
            nodes: vec![prompt_node("n1", "chi mot canh")],
            edges: vec![],
        };
        let rp = plan(&g, &Limits::default()).unwrap();
        assert_eq!(rp.steps.len(), 1);
        match &rp.steps[0] {
            Step::Prompt { node_id, scenes } => {
                assert_eq!(node_id, "n1");
                assert_eq!(scenes.len(), 1);
                assert_eq!(scenes[0].prompt, "chi mot canh");
            }
            _ => unreachable!(),
        }
        assert_eq!(rp.video_total, 0);
    }

    #[test]
    fn image_upload_don_doc_khong_noi_edge_van_ra_step() {
        let g = Graph {
            version: 1,
            nodes: vec![image_upload_node("n1", &["a.jpg", "b.jpg"])],
            edges: vec![],
        };
        let rp = plan(&g, &Limits::default()).unwrap();
        assert_eq!(
            rp.steps,
            vec![Step::ImageUpload {
                node_id: "n1".to_string(),
                paths: vec!["a.jpg".to_string(), "b.jpg".to_string()],
            }]
        );
    }

    #[test]
    fn account_node_thang_neu_khac_rong_con_khong_thi_lay_txt_acc() {
        let mut g = fixture("canh 1 | acc=tu_txt\ncanh 2 | acc=tu_txt");
        if let NodeKind::Video { account, .. } = &mut g.nodes[1].kind {
            *account = "tu_node".to_string();
        }
        let rp = plan(&g, &Limits::default()).unwrap();
        let Step::Video { jobs, .. } = rp
            .steps
            .iter()
            .find(|s| matches!(s, Step::Video { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        // account node khac rong -> node thang, du TXT co acc=.
        assert_eq!(jobs[0].scene.account, "tu_node");
        assert_eq!(jobs[1].scene.account, "tu_node");
    }

    #[test]
    fn account_node_rong_thi_lay_txt_acc() {
        let g = fixture("canh 1 | acc=tu_txt\ncanh 2");
        let rp = plan(&g, &Limits::default()).unwrap();
        let Step::Video { jobs, .. } = rp
            .steps
            .iter()
            .find(|s| matches!(s, Step::Video { .. }))
            .unwrap()
        else {
            unreachable!()
        };
        assert_eq!(jobs[0].scene.account, "tu_txt");
        assert_eq!(jobs[1].scene.account, "");
    }

    #[test]
    fn vuot_tran_tong_so_canh_gop_bao_loi_scene() {
        let l = Limits {
            batch_max_scenes: 2,
            ..Limits::default()
        };
        let g = fixture("a\nb\nc");
        let errs = plan(&g, &l).unwrap_err();
        assert!(
            errs.iter()
                .any(|e| matches!(e, GraphError::Scene { node, .. } if node == "n2"))
        );
    }

    #[test]
    fn so_canh_prompt_bang_so_job_video_khi_co_dong_comment_va_dong_trong() {
        // Fix round 1 (review Important #2): ghim bat bien noi trong doc comment cua plan()
        // — parse_txt (Step::Prompt) va parse_lines_raw (Step::Video) dung chung parse_line
        // va cung bo qua dong trong/`#`, nen 3 dong tho (1 canh that + 1 comment + 1 dong
        // trong) phai cho ra CUNG so luong o ca hai ben (o day: 1).
        let g = fixture("canh that\n# ghi chu\n   \n");
        let rp = plan(&g, &Limits::default()).unwrap();

        let prompt_len = rp
            .steps
            .iter()
            .find_map(|s| match s {
                Step::Prompt { scenes, .. } => Some(scenes.len()),
                _ => None,
            })
            .unwrap();
        let video_len = rp
            .steps
            .iter()
            .find_map(|s| match s {
                Step::Video { jobs, .. } => Some(jobs.len()),
                _ => None,
            })
            .unwrap();

        assert_eq!(
            prompt_len, video_len,
            "Step::Prompt va Step::Video phai dem cung so canh cho cung 1 node Prompt"
        );
        assert_eq!(
            prompt_len, 1,
            "chi 1 dong that, comment/dong trong bi bo qua"
        );
    }

    #[test]
    fn json_dung_camelcase_khop_quy_uoc_graph_error() {
        // I1 (review Important #1): RunPlan/Step/VideoJob/SaveJob phai cung quy uoc
        // camelCase voi GraphError (fix round 1 Task 1) de Task 9 (Tauri IPC) khong can
        // them lop DTO. Dung plan() that tren fixture 1 Prompt/1 Video/1 Save de ghim ca
        // 3 buoc trong cung 1 JSON, khop dung field ten nhieu tu: videoTotal, sceneIndex,
        // nodeId, outDir, fromNode; tag "type" van snake_case (khop NodeKind."kind").
        let g = fixture("canh 1");
        let rp = plan(&g, &Limits::default()).unwrap();
        let value = serde_json::to_value(&rp).unwrap();

        assert!(value.get("videoTotal").is_some(), "{value:?}");
        assert!(value.get("video_total").is_none(), "{value:?}");

        let steps = value
            .get("steps")
            .and_then(serde_json::Value::as_array)
            .unwrap();
        let tags: Vec<&str> = steps
            .iter()
            .map(|s| s.get("type").and_then(serde_json::Value::as_str).unwrap())
            .collect();
        assert_eq!(tags, vec!["prompt", "video", "save"]);

        let prompt_step = &steps[0];
        assert!(prompt_step.get("nodeId").is_some(), "{prompt_step:?}");
        assert!(prompt_step.get("node_id").is_none(), "{prompt_step:?}");

        let video_step = &steps[1];
        let job = &video_step
            .get("jobs")
            .and_then(serde_json::Value::as_array)
            .unwrap()[0];
        assert!(job.get("nodeId").is_some(), "{job:?}");
        assert!(job.get("sceneIndex").is_some(), "{job:?}");
        assert!(job.get("scene_index").is_none(), "{job:?}");

        let save_step = &steps[2];
        assert!(save_step.get("nodeId").is_some(), "{save_step:?}");
        assert!(save_step.get("outDir").is_some(), "{save_step:?}");
        assert!(save_step.get("fromNode").is_some(), "{save_step:?}");
        assert!(save_step.get("out_dir").is_none(), "{save_step:?}");
        assert!(save_step.get("from_node").is_none(), "{save_step:?}");
    }
}
