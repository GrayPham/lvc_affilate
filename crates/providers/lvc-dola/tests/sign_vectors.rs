//! Kiểm thử golden vector cho lớp ký Dola — so khớp TỪNG BYTE với bản Python.
//!
//! Vector sinh từ `tests/fixtures/gen_vectors.py` (time=1700000000, argus rand=123456789,
//! ladon `random_bytes`=01020304). Nếu một tầng lệch, dùng vector trung gian để khoanh vùng.

use base64::Engine as _;
use lvc_dola::sign::argus;
use lvc_dola::sign::gorgon::xgorgon;
use lvc_dola::sign::ladon::{encrypt_ladon, xladon};
use lvc_dola::sign::protobuf::{ProtoVal, protobuf_encode};
use lvc_dola::sign::simon::{key_expansion, simon_dec, simon_enc};
use lvc_dola::sign::sm3::sm3_hash;
use serde_json::Value;

const SDK_VERSION: &str = "v04.04.05-ov-android";
const SDK_VERSION_INT: u64 = 134_744_640;

fn fixtures() -> Value {
    let raw = include_str!("fixtures/sign_vectors.json");
    serde_json::from_str(raw).expect("fixture JSON hop le")
}

fn u64s(v: &Value) -> Vec<u64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_str().unwrap().parse::<u64>().unwrap())
        .collect()
}

#[test]
fn sm3_khop_vector() {
    let fx = fixtures();
    for case in fx["sm3"].as_array().unwrap() {
        let input = hex::decode(case["in_hex"].as_str().unwrap()).unwrap();
        let got = hex::encode(sm3_hash(&input));
        assert_eq!(got, case["out"].as_str().unwrap(), "sm3 {}", case["in_hex"]);
    }
}

#[test]
fn simon_khop_vector() {
    let fx = fixtures();
    let s = &fx["simon"];
    let key_v = u64s(&s["key"]);
    let key = [key_v[0], key_v[1], key_v[2], key_v[3]];
    let pt_v = u64s(&s["pt"]);
    let pt = [pt_v[0], pt_v[1]];
    let ct_v = u64s(&s["ct"]);

    // key_expansion
    let expanded = key_expansion(&key);
    let exp_v = u64s(&s["key_expanded"]);
    for (i, want) in exp_v.iter().enumerate() {
        assert_eq!(expanded[i], *want, "key_expanded[{i}]");
    }

    // enc
    let ct = simon_enc(pt, &key);
    assert_eq!([ct[0], ct[1]], [ct_v[0], ct_v[1]], "simon_enc");

    // dec (đối xứng)
    let back = simon_dec(ct, &key);
    assert_eq!(back, pt, "simon_dec quay lai plaintext");
}

#[test]
fn protobuf_khop_vector() {
    let fx = fixtures();
    let cases = fx["protobuf"].as_array().unwrap();

    // 0: {1:1, 2:300, 3:"abc"}
    let enc = protobuf_encode(&[
        (1, ProtoVal::Varint(1)),
        (2, ProtoVal::Varint(300)),
        (3, ProtoVal::Str(b"abc".to_vec())),
    ]);
    assert_eq!(hex::encode(enc), cases[0]["out"].as_str().unwrap());

    // 1: {1:0x40401252, 12:0xCA9A3B00}
    let enc = protobuf_encode(&[
        (1, ProtoVal::Varint(0x4040_1252)),
        (12, ProtoVal::Varint(0xCA9A_3B00)),
    ]);
    assert_eq!(hex::encode(enc), cases[1]["out"].as_str().unwrap());

    // 2: {5:"7177009225176253958", 13:bytes(aabbccddeeff)}
    let enc = protobuf_encode(&[
        (5, ProtoVal::Str(b"7177009225176253958".to_vec())),
        (13, ProtoVal::Str(hex::decode("aabbccddeeff").unwrap())),
    ]);
    assert_eq!(hex::encode(enc), cases[2]["out"].as_str().unwrap());

    // 3: {23:{1:"SM-A127F", 3:"googleplay"}}
    let enc = protobuf_encode(&[(
        23,
        ProtoVal::Nested(vec![
            (1, ProtoVal::Str(b"SM-A127F".to_vec())),
            (3, ProtoVal::Str(b"googleplay".to_vec())),
        ]),
    )]);
    assert_eq!(hex::encode(enc), cases[3]["out"].as_str().unwrap());

    // 4..6: edge varint 0x80 / 0x7f / 0x81
    for (idx, val) in [(4usize, 0x80u64), (5, 0x7f), (6, 0x81)] {
        let enc = protobuf_encode(&[(1, ProtoVal::Varint(val))]);
        assert_eq!(
            hex::encode(enc),
            cases[idx]["out"].as_str().unwrap(),
            "varint edge {val:#x}"
        );
    }
}

#[test]
fn xgorgon_khop_vector() {
    let fx = fixtures();
    let cases = fx["xgorgon"].as_array().unwrap();

    // case 0: khong stub, khong cookie
    let params = cases[0]["params"].as_str().unwrap();
    let (g, k) = xgorgon(params, None, None, 1_700_000_000);
    assert_eq!(g, cases[0]["X-Gorgon"].as_str().unwrap(), "gorgon case0");
    assert_eq!(k, cases[0]["X-Khronos"].as_str().unwrap(), "khronos case0");

    // case 1: co x-ss-stub + cookie
    let h = &cases[1]["headers"];
    let stub = h["x-ss-stub"].as_str().unwrap();
    let cookie = h["cookie"].as_str().unwrap();
    let (g, k) = xgorgon(
        cases[1]["params"].as_str().unwrap(),
        Some(stub),
        Some(cookie),
        1_700_000_000,
    );
    assert_eq!(g, cases[1]["X-Gorgon"].as_str().unwrap(), "gorgon case1");
    assert_eq!(k, cases[1]["X-Khronos"].as_str().unwrap(), "khronos case1");
}

