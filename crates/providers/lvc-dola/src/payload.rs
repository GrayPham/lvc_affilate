//! Body `chat/completion` của app Android — port `dola_app.py:478-615` (video/ảnh qua action bar
//! "Tạo bằng AI", tin chat thường cho chế độ Agent/Pro) và khối đính ảnh `10052` (`:468-475`,
//! `:805-815`).
//!
//! Mọi `uuid`/thời điểm lấy từ [`Ids`] để test tất định (so với vector sinh từ bản Python).
//! Server parse JSON nên chỉ cần TƯƠNG ĐƯƠNG ngữ nghĩa với Python; thứ tự khoá vẫn giữ nguyên nhờ
//! `serde_json/preserve_order` (bật ở `lvc-core`).

use lvc_core::{Model, Ratio};
use serde_json::{Map, Value, json};

/// action bar "Tạo bằng AI" (`dola_app.py:130`).
pub const ITEM_ID: &str = "101851453440017";
/// `chat_ability.ability_type` cho lệnh action bar (`:131`).
pub const ABILITY_INSTRUCTION: u32 = 2025;
/// `instruction_type` 28 = coco instruction (`:132`).
pub const INSTRUCTION_TYPE: u32 = 28;
/// Bot Dola mặc định của app (`:283`).
pub const BOT_ID: &str = "7241547611541340167";
/// `model_item_key` chế độ プロ web (`:546`).
pub const PRO_MODEL_ITEM_KEY: &str = "4";

const VIDEO_TEMPLATE: &str = include_str!("templates/video.json");
const IMAGE_TEMPLATE: &str = include_str!("templates/image.json");

/// Nguồn `uuid v4` + thời điểm (ms) cho một lần dựng body.
pub trait Ids {
    fn uuid(&mut self) -> String;
    fn now_ms(&mut self) -> u64;
}

/// Nguồn thật: `uuid::Uuid::new_v4` + đồng hồ hệ thống.
#[derive(Debug, Default, Clone, Copy)]
pub struct RealIds;

impl Ids for RealIds {
    fn uuid(&mut self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn now_ms(&mut self) -> u64 {
        let d = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
    }
}

/// Phần định danh của phiên cần cho body (không chứa bí mật).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatIdentity {
    pub bot_id: String,
    pub uid: String,
    pub language: String,
}

/// Vị trí tin nhắn trong một cuộc chat đã có (trả lời tiếp) — rỗng = chat mới.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatRef {
    pub conversation_id: String,
    pub local_conv_id: String,
    pub last_message_index: u64,
    pub section_id: String,
}

/// Model video → `(option id, display_text, selected_display_text, extra)` (`dola_app.py:138-143`).
fn video_model(m: Model) -> (u32, &'static str, &'static str, Value) {
    match m {
        Model::Seedance25 => (
            7,
            "Dreamina Seedance 2.5",
            "2.5",
            json!({"model": "seedance_v2.5", "subs_only_tag": "enhanced", "sub_display": "Best • Uses 5x credits", "i2v_can_choose_ratio": "1"}),
        ),
        Model::Seedance20 => (
            4,
            "Dreamina Seedance 2.0 Nhanh",
            "2.0",
            json!({"model": "Seedance_2.0_Pro", "sub_display": "Fast, high-quality results"}),
        ),
        Model::Seedance10 => (
            1,
            "Dreamina Seedance 1.0",
            "1.0",
            json!({"model": "ic_mini", "sub_display": "For simple videos"}),
        ),
    }
}

/// Tỉ lệ video → `(id, display, message_text, extra)` (`:144-151`).
fn video_ratio(r: Ratio) -> (u32, &'static str, &'static str, Value) {
    let (id, s, w, h) = match r {
        Ratio::R3x4 => (1, "3:4", "3", "4"),
        Ratio::R4x3 => (2, "4:3", "4", "3"),
        Ratio::R9x16 => (3, "9:16", "9", "16"),
        Ratio::R16x9 => (4, "16:9", "16", "9"),
        Ratio::R1x1 => (5, "1:1", "1", "1"),
    };
    (
        id,
        s,
        s,
        json!({"aspect_ratio_height": h, "aspect_ratio_width": w}),
    )
}

