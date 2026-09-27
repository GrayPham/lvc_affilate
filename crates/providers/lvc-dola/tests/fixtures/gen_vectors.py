"""Sinh golden test vector cho port Rust cua lop ky Dola (X-Gorgon/Ladon/Argus).
Chay bang venv Python cua lvcseedance. Fix time/random de deterministic.
Xuat JSON ra stdout."""
import sys, os, json, hashlib, base64

BASE = r"A:/ToolLVC/lvcseedance/src/api/upscaledola"
sys.path.insert(0, os.path.join(BASE, "x_signatures"))

import time as _time
import lib.XGorgon as XG
import lib.XLadon as XL
import lib.XArgus as XA
from lib.Sm3 import SM3
from lib.Simon import simon_enc, simon_dec, key_expansion
from lib.protobuf import ProtoBuf
from struct import unpack

FIXED_UNIX = 1700000000
FIXED_RAND = 123456789
FIXED_LADON_RB = bytes([1, 2, 3, 4])

out = {}

# --- SM3 ---
def sm3hex(b):
    return SM3().sm3_hash(b).hex()
out["sm3"] = [
    {"in_hex": b"".hex(), "out": sm3hex(b"")},
    {"in_hex": bytes(16).hex(), "out": sm3hex(bytes(16))},
    {"in_hex": b"hello world".hex(), "out": sm3hex(b"hello world")},
    {"in_hex": (b"a" * 55).hex(), "out": sm3hex(b"a" * 55)},
    {"in_hex": (b"a" * 56).hex(), "out": sm3hex(b"a" * 56)},
    {"in_hex": (b"a" * 64).hex(), "out": sm3hex(b"a" * 64)},
    {"in_hex": (b"abc" * 40).hex(), "out": sm3hex(b"abc" * 40)},
]

# --- Simon (key_expansion + enc + dec) ---
key_list = [0x0706050403020100, 0x0f0e0d0c0b0a0908,
            0x1716151413121110, 0x1f1e1d1c1b1a1918]
pt = [0x6373656420737265, 0x6f727020646e756f]
ct = simon_enc(list(pt), list(key_list))
out["simon"] = {
    "key": [str(k) for k in key_list],
    "pt": [str(x) for x in pt],
    "ct": [str(x) for x in ct],
    "key_expanded": [str(x) for x in key_expansion([key_list[0], key_list[1], key_list[2], key_list[3]] + [0]*68)],
}

# --- protobuf encoder ---
def pbhex(d):
    return ProtoBuf(d).toBuf().hex()
out["protobuf"] = [
    {"dict": {"1": 1, "2": 300, "3": "abc"}, "out": pbhex({1: 1, 2: 300, 3: "abc"})},
    {"dict": {"1": 0x40401252, "12": 0xCA9A3B00}, "out": pbhex({1: 0x40401252, 12: 0xCA9A3B00})},
    {"dict": {"5": "7177009225176253958", "13_bytes": "aabbccddeeff"},
     "out": pbhex({5: "7177009225176253958", 13: bytes.fromhex("aabbccddeeff")})},
    {"dict": {"23_nested": {"1": "SM-A127F", "3": "googleplay"}},
     "out": pbhex({23: {1: "SM-A127F", 3: "googleplay"}})},
    {"note": "varint edge 0x80", "val": 0x80, "out": pbhex({1: 0x80})},
    {"note": "varint edge 0x7f", "val": 0x7f, "out": pbhex({1: 0x7f})},
    {"note": "varint edge 0x81", "val": 0x81, "out": pbhex({1: 0x81})},
]

# --- XGorgon (fix time) ---
_orig_time = _time.time
XG.time.time = lambda: float(FIXED_UNIX)
params_a = "aid=1233&device_id=7177009225176253958&iid=7177010410163668742"
g1 = XG.XGorgon().calculate(params=params_a, headers={})
g2 = XG.XGorgon().calculate(params=params_a, headers={"x-ss-stub": "A" * 32, "cookie": "tt_webid=7177009225176253958"})
out["xgorgon"] = [
    {"params": params_a, "headers": {}, "X-Gorgon": g1["X-Gorgon"], "X-Khronos": g1["X-Khronos"]},
    {"params": params_a, "headers": {"x-ss-stub": "A" * 32, "cookie": "tt_webid=7177009225176253958"},
     "X-Gorgon": g2["X-Gorgon"], "X-Khronos": g2["X-Khronos"]},
]
XG.time.time = _orig_time

