# Node `script_gen` - luat

Hop dong chung: `crates/lvc-node-api` | Luat repo: `CLAUDE.md` goc (muc "Node doc lap").

## Phu thuoc duoc phep
lvc-node-api, lvc-core, `lvc-genai` (provider sinh text), `lvc-scripts` (dich vu luu kich ban).
**Cam** node khac, `lvc-engine`, `lvc-store`, `lvc-browser`, `tauri`.

## Hop dong vao/ra
- Vao: `brief` (Text, tuy chon, `.many()`) - brief/chu de; rong thi dung tham so `topic`.
- Ra: `out` (Text, `.many()`) - moi dong mot canh (dinh dang nhu node `prompt`).
- Tai khoan: `credential = { provider: "gemini", kinds: ["api_key"] }` (openai cung dung api_key;
  provider thuc chon o tham so `provider`, engine resolve theo do).

## Tham so
`topic`, `num_scenes` (>=1), `language` (mac dinh "vi"), `style`, `system` (chi dan he thong tuy
chon), `credential` (ten API key), `provider` ("gemini"|"openai"), `save_as_script` (mac dinh
false - luu ket qua thanh kich ban moi trong tab "Kịch bản" khi `run` thanh cong), `script_name`
(ten kich ban khi luu, rong = host tu dat "Kịch bản {n}").

## Luat rieng
- `preview` THUAN: kiem tham so, tra `num_scenes` cai gia cho co prompt KHONG rong (node sau dung
  de dem job).
- `run` lay `Arc<dyn lvc_genai::TextGenHost>` tu `ctx.services`, goi `generate`, tach dong bang
  `lvc_genai::split_scenes`. KHONG tu goi mang, KHONG giu API key (chi truyen TEN credential).
- Loi API la loi MOT node (khong fatal) - khong dung ca run.
- `save_as_script=true`: sau khi sinh canh thanh cong, lay `Arc<dyn lvc_scripts::ScriptSinkHost>`
  tu `ctx.services`, goi `save_script`. Day la tac vu PHU - loi luu (thieu service/ghi kho that
  bai) KHONG lam that bai ca node (outputs van tra ve day du, van chay xuong node sau), chi bao vao
  `NodeResult.failure` (vao `runs.error`).

## Kiem tra
- `cargo test -p node-script-gen` (dung FAKE host, KHONG goi API that).
- Doi manifest: `LVC_WRITE_FIXTURES=1 cargo test -p node-script-gen` roi sinh lai fixture UI
  `LVC_WRITE_FIXTURES=1 cargo test -p lvc-app commands::fixtures`.