/// Thời lượng → `(id, display)`; giá trị lạ → 10s như Python (`:152`, `:510`).
fn video_duration(d: u16) -> (u32, &'static str) {
    match d {
        5 => (1, "5s"),
        15 => (3, "15s"),
        30 => (4, "30s"),
        _ => (2, "10s"),
    }
}

/// Model ảnh Seedream (`:703-706`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ImageModel {
    #[serde(rename = "seedream_5.0_pro")]
    Seedream50Pro,
    #[default]
    #[serde(rename = "seedream_4.5")]
    Seedream45,
}

impl ImageModel {
    fn option(self) -> (u32, &'static str, &'static str, &'static str) {
        match self {
            Self::Seedream50Pro => (
                5,
                "Seedream 5.0 Pro",
                "5.0 Pro",
                "Professional use • 4x credit usage",
            ),
            Self::Seedream45 => (3, "Seedream 4.5", "4.5", "Reliable quality for daily use"),
        }
    }
}

/// Tỉ lệ ảnh (`:707-708`) — nhiều hơn video (2:3, 3:2, original).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ImageRatio {
    #[serde(rename = "original")]
    Original,
    #[serde(rename = "9:16")]
    R9x16,
    #[serde(rename = "2:3")]
    R2x3,
    #[serde(rename = "3:4")]
    R3x4,
    #[default]
    #[serde(rename = "1:1")]
    R1x1,
    #[serde(rename = "4:3")]
    R4x3,
    #[serde(rename = "3:2")]
    R3x2,
    #[serde(rename = "16:9")]
    R16x9,
}

impl ImageRatio {
    fn option(self) -> (u32, &'static str, &'static str) {
        match self {
            Self::Original => (0, "Original", "original ratio"),
            Self::R9x16 => (1, "9:16", "9:16"),
            Self::R2x3 => (6, "2:3", "2:3"),
            Self::R3x4 => (2, "3:4", "3:4"),
            Self::R1x1 => (3, "1:1", "1:1"),
            Self::R4x3 => (4, "4:3", "4:3"),
            Self::R3x2 => (7, "3:2", "3:2"),
            Self::R16x9 => (5, "16:9", "16:9"),
        }
    }
}

fn compact(v: &Value) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// Chuỗi hoặc object JSON → object (template lồng dạng chuỗi JSON).
fn as_obj(v: &Value) -> Option<Value> {
    match v {
        Value::String(s) => serde_json::from_str(s).ok(),
        Value::Object(_) => Some(v.clone()),
        _ => None,
    }
}

/// Thay `option_list` của từng selector có `label` theo `pick(label, base)`; `base` = option đầu
/// tiên đã bỏ `extra`. `pick` trả `false` = bỏ qua selector đó (`_selectors`, `:479-503`).
fn rewrite_selectors(
    selectors: &mut [Value],
    pick: &mut dyn FnMut(&str, &mut Map<String, Value>) -> bool,
) {
    for s in selectors {
        let Some(mut t) = s.get("template").and_then(as_obj) else {
            continue;
        };
        let Some(label) = t.get("label").and_then(Value::as_str).map(str::to_string) else {
            continue;
        };
        if label.is_empty() {
            continue;
        }
        let Some(mut base) = t
            .get("option_list")
            .and_then(|l| l.get(0))
            .and_then(Value::as_object)
            .cloned()
        else {
            continue;
        };
        base.remove("extra");
        if !pick(&label, &mut base) {
            continue;
        }
        t["option_list"] = Value::Array(vec![Value::Object(base)]);
        s["template"] = Value::String(compact(&t));
    }
}

