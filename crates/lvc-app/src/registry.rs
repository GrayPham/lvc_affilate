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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_du_4_node_p4a() {
        let kinds: Vec<String> = build_registry()
            .manifests()
            .into_iter()
            .map(|m| m.kind)
            .collect();
        assert_eq!(kinds, ["dola_video", "image_upload", "prompt", "save"]);
    }
}
