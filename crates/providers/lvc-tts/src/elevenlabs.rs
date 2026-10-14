//! `ElevenLabs` text-to-speech — `POST /v1/text-to-speech/{voice_id}`, key qua header `xi-api-key`.
//! Phản hồi thành công là BYTES AUDIO THÔ (không phải JSON) — khác hẳn `MiniMax` (xem `minimax.rs`).
//! Hợp đồng xác nhận qua doc chính thức (2026-10-13): body `{text, model_id, voice_settings:
//! {speed}}`, query `output_format=mp3_44100_128`.

use serde_json::Value;

use crate::error::TtsError;
use crate::req::SynthReq;

/// Model mặc định (đa ngôn ngữ — hỗ trợ tiếng Việt).
const DEFAULT_MODEL: &str = "eleven_multilingual_v2";
const DEFAULT_BASE: &str = "https://api.elevenlabs.io";

/// Body request (THUẦN — test riêng, không lộ key vì key đi ở header, không phải body).
#[must_use]
pub fn body(req: &SynthReq) -> Value {
    let model_id = if req.model.trim().is_empty() {
        DEFAULT_MODEL
    } else {
        req.model.trim()
    };
    serde_json::json!({
        "text": req.text,
        "model_id": model_id,
        "voice_settings": { "speed": f64::from(req.speed) }
    })
}

/// URL `text-to-speech/{voice_id}` cho `base` (không chứa key — key ở header `xi-api-key`).
#[must_use]
pub fn url(base: Option<&str>, voice_id: &str) -> String {
    format!(
        "{}/v1/text-to-speech/{voice_id}",
        base.unwrap_or(DEFAULT_BASE).trim_end_matches('/')
    )
}

/// Gọi `ElevenLabs`. Thành công trả về BYTES audio thô (mp3).
///
/// # Errors
/// [`TtsError`] theo mã HTTP / phản hồi.
pub async fn synth(
    http: &reqwest::Client,
    api_key: &str,
    base: Option<&str>,
    req: &SynthReq,
) -> Result<Vec<u8>, TtsError> {
    let resp = http
        .post(url(base, &req.voice_id))
        .query(&[("output_format", "mp3_44100_128")])
        .header("xi-api-key", api_key)
        .json(&body(req))
        .send()
        .await
        .map_err(|e| TtsError::Network(e.without_url().to_string()))?;
    let status = resp.status().as_u16();
    if !(200..=299).contains(&status) {
        let text = resp.text().await.unwrap_or_default();
        return Err(crate::client::error_for_status(status, &text));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| TtsError::Network(e.without_url().to_string()))?;
    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::req::Provider;

    fn req() -> SynthReq {
        SynthReq {
            provider: Provider::ElevenLabs,
            text: "xin chao".into(),
            voice_id: "21m00Tcm4TlvDq8ikWAM".into(),
            model: String::new(),
            speed: 1.0,
        }
    }

    #[test]
    fn body_dung_model_mac_dinh_khi_rong() {
        let b = body(&req());
        assert_eq!(b["text"], "xin chao");
        assert_eq!(b["model_id"], DEFAULT_MODEL);
        assert_eq!(b["voice_settings"]["speed"], 1.0);
    }

    #[test]
    fn body_giu_model_tuy_chinh() {
        let mut r = req();
        r.model = "eleven_flash_v2_5".into();
        assert_eq!(body(&r)["model_id"], "eleven_flash_v2_5");
    }

    #[test]
    fn url_ghep_dung_voice_id() {
        assert_eq!(
            url(None, "abc123"),
            "https://api.elevenlabs.io/v1/text-to-speech/abc123"
        );
        assert_eq!(
            url(Some("https://custom.example/"), "abc123"),
            "https://custom.example/v1/text-to-speech/abc123"
        );
    }
}