/// Áp `rewrite` cho mọi item `instruction_item_type == 8` của `conf` + đặt nội dung ô nhập, rồi
/// làm lại cho từng `menu_conf.item_list[].instruction_conf` (`:514-528`, `:718-744`).
fn apply_instruction(instr: &mut Value, prompt: &str, rewrite: &mut dyn FnMut(&mut [Value])) {
    fn apply(conf: &mut Value, prompt: &str, rewrite: &mut dyn FnMut(&mut [Value])) {
        if let Some(items) = conf
            .get_mut("instruction_item_list")
            .and_then(Value::as_array_mut)
        {
            for item in items {
                if item.get("instruction_item_type").and_then(Value::as_u64) != Some(8) {
                    continue;
                }
                let Some(mut tpl) = item.get("template").and_then(as_obj) else {
                    continue;
                };
                if let Some(sel) = tpl.get_mut("selector_list").and_then(Value::as_array_mut) {
                    rewrite(sel);
                }
                item["template"] = Value::String(compact(&tpl));
            }
        }
        if let Some(ibc) = conf.get_mut("input_box_content") {
            ibc["user_input_content"] = Value::String(prompt.to_string());
        }
    }
    apply(instr, prompt, rewrite);
    if let Some(list) = instr
        .get_mut("menu_conf")
        .and_then(|m| m.get_mut("item_list"))
        .and_then(Value::as_array_mut)
    {
        for it in list {
            if let Some(conf) = it.get_mut("instruction_conf") {
                if conf.is_object() && !conf.as_object().is_some_and(Map::is_empty) {
                    apply(conf, prompt, rewrite);
                }
            }
        }
    }
}

/// Khung body chung (`base_payload`, `:575-614`).
#[allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "mot object JSON lon chep nguyen van tu Python - tach nho lam kho doi chieu"
)]
fn base_payload(
    who: &ChatIdentity,
    text: &str,
    chat: &ChatRef,
    enter_method: &str,
    content_source: &str,
    media_blocks: &[Value],
    ids: &mut dyn Ids,
) -> Value {
    let now_ms = ids.now_ms();
    let mut blocks: Vec<Value> = media_blocks.to_vec();
    blocks.push(json!({
        "append_fields": [], "block_id": ids.uuid(), "block_type": 10000,
        "content": {"text_block": {"text": text, "text_block_scene": 0}},
        "is_finish": false, "meta_info": [], "parent_id": ""
    }));
    let local_conv = if chat.local_conv_id.is_empty() {
        format!("main_{}_{}-local", ids.uuid(), who.bot_id)
    } else {
        chat.local_conv_id.clone()
    };
    let mut ext = json!({
        "answer_with_suggest": "1", "commerce_credit_config_enable": "0", "content_source": content_source,
        "create_time_ms": now_ms.to_string(), "delay_request_interval_ms": "", "enter_method_trace": enter_method,
        "gen_music_create_type": if media_blocks.is_empty() { "ai" } else { "" },
        "is_app_background": "0", "is_audio": "false", "is_douyin_installed": "0",
        "is_luna_installed": "0", "media_player_business_scene": "", "media_search_type": "0", "need_deep_think": "0",
        "previous_page_trace": if enter_method.is_empty() { "" } else { "chat" },
        "record_status": "1", "search_engine_type": "1", "send_message_scene": "keyboard",
        "sub_conv_firstmet_type": "1", "sub_conv_source_message": "[]", "system_language": who.language, "tts": "1",
        "voice_mix_input": "0", "wiki": "1", "resolution_name": "1080p"
    });
    if !media_blocks.is_empty() {
        ext["vlm_da"] = Value::String(format!("{{\"picture_num\": {}}}", media_blocks.len()));
    }
    let local_message_id = ids.uuid();
    let unique_key = format!("{}_{}", who.uid, ids.uuid());
    json!({
        "client_meta": {"biz_type": "", "bot_id": who.bot_id, "conversation_id": chat.conversation_id, "history_message_list": [],
                        "last_message_index": chat.last_message_index, "last_section_id": chat.section_id,
                        "local_conversation_id": local_conv,
                        "local_onboarding_message_id": "", "local_permissions": [], "scene_type": 0, "section_id": ""},
        "ext": ext,
        "messages": [{"content_block": blocks, "local_message_id": local_message_id,
                      "message_specific_ext": {"before_content_type": 1, "send_message_scene": "keyboard"}, "message_status": 0}],
        "option": {"ab_launch_only": false, "agent_mode": 2,
                   "aggregate_params": {"mode_id": "", "model_item_key": "0", "provider_id": "", "reasoning_effort": ""},
                   "answer_with_suggest": true, "append_system_prompts": [], "banner_send_ext": {},
                   "chat_entry_source": {"from_origin_message_id": "", "is_chat_flow_collapsed": false, "is_first_open_in_source_page": false, "source_type": 0},
                   "click_clear_context": false, "collect_id": "", "connector_info_list": [],
                   "conversation_init_ext": if chat.conversation_id.is_empty() { json!({"mode_id": "", "model_item_key": "0"}) } else { json!({}) },
                   "conversation_init_option": {"independent_sites": [], "need_ack_conversation": true},
                   "create_time_ms": now_ms, "disable_sse_cache": false, "dora_vision_info": "", "edit_query_id": [],
                   "from_suggest": false, "history_messages": [], "is_audio": false, "is_from_click_option": false,
                   "is_from_click_softlink": false, "is_regen": false, "is_replace": false, "lark_mention": false,
                   "message_from": 0, "moa_model_name": "", "model_config": {"model_extra_params": {}, "model_item_key": "0"},
                   "need_create_conversation": false, "need_deep_think": 0, "no_replace_for_regen": false,
                   "permission_interact": 0, "prompt_modification": "", "regen_instruction": "", "regen_query_id": [],
                   "related_deleted_message_ids": {}, "resend_for_regen": false, "select_text_action": "",
                   "send_message_scene": "keyboard", "shared_app_name": "", "sse_recv_event_options": {"support_chunk_delta": true},
                   "start_seq": 0, "unique_key": unique_key, "user_ug_source_info": {}},
        "user_context": []
    })
}

