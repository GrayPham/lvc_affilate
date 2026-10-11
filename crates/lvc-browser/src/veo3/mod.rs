//! Lop Veo3/Flow (P5, spec `docs/superpowers/specs/2026-09-27-veo3-image-video-nodes-design.md`
//! v2, ADR-R018): dung `chromiumoxide` TRUC TIEP, KHONG qua trait `BrowserDriver` cua `dola/`
//! (trait do gan voi mo hinh submit/poll rieng cua Dola, khong hop voi captcha/harvest cua Veo3).
//!
//! Module nay TU DUNG — giong `dola/` khong phu thuoc crate provider `lvc-dola`, o day cung
//! KHONG phu thuoc crate provider `lvc-veo3`. `lvc-engine` la noi duy nhat noi 2 ben (impl trait
//! `VeoCaptchaHost`/`VeoSessionHost` cua `lvc-veo3` bang ham trong module nay).

pub mod captcha;
pub mod harvest;
