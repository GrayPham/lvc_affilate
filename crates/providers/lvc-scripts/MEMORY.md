# Ghi nhớ — `lvc-scripts`

- Crate này CHỈ là hợp đồng (trait + kiểu dữ liệu), không có impl/test hành vi — impl thật nằm ở
  `crates/lvc-engine/src/scriptsink.rs` (`EngineScriptSinkHost`, dùng `lvc_store::ScriptRepo`).
- `ScriptDraft.scenes[i]` lưu NGUYÊN VĂN dòng do `node-script-gen` sinh ra (sau
  `lvc_genai::split_scenes`) — không ép ratio/duration/model/mode/logo (các cột đó để `NULL`,
  giống hành vi `SceneIn` mặc định — `Engine::run_script` tự điền từ `AppSettings` lúc CHẠY, không
  điền ở bước lưu này).
