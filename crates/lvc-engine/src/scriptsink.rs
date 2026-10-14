//! Dịch vụ [`lvc_scripts::ScriptSinkHost`] do engine cấp cho node `script_gen` — lưu kịch bản AI
//! sinh ra (trong lúc workflow CHẠY) thành một dòng mới trong kho "Kịch bản" (bảng
//! `scripts`/`scenes`, xem `lvc_store::scripts`). Node KHÔNG tự đụng `lvc-store`/`sqlx` (ADR-R014);
//! trait này là ranh giới duy nhất.
//!
//! `dir` của kịch bản mới để RỖNG — giống hành vi "Lưu" tay nhanh trong tab Kịch bản khi chưa chọn
//! thư mục; người dùng chỉnh lại sau trong tab Kịch bản nếu cần. Mỗi cảnh lưu NGUYÊN VĂN dòng do
//! node sinh ra, các cột ratio/duration/model/mode/logo để `NULL` (xem doc `MEMORY.md` crate
//! `lvc-scripts`) — `Engine::run_script` tự điền mặc định từ `AppSettings` lúc CHẠY, không phải
//! lúc lưu này.

use async_trait::async_trait;
use lvc_scripts::{ScriptDraft, ScriptSinkError, ScriptSinkHost};
use lvc_store::{Db, SceneIn, ScriptRepo};

pub(crate) struct EngineScriptSinkHost {
    db: Db,
}

impl EngineScriptSinkHost {
    pub(crate) fn new(db: Db) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ScriptSinkHost for EngineScriptSinkHost {
    async fn save_script(&self, draft: ScriptDraft) -> Result<String, ScriptSinkError> {
        let repo = ScriptRepo::new(&self.db);
        let row = repo
            .create(&draft.name, "")
            .await
            .map_err(|e| ScriptSinkError::Save(e.to_string()))?;
        let scenes = draft
            .scenes
            .into_iter()
            .map(|prompt| SceneIn {
                prompt,
                ..SceneIn::default()
            })
            .collect();
        repo.replace_scenes(&row.id, scenes)
            .await
            .map_err(|e| ScriptSinkError::Save(e.to_string()))?;
        Ok(row.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn luu_kich_ban_tao_dong_moi_voi_dung_so_canh() {
        let db = Db::open_memory().await.unwrap();
        let host = EngineScriptSinkHost::new(db.clone());
        let id = host
            .save_script(ScriptDraft {
                name: "giày chạy bộ".to_string(),
                scenes: vec!["cảnh 1".to_string(), "cảnh 2".to_string()],
            })
            .await
            .unwrap();
        let repo = ScriptRepo::new(&db);
        let saved = repo.get(&id).await.unwrap().unwrap();
        assert_eq!(saved.name, "giày chạy bộ");
        let scenes = repo.list_scenes(&id).await.unwrap();
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes[0].prompt, "cảnh 1");
    }
}