# --- XLadon (explicit random_bytes) ---
l1 = XL.ladon_encrypt(timestamp=FIXED_UNIX, license_id=1611921764, aid=1233, random_bytes=FIXED_LADON_RB)
l2 = XL.ladon_encrypt(timestamp=1674223203, license_id=1611921764, aid=1233, random_bytes=FIXED_LADON_RB)
# intermediate: md5hex + encrypt_ladon
data = f"{FIXED_UNIX}-1611921764-1233"
md5hex = XL.md5bytes(FIXED_LADON_RB + str(1233).encode())
enc = XL.encrypt_ladon(md5hex.encode(), data.encode(), len(data))
out["xladon"] = {
    "random_bytes_hex": FIXED_LADON_RB.hex(),
    "cases": [
        {"timestamp": FIXED_UNIX, "license_id": 1611921764, "aid": 1233, "out": l1},
        {"timestamp": 1674223203, "license_id": 1611921764, "aid": 1233, "out": l2},
    ],
    "intermediate": {"data": data, "md5hex": md5hex, "encrypt_ladon_hex": bytes(enc).hex()},
}

# --- XArgus (fix randint) ---
XA.randint = lambda a, b: FIXED_RAND
stub = None
params_arg = params_a
# expose bean + protobuf bytes
import lib.XArgus as _XA
# reconstruct bean identically to get_sign to dump protobuf
from urllib.parse import parse_qs
pd = parse_qs(params_arg)
bean = {
    1: 0x20200929 << 1, 2: 2, 3: FIXED_RAND, 4: str(1233),
    5: pd.get("device_id", [""])[0] or "", 6: str(1611921764),
    7: pd.get("version_name", [""])[0] or "", 8: "v04.04.05-ov-android",
    9: 134744640, 10: bytes(8), 11: 0, 12: FIXED_UNIX << 1,
    13: _XA.Argus.get_bodyhash(stub), 14: _XA.Argus.get_queryhash(params_arg),
}
bean[20] = 738
bean[23] = {1: str(pd.get("device_type", [""])[0]), 2: pd.get("os_version", [""])[0],
            3: "googleplay", 4: _XA.Argus.calculate_constant(pd.get("os_version", [""])[0])}
pb_hex = ProtoBuf(bean).toBuf().hex()
argus_out = _XA.Argus.get_sign(params=params_arg, stub=stub, timestamp=FIXED_UNIX,
                               platform=0, aid=1233, license_id=1611921764,
                               sec_device_id="", sdk_version="v04.04.05-ov-android",
                               sdk_version_int=134744640)
# AES key/iv derivation constants
sign_key = b"\xac\x1a\xda\xae\x95\xa7\xaf\x94\xa5\x11J\xb3\xb3\xa9}\xd8\x00P\xaa\n91L@R\x8c\xae\xc9RV\xc2\x8c"
out["xargus"] = {
    "params": params_arg, "stub": stub, "timestamp": FIXED_UNIX, "rand": FIXED_RAND,
    "bean_protobuf_hex": pb_hex,
    "bodyhash_hex": _XA.Argus.get_bodyhash(stub).hex(),
    "queryhash_hex": _XA.Argus.get_queryhash(params_arg).hex(),
    "aes_key_hex": hashlib.md5(sign_key[:16]).hexdigest(),
    "aes_iv_hex": hashlib.md5(sign_key[16:]).hexdigest(),
    "X-Argus": argus_out,
}

# --- full sign ---
XG.time.time = lambda: float(FIXED_UNIX)
from lib.sign import Sign
full = Sign(params=params_a, headers={"x-ss-stub": "A" * 32},
           sec_device_id="7681452747673093652", aid=1233, license_id=1611921764,
           unix=FIXED_UNIX)
XG.time.time = _orig_time
out["full_sign"] = {
    "params": params_a, "x-ss-stub": "A" * 32, "sec_device_id": "7681452747673093652",
    "unix": FIXED_UNIX,
    "result": {k: v for k, v in full.items()},
}

print(json.dumps(out, indent=2, ensure_ascii=False))
