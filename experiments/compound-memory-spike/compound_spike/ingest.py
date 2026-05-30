import json
from datetime import datetime, timezone
from pathlib import Path
from compound_spike.models import Message

def _epoch_to_iso(epoch: float | None) -> str:
    if not epoch:
        return ""
    return datetime.fromtimestamp(epoch, tz=timezone.utc).isoformat()

def parse_chatgpt(data: list[dict]) -> list[Message]:
    out: list[Message] = []
    for conv in data:
        conv_id = conv.get("title", "") + str(conv.get("create_time", ""))
        nodes = conv.get("mapping", {})
        # order by message create_time; skip null-message (root) nodes
        items = []
        for node in nodes.values():
            msg = node.get("message")
            if not msg:
                continue
            role = (msg.get("author") or {}).get("role")
            if role not in ("user", "assistant"):
                continue
            parts = (msg.get("content") or {}).get("parts") or []
            text = "\n".join(p for p in parts if isinstance(p, str)).strip()
            if not text:
                continue
            items.append((msg.get("create_time") or 0.0, msg, role, text))
        items.sort(key=lambda t: t[0])
        for ct, msg, role, text in items:
            out.append(Message(
                source="chatgpt", conv_id=conv_id, msg_id=msg.get("id", ""),
                role=role, text=text, ts=_epoch_to_iso(ct),
            ))
    return out

def _claude_text(m: dict) -> str:
    if (m.get("text") or "").strip():
        return m["text"].strip()
    blocks = m.get("content") or []
    return "\n".join(b.get("text", "") for b in blocks if b.get("type") == "text").strip()

def parse_claude(data: list[dict]) -> list[Message]:
    out: list[Message] = []
    for conv in data:
        conv_id = conv.get("uuid", "")
        for m in conv.get("chat_messages", []):
            role = {"human": "user", "assistant": "assistant"}.get(m.get("sender", ""))
            if role is None:
                continue
            text = _claude_text(m)
            if not text:
                continue
            out.append(Message(
                source="claude", conv_id=conv_id, msg_id=m.get("uuid", ""),
                role=role, text=text, ts=m.get("created_at", ""),
            ))
    return out

def load_all(chatgpt_path: Path | None = None, claude_path: Path | None = None) -> list[Message]:
    msgs: list[Message] = []
    if chatgpt_path:
        msgs += parse_chatgpt(json.loads(Path(chatgpt_path).read_text()))
    if claude_path:
        msgs += parse_claude(json.loads(Path(claude_path).read_text()))
    return msgs
