//! P3c Task 1 — bang `scripts`/`scenes` (migration `0003_scripts.sql`), CRUD tho giong het
//! `src/core/scripts_store.py::ScriptStore` (khong validate gia tri Scene o day — do la viec
//! cua `lvc-engine::scripts` luc CHAY, xem doc do). Kich ban la goc; task la "lan chay" cua
//! mot canh: `scenes.task_id` tro task MOI NHAT trong bang `tasks` (P0).

use std::collections::HashMap;

use lvc_core::TaskStatus;
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::{Db, StoreError};

fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// `uuid v4` 12 ky tu hex dau (giong Python `uuid.uuid4().hex[:12]` — `scripts_store.py:70/198`).
fn new_id(prefix: &str) -> String {
    let raw = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}_{}", &raw[..12])
}

/// `name.strip()[:80]` (dem theo KY TU, khong phai byte — `scripts_store.py:72`).
fn clip_name(name: &str) -> String {
    name.trim().chars().take(80).collect()
}

/// Mot dong `scripts` day du (khong kem so dem — [`ScriptMeta`] moi co so dem).
///
/// `Serialize`/`Deserialize` camelCase: `script_create`/`script_update` (lvc-app) tra thang
/// kieu nay cho IPC, giong `WorkflowRow`/`WorkflowMeta` (`lvc_store::workflows`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRow {
    pub id: String,
    pub name: String,
    pub dir: String,
    pub created_at: f64,
    pub updated_at: f64,
    /// `kind` node Video dung khi Chay (vd `"dola_video"`/`"veo3_video"`/`"google_video"`/
    /// `"seedance_video"`) — migration `0006_scripts_video_config.sql`, mac dinh `"dola_video"`.
    pub video_kind: String,
    /// Tham so node Video dang JSON tho; rong (`""`) = "chua dat" (xem
    /// `lvc_engine::scripts::effective_video_params`). KHONG giai ma o day — `ScriptRepo` khong
    /// biet/khong validate hinh dang params theo kind (ADR-R014).
    pub video_params: String,
    /// Nhan phan loai tu do (vd "Affiliate") — khong rang buoc enum.
    pub category: String,
}

fn row_to_script(r: &sqlx::sqlite::SqliteRow) -> Result<ScriptRow, StoreError> {
    Ok(ScriptRow {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        dir: r.try_get("dir")?,
        created_at: r.try_get("created_at")?,
        updated_at: r.try_get("updated_at")?,
        video_kind: r.try_get("video_kind")?,
        video_params: r.try_get("video_params")?,
        category: r.try_get("category")?,
    })
}

/// [`ScriptRow`] + so dem canh theo trang thai task — `scripts_store.py::list_scripts` (don gian
/// hoa: gop `processing`/`queued` cua Python thanh MOT so `running`, xem task-1-brief.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptMeta {
    pub id: String,
    pub name: String,
    pub dir: String,
    pub created_at: f64,
    pub updated_at: f64,
    pub total: i64,
    pub done: i64,
    pub failed: i64,
    pub running: i64,
    pub video_kind: String,
    pub video_params: String,
    pub category: String,
}

const COUNTS_SQL: &str = "SELECT s.id, s.name, s.dir, s.created_at, s.updated_at,
       s.video_kind, s.video_params, s.category,
       COUNT(sc.id) AS total,
       SUM(CASE WHEN t.status = ?1 THEN 1 ELSE 0 END) AS done,
       SUM(CASE WHEN t.status = ?2 THEN 1 ELSE 0 END) AS failed,
       SUM(CASE WHEN t.status IN (?3, ?4) THEN 1 ELSE 0 END) AS running
     FROM scripts s
     LEFT JOIN scenes sc ON sc.script_id = s.id
     LEFT JOIN tasks t ON t.id = sc.task_id";

fn row_to_meta(r: &sqlx::sqlite::SqliteRow) -> Result<ScriptMeta, StoreError> {
    // COUNT/SUM tra NULL khi khong co dong nao duoc gop (kich ban chua co canh nao) — ve 0,
    // giong Python `int(d[k] or 0)` (`scripts_store.py:123-124`).
    let done: Option<i64> = r.try_get("done")?;
    let failed: Option<i64> = r.try_get("failed")?;
    let running: Option<i64> = r.try_get("running")?;
    Ok(ScriptMeta {
        id: r.try_get("id")?,
        name: r.try_get("name")?,
        dir: r.try_get("dir")?,
        created_at: r.try_get("created_at")?,
        updated_at: r.try_get("updated_at")?,
        total: r.try_get("total")?,
        done: done.unwrap_or(0),
        failed: failed.unwrap_or(0),
        running: running.unwrap_or(0),
        video_kind: r.try_get("video_kind")?,
        video_params: r.try_get("video_params")?,
        category: r.try_get("category")?,
    })
}