/// Gắn lệnh action bar vào body (`:533-538`, `:748-753`).
fn attach_instruction(payload: &mut Value, instr: &Value, image: bool) {
    let instr_s = compact(instr);
    let ext = &mut payload["ext"];
    ext["action_bar_instruction"] = Value::String(instr_s.clone());
    ext["action_bar_instruction_type"] = Value::String(INSTRUCTION_TYPE.to_string());
    ext["action_bar_item_id"] = Value::String(ITEM_ID.to_string());
    ext["action_bar_key"] = Value::String("coco_instruction".to_string());
    ext["original_content_source"] = Value::String("original".to_string());
    if image {
        ext["is_image_related"] = Value::String("1".to_string());
    }
    ext["is_manual_input"] = Value::String("true".to_string());
    let param = json!({"key": "coco_instruction", "instruction_type": INSTRUCTION_TYPE, "instruction": instr_s});
    payload["chat_ability"] =
        json!({"ability_type": ABILITY_INSTRUCTION, "ability_param": compact(&param)});
}

/// Body tạo VIDEO qua action bar (chế độ Nhanh/API) — `build_video_payload` (`:506-539`).
#[must_use]
#[allow(
    clippy::too_many_arguments,
    reason = "tham so 1-1 voi build_video_payload cua Python (who/prompt/model/ratio/duration/chat/media/ids)"
)]
pub fn video_payload(
    who: &ChatIdentity,
    prompt: &str,
    model: Model,
    ratio: Ratio,
    duration: u16,
    chat: &ChatRef,
    media_blocks: &[Value],
    ids: &mut dyn Ids,
) -> Value {
    let (mid, mdisp, msel, mextra) = video_model(model);
    let (rid, rdisp, rmsg, rextra) = video_ratio(ratio);
    let (did, ddisp) = video_duration(duration);
    let mut instr: Value = serde_json::from_str(VIDEO_TEMPLATE).unwrap_or(Value::Null);
    let mut suffix = String::new();
    apply_instruction(&mut instr, prompt, &mut |sel: &mut [Value]| {
        let mut s = String::new();
        rewrite_selectors(sel, &mut |label, base| {
            match label {
                "Model" => {
                    base.insert("id".into(), json!(mid));
                    base.insert("display_text".into(), json!(mdisp));
                    base.insert("selected_display_text".into(), json!(msel));
                    base.insert("message_text".into(), json!(""));
                    base.insert("extra".into(), mextra.clone());
                }
                "Ratio" => {
                    base.insert("id".into(), json!(rid));
                    base.insert("display_text".into(), json!(rdisp));
                    base.insert("message_text".into(), json!(rmsg));
                    base.insert("extra".into(), rextra.clone());
                    s = rmsg.to_string();
                }
                "Duration" => {
                    base.insert("id".into(), json!(did));
                    base.insert("display_text".into(), json!(ddisp));
                    base.insert("message_text".into(), json!(""));
                    base.insert("extra".into(), json!({"duration": ddisp}));
                }
                _ => return false,
            }
            true
        });
        if !s.is_empty() {
            suffix = s;
        }
    });
    let text = if suffix.is_empty() {
        prompt.to_string()
    } else {
        format!("{prompt}, {suffix}.")
    };
    let mut payload = base_payload(
        who,
        &text,
        chat,
        "chat_action_bar",
        "upload",
        media_blocks,
        ids,
    );
    attach_instruction(&mut payload, &instr, false);
    payload
}

