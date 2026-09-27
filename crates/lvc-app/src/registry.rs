//! Composition root cho node (ADR-R014): MOI node module đăng ký ĐÚNG MỘT DÒNG ở đây. Thêm node
//! mới = thêm crate `crates/nodes/<ten>` + dependency trong `Cargo.toml` + một dòng `.with(..)`.
//! Không file nào khác (engine, lõi, UI chung) phải sửa.

use lvc_node_api::Registry;

/// Registry mọi node đã cài trong bản build này.
#[must_use]
pub fn build_registry() -> Registry {
    Registry::new()
        .with(node_prompt::node())
        .with(node_image_upload::node())
        .with(node_save::node())
        .with(node_dola_video::node())
        .with(node_concat::node())
        .with(node_trim::node())
        .with(node_frame_capture::node())
        .with(node_load_video::node())
        .with(node_audio_extract::node())
        .with(node_audio_split::node())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_du_cac_node() {
        let kinds: Vec<String> = build_registry()
            .manifests()
            .into_iter()
            .map(|m| m.kind)
            .collect();
        assert_eq!(
            kinds,
            [
                "audio_extract",
                "audio_split",
                "concat",
                "dola_video",
                "frame_capture",
                "image_upload",
                "load_video",
                "prompt",
                "save",
                "trim"
            ]
        );
    }
}