/// Mot dong `scenes` day du, `refs` da giai ma JSON. Khong kem trang thai task (xem
/// [`ScriptRepo::scene_task_statuses`] cho rieng phan do — canh y het `scripts_store.py::scene`/
/// `list_scenes` TRUOC khi ghep `task_status`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneRow {
    pub id: String,
    pub script_id: String,
    pub idx: i64,
    pub prompt: String,
    pub ratio: Option<String>,
    pub duration: Option<i64>,
    pub model: Option<String>,
    pub mode: Option<String>,
    pub logo: Option<String>,
    pub account: String,
    pub refs: Vec<String>,
    pub task_id: Option<String>,
    pub updated_at: f64,
}

fn row_to_scene(r: &sqlx::sqlite::SqliteRow) -> Result<SceneRow, StoreError> {
    let refs_json: String = r.try_get("refs")?;
    // `json.loads(d.get("refs") or "[]")`, nuot loi ve `[]` (`scripts_store.py:130-136`) — cot
    // NOT NULL DEFAULT '[]' nen chi phong thu du lieu bi sua tay/hong.
    let refs = serde_json::from_str(&refs_json).unwrap_or_default();
    Ok(SceneRow {
        id: r.try_get("id")?,
        script_id: r.try_get("script_id")?,
        idx: r.try_get("idx")?,
        prompt: r.try_get("prompt")?,
        ratio: r.try_get("ratio")?,
        duration: r.try_get("duration")?,
        model: r.try_get("model")?,
        mode: r.try_get("mode")?,
        logo: r.try_get("logo")?,
        account: r.try_get("account")?,
        refs,
        task_id: r.try_get("task_id")?,
        updated_at: r.try_get("updated_at")?,
    })
}

/// Canh dau vao cho [`ScriptRepo::replace_scenes`]/`script_put_scenes` (lvc-app) — khop
/// `scripts_api.py::SceneIn`, TRU: gia tri thieu/rong luu NULL (khong ep mac dinh cung "9:16"/
/// "seedance-2.0"/"agent" nhu Python) de `lvc-engine::scripts::run_script` tu dien mac dinh tu
/// `AppSettings` luc CHAY (xem task-1-brief.md) — `ScriptRepo` KHONG validate gia tri o day.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneIn {
    pub id: Option<String>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub ratio: Option<String>,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub account: String,
    #[serde(default)]
    pub refs: Vec<String>,
}

