from compound_spike.models import Message, Fact, Provenance, Profile

def test_profile_roundtrips_through_json():
    profile = Profile(
        subject="user",
        facts=[
            Fact(
                id="f_1",
                category="preferences",
                statement="Prefers Rust over Go",
                status="active",
                superseded_by=None,
                first_seen="2025-11-02",
                last_seen="2026-04-18",
                provenance=[Provenance(source="claude", conv_id="c1", msg_id="m1", ts="2026-04-18")],
            )
        ],
    )
    restored = Profile.from_dict(profile.to_dict())
    assert restored == profile
    assert restored.facts[0].provenance[0].source == "claude"

def test_message_is_hashable_dataclass():
    m = Message(source="chatgpt", conv_id="c1", msg_id="m1", role="user", text="hi", ts="2026-01-01")
    assert m.text == "hi"
