//! Tien ich CHI cho test trong crate (P4a): registry voi 4 node module that (dev-dependency).

/// Registry prompt / `image_upload` / save / `dola_video` — giong `lvc-app::build_registry`.
pub(crate) fn test_registry() -> lvc_node_api::Registry {
    lvc_node_api::Registry::new()
        .with(node_prompt::node())
        .with(node_image_upload::node())
        .with(node_save::node())
        .with(node_dola_video::node())
}