/// Loi rieng cua [`ScriptRepo::replace_scenes`] — canh dang chay (`queued`/`processing`) ma bi
/// bo khoi mang truyen vao thi TU CHOI CA LOT (khong ghi gi), giong
/// `scripts_store.py::SceneRunningError` (`scripts_store.py:171-177`).
#[derive(Debug, thiserror::Error)]
pub enum ReplaceScenesError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("canh dang chay khong the bi go khoi kich ban: {0:?}")]
    SceneRunning(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct ScriptRepo {
    db: Db,
}

impl ScriptRepo {
    #[must_use]
    pub fn new(db: &Db) -> Self {
        Self { db: db.clone() }
    }

    /// `create_script` (`scripts_store.py:68-76`): ten rong sau khi cat -> "Kịch bản {n+1}"
    /// (`n` = so kich ban da co TRUOC khi tao dong nay).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi/doc lai that bai.
    pub async fn create(&self, name: &str, dir: &str) -> Result<ScriptRow, StoreError> {
        let id = new_id("script");
        let ts = now();
        let clean = clip_name(name);
        let clean = if clean.is_empty() {
            self.next_default_name().await?
        } else {
            clean
        };
        sqlx::query(
            "INSERT INTO scripts (id, name, dir, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?4)",
        )
        .bind(&id)
        .bind(&clean)
        .bind(dir.trim())
        .bind(ts)
        .execute(self.db.pool())
        .await?;
        self.get(&id).await?.ok_or_else(|| {
            StoreError::BadPath(format!("script {id} vua tao nhung doc lai khong thay"))
        })
    }

    async fn next_default_name(&self) -> Result<String, StoreError> {
        let row = sqlx::query("SELECT COUNT(*) AS c FROM scripts")
            .fetch_one(self.db.pool())
            .await?;
        let n: i64 = row.try_get("c")?;
        Ok(format!("Kịch bản {}", n + 1))
    }

    /// # Errors
    /// `StoreError::Sqlx` khi truy van that bai.
    pub async fn get(&self, id: &str) -> Result<Option<ScriptRow>, StoreError> {
        let row = sqlx::query("SELECT * FROM scripts WHERE id = ?1")
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_to_script).transpose()
    }

    /// `update_script` (`scripts_store.py:83-95`): `None` = giu nguyen; `Some` = ghi de (rong
    /// hop le cho `dir`, ten rong -> "Kịch bản" nguyen van nhu Python).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi/doc lai that bai; coi id khong ton tai la [`StoreError::BadPath`].
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        dir: Option<&str>,
    ) -> Result<ScriptRow, StoreError> {
        let ts = now();
        if let Some(name) = name {
            let clean = clip_name(name);
            let clean = if clean.is_empty() {
                "Kịch bản".to_string()
            } else {
                clean
            };
            sqlx::query("UPDATE scripts SET name = ?2, updated_at = ?3 WHERE id = ?1")
                .bind(id)
                .bind(&clean)
                .bind(ts)
                .execute(self.db.pool())
                .await?;
        }
        if let Some(dir) = dir {
            sqlx::query("UPDATE scripts SET dir = ?2, updated_at = ?3 WHERE id = ?1")
                .bind(id)
                .bind(dir.trim())
                .bind(ts)
                .execute(self.db.pool())
                .await?;
        }
        if name.is_none() && dir.is_none() {
            sqlx::query("UPDATE scripts SET updated_at = ?2 WHERE id = ?1")
                .bind(id)
                .bind(ts)
                .execute(self.db.pool())
                .await?;
        }
        self.get(id)
            .await?
            .ok_or_else(|| StoreError::BadPath(format!("script {id} khong ton tai")))
    }

    /// Dat engine Video + tham so dung khi Chay kich ban. `video_params` la CHUOI JSON tho (rong
    /// = "chua dat", xem doc [`ScriptRow::video_params`]) — `ScriptRepo` khong validate `kind`/
    /// hinh dang params (viec do thuoc lop goi, xem `lvc-app::commands::scripts::
    /// script_set_video_config`, dung `Registry::manifests()` de kiem `kind` ton tai).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi/doc lai that bai; coi id khong ton tai la [`StoreError::BadPath`].
    pub async fn set_video_config(
        &self,
        id: &str,
        video_kind: &str,
        video_params: &str,
    ) -> Result<ScriptRow, StoreError> {
        sqlx::query(
            "UPDATE scripts SET video_kind = ?2, video_params = ?3, updated_at = ?4 WHERE id = ?1",
        )
        .bind(id)
        .bind(video_kind)
        .bind(video_params)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        self.get(id)
            .await?
            .ok_or_else(|| StoreError::BadPath(format!("script {id} khong ton tai")))
    }

    /// Dat nhan phan loai tu do (vd "Affiliate") — khong rang buoc gia tri.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi/doc lai that bai; coi id khong ton tai la [`StoreError::BadPath`].
    pub async fn set_category(&self, id: &str, category: &str) -> Result<ScriptRow, StoreError> {
        sqlx::query("UPDATE scripts SET category = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(category.trim())
            .bind(now())
            .execute(self.db.pool())
            .await?;
        self.get(id)
            .await?
            .ok_or_else(|| StoreError::BadPath(format!("script {id} khong ton tai")))
    }

    /// `delete_script` (`scripts_store.py:97-104`): xoa CA canh ROI kich ban, tra ve `task_id`
    /// cua moi canh co task (KHONG dung toi dong `tasks` — xoa khong huy task, R3).
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi/doc that bai.
    pub async fn delete(&self, id: &str) -> Result<Vec<String>, StoreError> {
        let rows =
            sqlx::query("SELECT task_id FROM scenes WHERE script_id = ?1 AND task_id IS NOT NULL")
                .bind(id)
                .fetch_all(self.db.pool())
                .await?;
        let task_ids = rows
            .iter()
            .map(|r| r.try_get::<String, _>("task_id"))
            .collect::<Result<Vec<_>, _>>()?;
        sqlx::query("DELETE FROM scenes WHERE script_id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        sqlx::query("DELETE FROM scripts WHERE id = ?1")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        Ok(task_ids)
    }

    /// `list_scripts` (`scripts_store.py:106-126`), moi sua gan day nhat truoc.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy van that bai.
    pub async fn list_with_counts(&self) -> Result<Vec<ScriptMeta>, StoreError> {
        let sql = format!("{COUNTS_SQL} GROUP BY s.id ORDER BY s.updated_at DESC");
        let rows = sqlx::query(&sql)
            .bind(TaskStatus::Completed.as_str())
            .bind(TaskStatus::Failed.as_str())
            .bind(TaskStatus::Queued.as_str())
            .bind(TaskStatus::Processing.as_str())
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_to_meta).collect()
    }

    /// Nhu [`ScriptRepo::list_with_counts`] nhung chi MOT kich ban — dung boi lenh `script_get`
    /// (lvc-app) de tra `ScriptDoc.meta` ma khong phai loc ca danh sach.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy van that bai.
    pub async fn meta_for(&self, id: &str) -> Result<Option<ScriptMeta>, StoreError> {
        let sql = format!("{COUNTS_SQL} WHERE s.id = ?5 GROUP BY s.id");
        let row = sqlx::query(&sql)
            .bind(TaskStatus::Completed.as_str())
            .bind(TaskStatus::Failed.as_str())
            .bind(TaskStatus::Queued.as_str())
            .bind(TaskStatus::Processing.as_str())
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?;
        row.as_ref().map(row_to_meta).transpose()
    }

    /// `list_scenes` (phan doc — Python con don don `task_id` mo coi ngay trong ham nay;
    /// `ScriptRepo` KHONG lam vay, xem doc [`SceneRow`]), sap theo `idx`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy van that bai.
    pub async fn list_scenes(&self, script_id: &str) -> Result<Vec<SceneRow>, StoreError> {
        let rows = sqlx::query("SELECT * FROM scenes WHERE script_id = ?1 ORDER BY idx")
            .bind(script_id)
            .fetch_all(self.db.pool())
            .await?;
        rows.iter().map(row_to_scene).collect()
    }

    /// `scene_id -> trang thai task hien tai` cho MOI canh cua kich ban co `task_id` VA task do
    /// con ton tai — `running_scene_ids` (`scripts_store.py:154-159`) la tap con loc tu day
    /// (xem [`ScriptRepo::running_scene_ids`]); `lvc-engine::scripts::run_script` dung CA HAI
    /// nhanh (canh chua chay/da that bai VA canh dang chay) tu MOT lan truy van nay.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy van that bai.
    pub async fn scene_task_statuses(
        &self,
        script_id: &str,
    ) -> Result<HashMap<String, TaskStatus>, StoreError> {
        let rows = sqlx::query(
            "SELECT sc.id AS scene_id, t.status AS status FROM scenes sc
               JOIN tasks t ON t.id = sc.task_id
             WHERE sc.script_id = ?1",
        )
        .bind(script_id)
        .fetch_all(self.db.pool())
        .await?;
        let mut out = HashMap::with_capacity(rows.len());
        for r in &rows {
            let scene_id: String = r.try_get("scene_id")?;
            let status: String = r.try_get("status")?;
            if let Some(s) = TaskStatus::parse(&status) {
                out.insert(scene_id, s);
            }
        }
        Ok(out)
    }

    /// `running_scene_ids` (`scripts_store.py:154-159`): canh co task con `queued`/`processing`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy van that bai.
    pub async fn running_scene_ids(&self, script_id: &str) -> Result<Vec<String>, StoreError> {
        let statuses = self.scene_task_statuses(script_id).await?;
        Ok(statuses
            .into_iter()
            .filter(|(_, s)| matches!(s, TaskStatus::Queued | TaskStatus::Processing))
            .map(|(id, _)| id)
            .collect())
    }

    /// `replace_scenes` (`scripts_store.py:171-210`): ghi de CA mang — canh co `id` khop mot
    /// dong da co giu nguyen id (VA giu nguyen NOI DUNG neu dang chay, chi doi `idx`); canh
    /// KHONG co `id`/`id` la moi -> tao dong moi; dong CU khong con mat trong mang moi -> xoa.
    /// Canh dang chay ma bi bo khoi mang -> [`ReplaceScenesError::SceneRunning`] (TU CHOI CA
    /// LUOT, khong ghi gi — kiem TRUOC khi dung toi bat ky INSERT/UPDATE/DELETE nao).
    ///
    /// # Errors
    /// [`ReplaceScenesError::SceneRunning`] xem tren; [`ReplaceScenesError::Store`] khi DB loi.
    pub async fn replace_scenes(
        &self,
        script_id: &str,
        items: Vec<SceneIn>,
    ) -> Result<Vec<SceneRow>, ReplaceScenesError> {
        let running = self.running_scene_ids(script_id).await?;
        let wanted: std::collections::HashSet<&str> =
            items.iter().filter_map(|it| it.id.as_deref()).collect();
        let missing: Vec<String> = running
            .iter()
            .filter(|id| !wanted.contains(id.as_str()))
            .cloned()
            .collect();
        if !missing.is_empty() {
            let mut missing = missing;
            missing.sort_unstable();
            return Err(ReplaceScenesError::SceneRunning(missing));
        }

        let running: std::collections::HashSet<String> = running.into_iter().collect();
        Ok(self.replace_scenes_rows(script_id, items, &running).await?)
    }

    /// Phan ghi thuc su cua [`ScriptRepo::replace_scenes`] (SAU khi da kiem `SceneRunning`) —
    /// tach rieng de dung `Result<_, StoreError>` (moi loi o day deu la loi DB, khong con nhanh
    /// `SceneRunning` nao nua) roi `?` mot lan duy nhat o tren tu doi sang `ReplaceScenesError`.
    async fn replace_scenes_rows(
        &self,
        script_id: &str,
        items: Vec<SceneIn>,
        running: &std::collections::HashSet<String>,
    ) -> Result<Vec<SceneRow>, StoreError> {
        let old_rows = sqlx::query("SELECT id FROM scenes WHERE script_id = ?1")
            .bind(script_id)
            .fetch_all(self.db.pool())
            .await?;
        let old_ids: std::collections::HashSet<String> = old_rows
            .iter()
            .map(|r| r.try_get::<String, _>("id"))
            .collect::<Result<_, _>>()?;

        let ts = now();
        let mut keep: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (i, it) in items.iter().enumerate() {
            #[allow(
                clippy::cast_possible_wrap,
                reason = "so dong TXT/canh trong 1 kich ban nam rat xa duoi i64::MAX"
            )]
            let idx = (i + 1) as i64;
            let existing_id = it
                .id
                .as_deref()
                .filter(|id| old_ids.contains(*id))
                .map(str::to_string);

            if let Some(id) = existing_id.as_deref().filter(|id| running.contains(*id)) {
                // Canh dang chay: chi doi thu tu, GIU NGUYEN noi dung (scripts_store.py:185-188).
                sqlx::query("UPDATE scenes SET idx = ?2, updated_at = ?3 WHERE id = ?1")
                    .bind(id)
                    .bind(idx)
                    .bind(ts)
                    .execute(self.db.pool())
                    .await?;
                keep.insert(id.to_string());
                continue;
            }

            let refs = serde_json::to_string(&it.refs).unwrap_or_else(|_| "[]".to_string());
            // Fix-round 1 (C1 — Critical): gop moi xuong dong that (`\r\n`/`\r`/`\n`) + khoang
            // trang quanh no thanh MOT dau cach, trim — chuan hoa NGAY LUC GHI (bo sung phong thu
            // o `lvc_engine::scripts::run_script` khi DOC, xem doc `normalize_prompt_line`).
            // KHONG tu choi/bao loi o day (`ScriptRepo` khong validate gia tri, xem doc dau file)
            // — rong sau chuan hoa van luu duoc, `SCENE_EMPTY` la viec cua `run_script` luc CHAY.
            let prompt = lvc_core::normalize_prompt_line(&it.prompt);
            if let Some(id) = existing_id {
                sqlx::query(
                    "UPDATE scenes SET idx=?2,prompt=?3,ratio=?4,duration=?5,model=?6,mode=?7,\
                     logo=?8,account=?9,refs=?10,updated_at=?11 WHERE id=?1",
                )
                .bind(&id)
                .bind(idx)
                .bind(&prompt)
                .bind(it.ratio.as_deref())
                .bind(it.duration)
                .bind(it.model.as_deref())
                .bind(it.mode.as_deref())
                .bind(it.logo.as_deref())
                .bind(&it.account)
                .bind(&refs)
                .bind(ts)
                .execute(self.db.pool())
                .await?;
                keep.insert(id);
            } else {
                let id = new_id("scene");
                sqlx::query(
                    "INSERT INTO scenes (id,script_id,idx,prompt,ratio,duration,model,mode,\
                     logo,account,refs,task_id,updated_at) \
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,NULL,?12)",
                )
                .bind(&id)
                .bind(script_id)
                .bind(idx)
                .bind(&prompt)
                .bind(it.ratio.as_deref())
                .bind(it.duration)
                .bind(it.model.as_deref())
                .bind(it.mode.as_deref())
                .bind(it.logo.as_deref())
                .bind(&it.account)
                .bind(&refs)
                .bind(ts)
                .execute(self.db.pool())
                .await?;
                keep.insert(id);
            }
        }

        let gone: Vec<&String> = old_ids.iter().filter(|id| !keep.contains(*id)).collect();
        for id in gone {
            sqlx::query("DELETE FROM scenes WHERE id = ?1")
                .bind(id)
                .execute(self.db.pool())
                .await?;
        }
        sqlx::query("UPDATE scripts SET updated_at = ?2 WHERE id = ?1")
            .bind(script_id)
            .bind(ts)
            .execute(self.db.pool())
            .await?;

        self.list_scenes(script_id).await
    }

    /// `clear_scene_task` (`scripts_store.py:212-223`): canh khong ton tai hoac dang chay ->
    /// `false`; nguoc lai xoa `task_id` (task cu GIU NGUYEN, thanh video le) -> `true`.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi truy van/ghi that bai.
    pub async fn clear_scene_task(&self, scene_id: &str) -> Result<bool, StoreError> {
        let row = sqlx::query(
            "SELECT t.status AS status FROM scenes sc
               LEFT JOIN tasks t ON t.id = sc.task_id
             WHERE sc.id = ?1",
        )
        .bind(scene_id)
        .fetch_optional(self.db.pool())
        .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let status: Option<String> = row.try_get("status")?;
        if let Some(s) = status.as_deref() {
            if matches!(
                TaskStatus::parse(s),
                Some(TaskStatus::Queued | TaskStatus::Processing)
            ) {
                return Ok(false);
            }
        }
        sqlx::query("UPDATE scenes SET task_id = NULL, updated_at = ?2 WHERE id = ?1")
            .bind(scene_id)
            .bind(now())
            .execute(self.db.pool())
            .await?;
        Ok(true)
    }

    /// `set_scene_task` (`scripts_store.py:225-228`) — ghi de bat ke gia tri cu.
    ///
    /// # Errors
    /// `StoreError::Sqlx` khi ghi that bai.
    pub async fn set_scene_task(&self, scene_id: &str, task_id: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE scenes SET task_id = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(scene_id)
            .bind(task_id)
            .bind(now())
            .execute(self.db.pool())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{ReplaceScenesError, SceneIn, ScriptRepo};
    use crate::{BatchRef, Db, StoreError, TaskRepo};
    use lvc_core::{LogoMode, Mode, Model, Ratio, Scene};

    fn scene_in(id: Option<&str>, prompt: &str) -> SceneIn {
        SceneIn {
            id: id.map(str::to_string),
            prompt: prompt.to_string(),
            ..SceneIn::default()
        }
    }

    fn fake_scene(prompt: &str) -> Scene {
        Scene {
            prompt: prompt.to_string(),
            ratio: Ratio::R9x16,
            duration: 10,
            model: Model::Seedance20,
            mode: Mode::Fast,
            logo: LogoMode::Off,
            reference_images: Vec::new(),
            account: String::new(),
        }
    }

    #[tokio::test]
    async fn create_get_update_delete_round_trip() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);

        let created = repo.create("Demo", "").await.unwrap();
        assert_eq!(created.name, "Demo");
        assert_eq!(created.dir, "");
        assert!(created.id.starts_with("script_"));
        assert_eq!(created.id.len(), "script_".len() + 12);

        let fetched = repo.get(&created.id).await.unwrap().unwrap();
        assert_eq!(fetched, created);

        let updated = repo
            .update(&created.id, Some("Demo 2"), Some("D:/out"))
            .await
            .unwrap();
        assert_eq!(updated.name, "Demo 2");
        assert_eq!(updated.dir, "D:/out");
        assert!(updated.updated_at >= created.updated_at);

        // update chi name -> dir giu nguyen.
        let updated2 = repo
            .update(&created.id, Some("Demo 3"), None)
            .await
            .unwrap();
        assert_eq!(updated2.dir, "D:/out");

        let deleted = repo.delete(&created.id).await.unwrap();
        assert!(deleted.is_empty(), "chua co canh nao -> khong task_id nao");
        assert!(repo.get(&created.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn ten_rong_dung_ten_mac_dinh_tang_dan() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let a = repo.create("  ", "").await.unwrap();
        assert_eq!(a.name, "Kịch bản 1");
        let b = repo.create("", "").await.unwrap();
        assert_eq!(b.name, "Kịch bản 2");
    }

    #[tokio::test]
    async fn list_with_counts_gop_dung_theo_trang_thai_task() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let tasks = TaskRepo::new(&db);

        let s = repo.create("Demo", "").await.unwrap();
        repo.replace_scenes(
            &s.id,
            vec![
                scene_in(None, "canh 1"),
                scene_in(None, "canh 2"),
                scene_in(None, "canh 3"),
                scene_in(None, "canh 4"),
            ],
        )
        .await
        .unwrap();
        let scenes = repo.list_scenes(&s.id).await.unwrap();

        // canh 1: completed, canh 2: failed, canh 3: queued (running), canh 4: chua chay.
        let t1 = tasks
            .create("t1", &fake_scene("canh 1"), None)
            .await
            .unwrap();
        tasks
            .set_status("t1", lvc_core::TaskStatus::Completed, None, None)
            .await
            .unwrap();
        let t2 = tasks
            .create("t2", &fake_scene("canh 2"), None)
            .await
            .unwrap();
        tasks
            .set_status("t2", lvc_core::TaskStatus::Failed, None, None)
            .await
            .unwrap();
        let t3 = tasks
            .create("t3", &fake_scene("canh 3"), None)
            .await
            .unwrap();
        let _ = &t1;
        let _ = &t2;
        let _ = &t3;

        repo.set_scene_task(&scenes[0].id, "t1").await.unwrap();
        repo.set_scene_task(&scenes[1].id, "t2").await.unwrap();
        repo.set_scene_task(&scenes[2].id, "t3").await.unwrap();

        let metas = repo.list_with_counts().await.unwrap();
        assert_eq!(metas.len(), 1);
        let m = &metas[0];
        assert_eq!(m.total, 4);
        assert_eq!(m.done, 1);
        assert_eq!(m.failed, 1);
        assert_eq!(m.running, 1);

        let single = repo.meta_for(&s.id).await.unwrap().unwrap();
        assert_eq!(single, *m);
        assert!(repo.meta_for("khong-ton-tai").await.unwrap().is_none());
    }

    /// Fix-round 1 (C1 — Critical): `\r\n`/`\r`/`\n` that trong prompt phai bi gop thanh MOT dau
    /// cach NGAY LUC GHI — `scenes.prompt` khong bao gio duoc luu con xuong dong that (xem doc
    /// `normalize_prompt_line`, `lvc_core::scene_txt`).
    #[tokio::test]
    async fn replace_scenes_gop_xuong_dong_that_trong_prompt_thanh_dau_cach() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();

        let saved = repo
            .replace_scenes(
                &s.id,
                vec![scene_in(None, "dong mot\ndong hai\r\ndong ba\rdong bon")],
            )
            .await
            .unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].prompt, "dong mot dong hai dong ba dong bon");

        // Cap nhat lai (UPDATE, khong phai INSERT) mot canh da co id cung phai chuan hoa.
        let updated = repo
            .replace_scenes(
                &s.id,
                vec![scene_in(Some(&saved[0].id), "sua lai\ncon nhieu dong")],
            )
            .await
            .unwrap();
        assert_eq!(updated[0].prompt, "sua lai con nhieu dong");
    }

    #[tokio::test]
    async fn replace_scenes_giu_id_da_co_va_danh_lai_idx() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();

        let first = repo
            .replace_scenes(&s.id, vec![scene_in(None, "a"), scene_in(None, "b")])
            .await
            .unwrap();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].idx, 1);
        assert_eq!(first[1].idx, 2);
        let id_a = first[0].id.clone();
        let id_b = first[1].id.clone();

        // Dao thu tu (b truoc a) + them 1 canh moi -> id cu giu nguyen, idx danh lai.
        let second = repo
            .replace_scenes(
                &s.id,
                vec![
                    scene_in(Some(&id_b), "b sua"),
                    scene_in(Some(&id_a), "a"),
                    scene_in(None, "c moi"),
                ],
            )
            .await
            .unwrap();
        assert_eq!(second.len(), 3);
        assert_eq!(second[0].id, id_b);
        assert_eq!(second[0].idx, 1);
        assert_eq!(second[0].prompt, "b sua");
        assert_eq!(second[1].id, id_a);
        assert_eq!(second[1].idx, 2);
        assert_eq!(second[2].idx, 3);
        assert_ne!(second[2].id, id_a);
        assert_ne!(second[2].id, id_b);
    }

    #[tokio::test]
    async fn replace_scenes_bo_di_thi_xoa_dong_cu() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();
        repo.replace_scenes(&s.id, vec![scene_in(None, "a"), scene_in(None, "b")])
            .await
            .unwrap();

        let kept = repo
            .replace_scenes(&s.id, vec![scene_in(None, "chi con canh nay")])
            .await
            .unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].prompt, "chi con canh nay");
    }

    #[tokio::test]
    async fn replace_scenes_tu_choi_khi_canh_dang_chay_bi_bo() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let tasks = TaskRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();
        let saved = repo
            .replace_scenes(&s.id, vec![scene_in(None, "a"), scene_in(None, "b")])
            .await
            .unwrap();

        let batch = BatchRef {
            batch_id: "run1".to_string(),
            batch_name: "n".to_string(),
            scene_index: 1,
        };
        tasks
            .create("t1", &fake_scene("a"), Some(&batch))
            .await
            .unwrap();
        repo.set_scene_task(&saved[0].id, "t1").await.unwrap();
        // t1 mac dinh o trang thai "queued" (TaskRepo::create) -> canh 0 dang chay.

        let err = repo
            .replace_scenes(&s.id, vec![scene_in(None, "chi con canh moi")])
            .await
            .unwrap_err();
        match err {
            ReplaceScenesError::SceneRunning(ids) => assert_eq!(ids, vec![saved[0].id.clone()]),
            ReplaceScenesError::Store(e) => panic!("{e:?}"),
        }

        // DB khong doi gi (tu choi CA LUOT) — canh cu van con nguyen 2 dong.
        let still = repo.list_scenes(&s.id).await.unwrap();
        assert_eq!(still.len(), 2);

        // Giu id dang chay trong mang moi -> hop le, chi doi thu tu, noi dung GIU NGUYEN.
        let ok = repo
            .replace_scenes(
                &s.id,
                vec![
                    scene_in(None, "them truoc"),
                    scene_in(Some(&saved[0].id), "khong duoc doi noi dung nay"),
                ],
            )
            .await
            .unwrap();
        assert_eq!(ok.len(), 2);
        let running_scene = ok.iter().find(|s| s.id == saved[0].id).unwrap();
        assert_eq!(
            running_scene.prompt, "a",
            "canh dang chay giu nguyen prompt"
        );
        assert_eq!(running_scene.idx, 2);
    }

    #[tokio::test]
    async fn clear_scene_task_khong_ton_tai_hoac_dang_chay_tra_false() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let tasks = TaskRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();
        let saved = repo
            .replace_scenes(&s.id, vec![scene_in(None, "a"), scene_in(None, "b")])
            .await
            .unwrap();

        assert!(!repo.clear_scene_task("khong-ton-tai").await.unwrap());

        tasks.create("t1", &fake_scene("a"), None).await.unwrap();
        repo.set_scene_task(&saved[0].id, "t1").await.unwrap();
        assert!(
            !repo.clear_scene_task(&saved[0].id).await.unwrap(),
            "t1 con queued -> tu choi"
        );

        tasks
            .set_status("t1", lvc_core::TaskStatus::Completed, None, None)
            .await
            .unwrap();
        assert!(repo.clear_scene_task(&saved[0].id).await.unwrap());
        let after = repo.list_scenes(&s.id).await.unwrap();
        assert_eq!(after[0].task_id, None);

        // canh chua tung co task -> van tra true (idempotent, giong Python: hang chi khong con
        // trong RUNNING, khong phan biet NULL san co).
        assert!(repo.clear_scene_task(&saved[1].id).await.unwrap());
    }

    #[tokio::test]
    async fn scene_task_statuses_bo_qua_task_da_bi_xoa() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();
        let saved = repo
            .replace_scenes(&s.id, vec![scene_in(None, "a")])
            .await
            .unwrap();
        // task_id tro toi task khong ton tai (chua tung tao) -> JOIN loai bo, khong panic.
        repo.set_scene_task(&saved[0].id, "mo-coi").await.unwrap();
        let statuses = repo.scene_task_statuses(&s.id).await.unwrap();
        assert!(statuses.is_empty());
        assert!(repo.running_scene_ids(&s.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn script_moi_tao_mac_dinh_dola_video_va_category_rong() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();
        assert_eq!(s.video_kind, "dola_video");
        assert_eq!(s.video_params, "");
        assert_eq!(s.category, "");

        let metas = repo.list_with_counts().await.unwrap();
        assert_eq!(metas[0].video_kind, "dola_video");
        assert_eq!(metas[0].category, "");
    }

    #[tokio::test]
    async fn set_video_config_ghi_dung_kind_va_params() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();

        let params = r#"{"mode":"t2v","aspect":"portrait","seed_count":1,"account":"acc1"}"#;
        let updated = repo
            .set_video_config(&s.id, "veo3_video", params)
            .await
            .unwrap();
        assert_eq!(updated.video_kind, "veo3_video");
        assert_eq!(updated.video_params, params);
        assert!(updated.updated_at >= s.updated_at);

        let fetched = repo.get(&s.id).await.unwrap().unwrap();
        assert_eq!(fetched.video_kind, "veo3_video");
        assert_eq!(fetched.video_params, params);
    }

    #[tokio::test]
    async fn set_video_config_khong_ton_tai_bao_bad_path() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let err = repo
            .set_video_config("khong-ton-tai", "veo3_video", "{}")
            .await
            .unwrap_err();
        assert!(matches!(err, StoreError::BadPath(_)), "{err:?}");
    }

    #[tokio::test]
    async fn set_category_ghi_dung_va_trim() {
        let db = Db::open_memory().await.unwrap();
        let repo = ScriptRepo::new(&db);
        let s = repo.create("Demo", "").await.unwrap();

        let updated = repo.set_category(&s.id, "  Affiliate  ").await.unwrap();
        assert_eq!(updated.category, "Affiliate");

        let fetched = repo.get(&s.id).await.unwrap().unwrap();
        assert_eq!(fetched.category, "Affiliate");
    }
}