#[test]
fn xladon_khop_vector() {
    let fx = fixtures();
    let l = &fx["xladon"];
    let random4_v = hex::decode(l["random_bytes_hex"].as_str().unwrap()).unwrap();
    let random4 = [random4_v[0], random4_v[1], random4_v[2], random4_v[3]];

    // intermediate: encrypt_ladon(md5hex, data)
    let inter = &l["intermediate"];
    let md5hex = inter["md5hex"].as_str().unwrap();
    let data = inter["data"].as_str().unwrap();
    let enc = encrypt_ladon(md5hex.as_bytes(), data.as_bytes());
    assert_eq!(
        hex::encode(enc),
        inter["encrypt_ladon_hex"].as_str().unwrap(),
        "encrypt_ladon trung gian"
    );

    // cases: xladon day du
    for case in l["cases"].as_array().unwrap() {
        let ts = case["timestamp"].as_u64().unwrap();
        let license = case["license_id"].as_u64().unwrap();
        let aid = case["aid"].as_u64().unwrap();
        let got = xladon(ts, license, aid, random4);
        assert_eq!(got, case["out"].as_str().unwrap(), "xladon ts={ts}");
    }
}

#[test]
fn xargus_khop_vector() {
    let fx = fixtures();
    let a = &fx["xargus"];
    let params = a["params"].as_str().unwrap();
    let ts = a["timestamp"].as_u64().unwrap();
    let rand_field3 = u32::try_from(a["rand"].as_u64().unwrap()).unwrap();

    // 1) bean protobuf phai khop TRUOC (neu sai thi moi thu sau sai)
    let bean = argus::build_bean(
        params,
        None,
        ts,
        1233,
        1_611_921_764,
        0,
        "",
        SDK_VERSION,
        SDK_VERSION_INT,
        rand_field3,
    );
    let pb = protobuf_encode(&bean);
    assert_eq!(
        hex::encode(&pb),
        a["bean_protobuf_hex"].as_str().unwrap(),
        "bean_protobuf"
    );

    // 2) bodyhash / queryhash
    assert_eq!(
        hex::encode(&sm3_hash(&[0u8; 16])[..6]),
        a["bodyhash_hex"].as_str().unwrap(),
        "bodyhash"
    );
    assert_eq!(
        hex::encode(&sm3_hash(params.as_bytes())[..6]),
        a["queryhash_hex"].as_str().unwrap(),
        "queryhash"
    );

    // 3) aes key/iv
    let (key, iv) = argus::aes_key_iv();
    assert_eq!(
        hex::encode(key),
        a["aes_key_hex"].as_str().unwrap(),
        "aes_key"
    );
    assert_eq!(hex::encode(iv), a["aes_iv_hex"].as_str().unwrap(), "aes_iv");

    // 4) X-Argus day du
    let got = argus::xargus(
        params,
        None,
        ts,
        1233,
        1_611_921_764,
        0,
        "",
        SDK_VERSION,
        SDK_VERSION_INT,
        rand_field3,
    );
    assert_eq!(got, a["X-Argus"].as_str().unwrap(), "X-Argus");
}

#[test]
fn full_sign_khop_vector() {
    let fx = fixtures();
    let f = &fx["full_sign"];
    let params = f["params"].as_str().unwrap();
    let stub = f["x-ss-stub"].as_str().unwrap();
    let sec_device_id = f["sec_device_id"].as_str().unwrap();
    let unix = f["unix"].as_u64().unwrap();
    let want = &f["result"];

    // X-Gorgon: Sign truyen headers chi co x-ss-stub (khong cookie).
    let (g, k) = xgorgon(params, Some(stub), None, unix);
    assert_eq!(g, want["X-Gorgon"].as_str().unwrap(), "full X-Gorgon");
    assert_eq!(k, want["X-Khronos"].as_str().unwrap(), "full X-Khronos");

    // X-Ladon: random4 khong tat dinh -> tach lai tu chinh output ky vong roi tinh lai.
    let want_ladon = want["x-ladon"].as_str().unwrap();
    let ladon_bytes = base64::engine::general_purpose::STANDARD
        .decode(want_ladon)
        .unwrap();
    let random4 = [
        ladon_bytes[0],
        ladon_bytes[1],
        ladon_bytes[2],
        ladon_bytes[3],
    ];
    let got_ladon = xladon(unix, 1_611_921_764, 1233, random4);
    assert_eq!(got_ladon, want_ladon, "full x-ladon");

    // X-Argus: stub = x-ss-stub, rand field3 = 123456789 (đã cố định trong bộ sinh), co sec_device_id.
    let got_argus = argus::xargus(
        params,
        Some(stub),
        unix,
        1233,
        1_611_921_764,
        0,
        sec_device_id,
        SDK_VERSION,
        SDK_VERSION_INT,
        123_456_789,
    );
    assert_eq!(got_argus, want["x-argus"].as_str().unwrap(), "full x-argus");
}
