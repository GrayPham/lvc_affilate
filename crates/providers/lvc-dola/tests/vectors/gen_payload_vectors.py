"""Sinh vector body chat/completion tu ban Python (CHI DOC A:/ToolLVC/lvcseedance).

Chay: python crates/providers/lvc-dola/tests/vectors/gen_payload_vectors.py
uuid4 -> "00000000-0000-4000-8000-%012d" tang dan, time.time -> 1_790_000_000.123 co dinh.
"""
import json
import os
import sys
import time
import uuid

sys.path.insert(0, r"A:\ToolLVC\lvcseedance")
from src.api import dola_app as d  # noqa: E402

_n = [0]


def fake_uuid4():
    _n[0] += 1
    return uuid.UUID(f"00000000-0000-4000-8000-{_n[0]:012d}")


d.uuid.uuid4 = fake_uuid4
d.time.time = lambda: 1_790_000_000.123


class Sess:
    bot_id = d.BOT_ID if hasattr(d, "BOT_ID") else "7241547611541340167"
    uid = "7000000000000000001"
    language = "ja"


S = Sess()
cases = []


def add(name, kind, args, fn):
    _n[0] = 0
    body = fn()
    cases.append({"name": name, "kind": kind, "args": args, "body": json.loads(body)})


for model in ("seedance_v2.0", "seedance_v2.5", "seedance_v1.0"):
    for ratio, dur in (("9:16", 10), ("16:9", 5), ("1:1", 15), ("4:3", 30), ("3:4", 7)):
        args = {"prompt": "Mot co gai mo hop qua | 'quote' \"dq\" 日本語", "model": model, "ratio": ratio, "duration": dur,
                "conversation_id": "", "media": 0}
        add(f"video_{model}_{ratio}_{dur}", "video", args,
            lambda: d.build_video_payload(S, args["prompt"], model, ratio, dur))

_n[0] = 0
blk = d.attachment_block({"uri": "tos-x/abc", "md5": "m" * 32, "width": 100, "height": 200})
args = {"prompt": "co anh", "model": "seedance_v2.0", "ratio": "9:16", "duration": 10, "conversation_id": "123", "media": 1}
cases.append({"name": "video_media_conv", "kind": "video", "args": args,
              "body": json.loads((lambda: (_n.__setitem__(0, 2), d.build_video_payload(S, "co anh", "seedance_v2.0", "9:16", 10, "123", media_blocks=[blk]))[1])())})

for model in ("seedream_4.5", "seedream_5.0_pro"):
    for ratio in ("1:1", "original", "2:3", "16:9"):
        args = {"prompt": "anh san pham", "model": model, "ratio": ratio, "conversation_id": ""}
        add(f"image_{model}_{ratio}", "image", args, lambda: d.build_image_payload(S, args["prompt"], model, ratio))

for pro in (False, True):
    for conv in ("", "555"):
        args = {"text": "Create a video", "pro": pro, "conversation_id": conv, "last_message_index": 4 if conv else 0,
                "section_id": "sec" if conv else ""}
        add(f"text_pro{pro}_conv{conv or 'new'}", "text", args,
            lambda: d.build_text_payload(S, args["text"], conv, last_message_index=args["last_message_index"],
                                         section_id=args["section_id"], pro=pro))

_n[0] = 0
cases.append({"name": "asset_block", "kind": "asset", "args": {"uri": "tos-mya-i-x/rc_gen_image/abc.jpeg", "name": ""},
              "body": d.asset_attachment_block("tos-mya-i-x/rc_gen_image/abc.jpeg")})
_n[0] = 0
cases.append({"name": "attachment_block", "kind": "attach", "args": {"uri": "tos-x/abc", "md5": "m" * 32, "width": 100, "height": 200},
              "body": d.attachment_block({"uri": "tos-x/abc", "md5": "m" * 32, "width": 100, "height": 200})})

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "payload_vectors.json")
with open(out, "w", encoding="utf-8", newline="\n") as f:
    json.dump({"uid": S.uid, "bot_id": S.bot_id, "now_ms": 1_790_000_000_123, "cases": cases}, f, ensure_ascii=False, indent=1)
print(out, len(cases))
