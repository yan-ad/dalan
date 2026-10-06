use gpui_kit::{
    AssetSource,
    assets::{Assets, IconName},
};

#[test]
fn default_assets_include_native_control_icons() {
    for icon in [
        IconName::Play,
        IconName::Check,
        IconName::ChevronDown,
        IconName::Search,
    ] {
        let bytes = Assets
            .load(&icon.path())
            .unwrap()
            .expect("bundled default icon");
        assert!(std::str::from_utf8(&bytes).unwrap().contains("<svg"));
    }
}
