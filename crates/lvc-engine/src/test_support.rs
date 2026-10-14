//! Tien ich CHI cho test trong crate (P4a): registry voi node module that (dev-dependency).

/// Registry prompt / `image_upload` / save / `dola_video` — giong `lvc-app::build_registry`.
pub(crate) fn test_registry() -> lvc_node_api::Registry {
    lvc_node_api::Registry::new()
        .with(node_prompt::node())
        .with(node_image_upload::node())
        .with(node_save::node())
        .with(node_dola_video::node())
}

/// [`test_registry`] + `veo3_video`/`google_video`/`seedance_video` — rieng cho test
/// `scripts::run_script` chon provider khac `dola_video` (xem `scripts.rs` test module). KHONG
/// dung lam registry mac dinh cho moi test khac (danh sach node co chu dich, giu [`test_registry`]
/// nguyen ban de cac test khong lien quan khong phai keo them node khong can).
pub(crate) fn test_registry_all_video() -> lvc_node_api::Registry {
    test_registry()
        .with(node_veo3_video::node())
        .with(node_google_video::node())
        .with(node_seedance_video::node())
}
