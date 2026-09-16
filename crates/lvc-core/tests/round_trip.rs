//! Tinh chat cot loi cua dinh dang TXT: parse(format(x)) == x.

use lvc_core::{Limits, LogoMode, Mode, Model, Ratio, Scene, format_txt, parse_txt};
use proptest::prelude::*;

fn any_ratio() -> impl Strategy<Value = Ratio> {
    prop_oneof![
        Just(Ratio::R9x16),
        Just(Ratio::R16x9),
        Just(Ratio::R1x1),
        Just(Ratio::R4x3),
        Just(Ratio::R3x4),
    ]
}

fn any_scene() -> impl Strategy<Value = Scene> {
    (
        // prompt: khong xuong dong, khong rong sau khi trim, co the chua dau gach dung.
        // CO Y khong cho '#' vao bo ky tu: prompt bat dau bang '#' khong the
        // round-trip (parse_txt coi ca dong la comment) — gioi han da duoc ghim
        // rieng o scene_txt::tests::prompt_bat_dau_bang_thang_..., proptest nay
        // chi kiem tra pham vi con lai thuc su round-trip duoc.
        "[a-zA-Z0-9 |]{1,40}",
        any_ratio(),
        prop_oneof![Just(10u16), Just(15u16), Just(30u16)],
        prop_oneof![Just(Model::Seedance20), Just(Model::Seedance25)],
        prop_oneof![Just(Mode::Fast), Just(Mode::Pro), Just(Mode::ProMax)],
        prop_oneof![
            Just(LogoMode::Off),
            Just(LogoMode::Crop),
            Just(LogoMode::Delogo)
        ],
        // "|" trong gia tri ref phai duoc escape/unescape dung khi round-trip,
        // nen mo rong bo ky tu de proptest thuc su di qua nhanh escape do.
        prop::collection::vec("[a-z0-9.|]{1,10}", 0..3),
        prop_oneof![Just(String::new()), "[a-z][a-z0-9_-]{0,10}"],
    )
        .prop_filter("prompt khong rong sau trim", |(p, ..)| !p.trim().is_empty())
        .prop_map(
            |(prompt, ratio, duration, model, mode, logo, reference_images, account)| Scene {
                prompt: prompt.trim().to_string(),
                ratio,
                duration,
                model,
                mode,
                logo,
                reference_images,
                account,
            },
        )
}

proptest! {
    #[test]
    fn parse_format_round_trip(scenes in prop::collection::vec(any_scene(), 1..6)) {
        let text = format_txt(&scenes);
        let back = parse_txt(&text, &Limits::default()).unwrap();
        prop_assert_eq!(scenes, back);
    }
}