/// Body tạo ẢNH Seedream qua action bar — `build_image_payload` (`:712-754`).
#[must_use]
pub fn image_payload(
    who: &ChatIdentity,
    prompt: &str,
    model: ImageModel,
    ratio: ImageRatio,
    chat: &ChatRef,
    media_blocks: &[Value],
    ids: &mut dyn Ids,
) -> Value {
    let (mid, mdisp, msel, msub) = model.option();
    let (rid, rdisp, rmsg) = ratio.option();
    let mut instr: Value = serde_json::from_str(IMAGE_TEMPLATE).unwrap_or(Value::Null);
    apply_instruction(&mut instr, prompt, &mut |sel: &mut [Value]| {
        rewrite_selectors(sel, &mut |label, base| {
            match label {
                "Model" => {
                    base.insert("id".into(), json!(mid));
                    base.insert("display_text".into(), json!(mdisp));
                    base.insert("message_text".into(), json!(""));
                    base.insert("selected_display_text".into(), json!(msel));
                    base.insert("extra".into(), json!({"sub_display": msub}));
                }
                "Ratio" => {
                    base.insert("id".into(), json!(rid));
                    base.insert("display_text".into(), json!(rdisp));
                    base.insert("message_text".into(), json!(rmsg));
                }
                _ => return false,
            }
            true
        });
    });
    let text = if rid == 0 {
        prompt.to_string()
    } else {
        format!("{prompt}, {rmsg}.")
    };
    let mut payload = base_payload(
        who,
        &text,
        chat,
        "chat_action_bar",
        "upload",
        media_blocks,
        ids,
    );
    attach_instruction(&mut payload, &instr, true);
    payload
}

/// Tin chat THƯỜNG (Agent) — `pro = true`: tham số chế độ プロ web (`build_text_payload`, `:549-572`).
#[must_use]
pub fn text_payload(
    who: &ChatIdentity,
    text: &str,
    chat: &ChatRef,
    pro: bool,
    media_blocks: &[Value],
    ids: &mut dyn Ids,
) -> Value {
    let mut payload = base_payload(who, text, chat, "", "", media_blocks, ids);
    if pro {
        let agent_task_param = json!({"runtime_type": 1, "folder_name": "", "device_name": "", "workspace": "", "local_app_id": "", "local_device_id": ""});
        let local_msg = payload["messages"][0]["local_message_id"].clone();
        let gtp = json!({"action": 0, "thread_local_message_id": [local_msg],
               "agent_task_param_change": {"runtime_changed": false, "device_changed": false, "sandbox_auth_type_changed": false},
               "need_modify_conversation": false, "runtime_type": 1, "agent_task_param": agent_task_param});
        let opt = &mut payload["option"];
        opt["agent_mode"] = json!(1);
        opt["need_deep_think"] = json!(4);
        opt["aggregate_params"]["model_item_key"] = json!(PRO_MODEL_ITEM_KEY);
        opt["aggregate_params"]["agent_mode"] = json!("1");
        opt["model_config"]["model_item_key"] = json!(PRO_MODEL_ITEM_KEY);
        opt["general_task_param"] = gtp.clone();
        if chat.conversation_id.is_empty() {
            opt["conversation_init_ext"] = json!({
                "agent_task_param": compact(&agent_task_param),
                "model_item_key": PRO_MODEL_ITEM_KEY,
            });
        }
        let ext = &mut payload["ext"];
        ext["agent_mode"] = json!("1");
        ext["use_deep_think"] = json!("4");
        ext["need_deep_think"] = json!("4");
        ext["general_task_param"] = Value::String(compact(&gtp));
    }
    payload
}

