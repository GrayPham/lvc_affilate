//! `MiniMax` T2A v2 text-to-speech — `POST /v1/t2a_v2`, key qua header `Authorization: Bearer`.
//! Phản hồi LUÔN là JSON (khác `ElevenLabs` — xem `elevenlabs.rs`); audio sinh ra nằm HEX-ENCODE
//! trong `data.audio`, phải tự giải mã. Hợp đồng xác nhận qua doc chính thức (2026-10-13): body
//! `{model, text, stream:false, voice_setting:{voice_id,speed,vol,pitch},
//! audio_setting:{sample_rate,bitrate,format,channel}}`, lỗi nghiệp vụ báo qua `base_resp` (HTTP
//! status vẫn có thể là 200 dù API lỗi).

use serde_json::Value;

use crate::error::TtsError;
use crate::req::SynthReq;

/// Model mặc định (HD — chất lượng cao, đủ dùng cho voiceover affiliate).
const DEFAULT_MODEL: &str = "speech-2.8-hd";
const DEFAULT_BASE: &str = "https://api.minimax.io";

/// Body request (THUẦN — test riêng, không lộ key vì key ở header, không phải body).
#[must_use]
pub fn body(req: &SynthReq) -> Value {
    let model = if req.model.trim().is_empty() {
        DEFAULT_MODEL
    } else {
        req.model.trim()
    };
    serde_json::json!({
        "model": model,
        "text": req.text,
        "stream": false,
        "voice_setting": {
            "voice_id": req.voice_id,
            "speed": f64::from(req.speed),
            "vol": 1,
            "pitch": 0
        },
        "audio_setting": {
            "sample_rate": 32_000,
            "bitrate": 128_000,
            "format": "mp3",
            "channel": 1
        }
    })
}

/// URL `t2a_v2` cho `base`.
#[must_use]
pub fn url(base: Option<&str>) -> String {
    format!(
        "{}/v1/t2a_v2",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// Gọi `MiniMax`. Thành công trả về BYTES audio (đã hex-decode từ `data.audio`).
///
/// # Errors
/// [`TtsError`] theo mã HTTP / `base_resp.status_code` / hex không giải mã được.
pub async fn synth(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &SynthReq,
) -> Result<Vec<u8>, TtsError> {
    let resp = http
        .post(url(base))
        .bearer_auth(api_key)
        .json(&body(req))
        .send()
        .await
        .map_err(|e| TtsError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    let text = resp
        .text()
        .await
        .map_err(|e| TtsError::Network(e.without_url().to_string()))?;
    crate::client::check_status(status, &text)?;
    parse(&text)
}

/// Kiểm `base_resp.status_code` (0 = thành công — `MiniMax` báo lỗi nghiệp vụ dù HTTP 200), rồi
/// hex-decode `data.audio`.
fn parse(raw: &str) -> Result<Vec<u8>, TtsError> {
    let v: Value = serde_json::from_str(raw).map_err(|e| TtsError::Decode(e.to_string()))?;

    let code = v
        .get("base_resp")
        .and_then(|b| b.get("status_code"))
        .and_then(Value::as_i64)
        .unwrap_or(-1);
    if code != 0 {
        let msg = v
            .get("base_resp")
            .and_then(|b| b.get("status_msg"))
            .and_then(Value::as_str)
            .unwrap_or("loi khong ro");
        return Err(TtsError::Http {
            status: u16::try_from(code).unwrap_or(0),
            detail: msg.chars().take(300).collect(),
        });
    }

    let hex_audio = v
        .get("data")
        .and_then(|d| d.get("audio"))
        .and_then(Value::as_str)
        .ok_or_else(|| TtsError::Decode("thieu truong data.audio".to_string()))?;
    hex::decode(hex_audio).map_err(|e| TtsError::Decode(format!("hex khong hop le: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Provider;

    fn req() -> SynthReq {
        SynthReq {
            provider: Provider::Minimax,
            text: "xin chao".into(),
            voice_id: "English_expressive_narrator".into(),
            model: String::new(),
            speed: 1.0,
        }
    }

    #[test]
    fn body_dung_model_mac_dinh_khi_rong() {
        let b = body(&req());
        assert_eq!(b["model"], DEFAULT_MODEL);
        assert_eq!(b["text"], "xin chao");
        assert_eq!(
            b["voice_setting"]["voice_id"],
            "English_expressive_narrator"
        );
        assert_eq!(b["audio_setting"]["format"], "mp3");
    }

    #[test]
    fn parse_thanh_cong_giai_ma_hex() {
        let raw = r#"{"data":{"audio":"68656c6c6f","status":2},"base_resp":{"status_code":0,"status_msg":"success"}}"#;
        assert_eq!(parse(raw).unwrap(), b"hello".to_vec());
    }

    #[test]
    fn parse_loi_nghiep_vu_du_http_200() {
        let raw = r#"{"base_resp":{"status_code":1004,"status_msg":"invalid api key"}}"#;
        match parse(raw).unwrap_err() {
            TtsError::Http { status, detail } => {
                assert_eq!(status, 1004);
                assert_eq!(detail, "invalid api key");
            }
            other => panic!("mong Http, gap {other:?}"),
        }
    }

    #[test]
    fn parse_hex_sai_bao_decode() {
        let raw = r#"{"data":{"audio":"zz"},"base_resp":{"status_code":0}}"#;
        assert!(matches!(parse(raw).unwrap_err(), TtsError::Decode(_)));
    }
}
