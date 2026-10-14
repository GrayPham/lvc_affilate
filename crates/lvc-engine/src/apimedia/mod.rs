//! P6: dịch vụ host cho các node gọi **API chính thức** (Google, Seedance, Grok, TTS `ElevenLabs`/
//! `MiniMax`). Khác `veo3.rs` (điều khiển trình duyệt Flow): ở đây chỉ HTTP thuần qua client provider.
//!
//! Ranh giới bí mật (giống `textgen.rs`): node chỉ truyền TÊN credential trong job. Engine resolve
//! tên đó → API key qua [`TextSecretResolver`] (lvc-app impl bằng `Vault`), rồi gọi client. API key
//! CHỈ nằm trong RAM engine giữa lúc resolve và lúc gọi HTTP — KHÔNG ra event/log/UI. Engine KHÔNG
//! `match` theo kind: các dịch vụ này được chèn VÔ ĐIỀU KIỆN vào `Services` trong `dag::start_run`.
//!
//! `local_path`: engine tự quyết theo `run_id`/`node_id`/`scene_index` (mirror `veo3::EngineVeoJobs`),
//! provider KHÔNG biết đường dẫn file. Chia module theo nhà cung cấp (luật repo): [`google`] ·
//! [`seedance`] · [`grok`]; [`util`] giữ hàm dùng chung (đặt tên file, ghi bytes, đọc data URL).

mod google;
mod grok;
mod seedance;
pub(crate) mod task_tracking;
mod tts;
mod util;

pub(crate) use google::{EngineGoogleImage, EngineGoogleVideo};
pub(crate) use grok::EngineGrokImage;
pub(crate) use seedance::{EngineSeedanceImage, EngineSeedanceVideo};
pub(crate) use tts::EngineTts;
