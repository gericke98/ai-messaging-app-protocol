import json
from pathlib import Path
from compound_spike.ingest import parse_chatgpt, parse_claude, load_all

FIX = Path(__file__).parent / "fixtures"

def test_parse_chatgpt_extracts_user_and_assistant_in_order():
    data = json.loads((FIX / "chatgpt_export_sample.json").read_text())
    msgs = parse_chatgpt(data)
    assert [m.role for m in msgs] == ["user", "assistant"]
    assert msgs[0].source == "chatgpt"
    assert "Rust over Go" in msgs[0].text
    assert msgs[0].ts.startswith("2024-")  # epoch 1730500001 -> 2024-11
    assert msgs[0].conv_id and msgs[0].msg_id == "msg_a"

def test_parse_claude_handles_text_and_content_array():
    data = json.loads((FIX / "claude_export_sample.json").read_text())
    msgs = parse_claude(data)
    assert [m.role for m in msgs] == ["user", "assistant"]
    assert "Streaks" in msgs[0].text
    assert msgs[1].text == "Nice — SwiftUI is great for that."
    assert msgs[0].source == "claude"

def test_load_all_merges_both_sources(tmp_path):
    cg = FIX / "chatgpt_export_sample.json"
    cl = FIX / "claude_export_sample.json"
    msgs = load_all(chatgpt_path=cg, claude_path=cl)
    sources = {m.source for m in msgs}
    assert sources == {"chatgpt", "claude"}
    assert len(msgs) == 4