/// Ảnh đã upload (xem `upload`) — thông tin gắn vào khối 10052.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedImage {
    pub uri: String,
    pub md5: String,
    pub width: u32,
    pub height: u32,
    pub resource_id: String,
}

/// Khối 10052 đính ảnh ĐÃ UPLOAD, đứng TRƯỚC khối chữ (`attachment_block`, `:468-475`).
#[must_use]
pub fn attachment_block(info: &UploadedImage, ids: &mut dyn Ids) -> Value {
    json!({"append_fields": [], "block_id": ids.uuid(), "block_type": 10052,
        "content": {"attachment_block": {"attachments": [{
            "identifier": ids.uuid(),
            "image": {"image_ori": {"height": info.height, "width": info.width}, "md5": info.md5, "uri": info.uri},
            "parse_state": 1, "progress": 100, "review_state": 1, "type": 1, "upload_status": 1}]}},
        "is_finish": false, "meta_info": [], "parent_id": ""})
}

/// Khối 10052 đính ảnh DO DOLA TẠO theo `uri` (không upload lại → né lọc chân dung 肖像保護) —
/// `asset_attachment_block` (`:805-815`).
#[must_use]
pub fn asset_attachment_block(uri: &str, name: &str, ids: &mut dyn Ids) -> Value {
    let last = uri.rsplit('/').next().unwrap_or("");
    let fmt = if last.contains('.') {
        uri.rsplit('.').next().unwrap_or("jpeg").to_lowercase()
    } else {
        "jpeg".to_string()
    };
    let block_id = ids.uuid();
    let identifier = ids.uuid();
    json!({"append_fields": [], "block_id": block_id, "block_type": 10052,
        "content": {"attachment_block": {"attachments": [{
            "type": 1, "identifier": identifier,
            "image": {"name": if name.is_empty() { "reference" } else { name }, "uri": uri,
                      "image_ori": {"url": "", "width": 0, "height": 0, "format": fmt, "url_formats": {}}},
            "parse_state": 1, "review_state": 1, "upload_status": 1, "progress": 100, "src": ""}]},
            "pc_event_block": ""},
        "is_finish": false, "meta_info": [], "parent_id": ""})
}

/// Body poll chat `im/chain/single` cmd 3100 (`poll_video`, `:1028-1032`).
#[must_use]
pub fn poll_chain_body(conversation_id: &str) -> Value {
    json!({"cmd": 3100, "sequence_id": "poll",
        "uplink_body": {"pull_singe_chain_uplink_body": {"conversation_id": conversation_id, "anchor_index": 9_007_199_254_740_991_u64,
            "conversation_type": 3, "direction": 1, "limit": 5, "ext": {}, "filter": {"index_list": []}}}})
}

/// Body `im/conversation/batch_get` cmd 1111 (`conv_video_model`, `:1014-1017`).
#[must_use]
pub fn batch_get_body(conversation_id: &str) -> Value {
    json!({"cmd": 1111, "sequence_id": "batchget",
        "uplink_body": {"batch_get_conv_info_uplink_body": {
            "conversation_id": [conversation_id], "option": {"recent_message_count_per_conv": 1}, "ext": {}}}})
}
