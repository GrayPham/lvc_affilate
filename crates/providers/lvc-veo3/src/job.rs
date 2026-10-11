//! Kieu "cong viec" cho 2 trait host cap cao ([`crate::VeoImageHost`]/[`crate::VeoVideoHost`]) —
//! khop khuon `lvc_dola::DolaVideoHost` (`SceneJob`/`JobsReport`): trait KHONG biet duong dan file
//! (`local_path` do phia impl — `lvc-engine` — tu quyet dinh theo `run_id`/`node_id`, giong
//! `run_video_jobs`), chi mang du lieu can de GOI request + anh tham chieu can UPLOAD (duong dan
//! local, node da co san tu `Inputs::assets`).

use std::path::PathBuf;

use crate::model::{Aspect, ImageModel};

/// Mot lan tao anh (`GEM_PIX_2`/Nano Banana) — dong bo, tra ve NGAY khi `VeoImageHost::generate_images`
/// xu ly xong job nay.
#[derive(Debug, Clone)]
pub struct ImageJob {
    /// Chi so canh 1-based (bao loi dung cho, khop `scene_index` cua Dola).
    pub scene_index: u32,
    /// Id credential `google/login` trong `lvc-vault`.
    pub credential_id: String,
    pub prompt: String,
    pub model: ImageModel,
    pub aspect: Aspect,
    pub seed: i64,
    /// Duong dan LOCAL cua anh tham chieu (node da tai/nhan qua cong `refs`) — impl tu doc + upload
    /// (`rpc::upload`) TRUOC khi goi `rpc::image`, gan theo project hien tai.
    pub ref_image_paths: Vec<PathBuf>,
}

/// Mot anh da tao xong.
#[derive(Debug, Clone)]
pub struct ImageJobDone {
    pub scene_index: u32,
    /// Duong dan FILE DA LUU (impl tu chon theo `run_id`/`node_id`/`scene_index`, download tu URL
    /// Google tra ve bang [`crate::flow::Flow::download`]).
    pub local_path: PathBuf,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// Ket qua `VeoImageHost::generate_images` — co the thanh cong MOT PHAN (giong `JobsReport` cua
/// Dola: `failed` la nhan canh + ly do, KHONG lam hong ca run).
#[derive(Debug, Default)]
pub struct ImageJobsReport {
    pub done: Vec<ImageJobDone>,
    /// Mot dong moi canh loi: `"canh <scene_index>: <thong diep>"`.
    pub failed: Vec<String>,
}

/// Nguon cho 1 job video — quyet dinh rpc goi (`text_to_video` hay `reference_to_video`). CHUA co
/// bien the I2V/Interpolation (spec P5 §2.5 diem 2: `eb1hJf`/`nprQif` chua verify cURL thuc — de
/// sau khi co bang chung song).
#[derive(Debug, Clone)]
pub enum VideoSource {
    Text {
        prompt: String,
    },
    /// 1-6 anh tham chieu (duong dan LOCAL — impl tu upload truoc khi goi `rpc::reference_to_video`).
    Reference {
        prompt: String,
        ref_image_paths: Vec<PathBuf>,
    },
}

/// Mot lan tao video — BAT DOI XUNG: `VeoVideoHost::generate_videos` submit roi TU POLL toi khi
/// xong/qua han (khac anh, tra ve ngay).
#[derive(Debug, Clone)]
pub struct VideoJob {
    pub scene_index: u32,
    pub credential_id: String,
    pub source: VideoSource,
    /// Model Google (vd `veo_3_1_t2v_lite_low_priority`, `abra_r2v_8s`) — xem
    /// `model::default_video_model` cho gia tri mac dinh theo mode.
    pub model: String,
    pub aspect: Aspect,
    /// So spec/bien the trong 1 lan goi (T2V/I2V/Interpolation) — R2V LUON ep = 1 o tang goi rpc
    /// (xem `rpc::reference_to_video`), tham so nay bi bo qua cho `VideoSource::Reference`.
    pub seed_count: u32,
}

#[derive(Debug, Clone)]
pub struct VideoJobDone {
    pub scene_index: u32,
    pub local_path: PathBuf,
}

#[derive(Debug, Default)]
pub struct VideoJobsReport {
    pub done: Vec<VideoJobDone>,
    pub failed: Vec<String>,
}
