from __future__ import annotations
from dataclasses import dataclass, asdict, field

@dataclass
class Message:
    source: str        # "chatgpt" | "claude"
    conv_id: str
    msg_id: str
    role: str          # "user" | "assistant"
    text: str
    ts: str            # ISO-8601

@dataclass
class Provenance:
    source: str
    conv_id: str
    msg_id: str
    ts: str

@dataclass
class Fact:
    id: str
    category: str      # identity|preferences|projects|decisions|patterns|relationships
    statement: str
    status: str = "active"          # active | superseded
    superseded_by: str | None = None
    first_seen: str = ""
    last_seen: str = ""
    provenance: list[Provenance] = field(default_factory=list)

    def to_dict(self) -> dict:
        d = asdict(self)
        return d

    @classmethod
    def from_dict(cls, d: dict) -> "Fact":
        prov = [Provenance(**p) for p in d.get("provenance", [])]
        return cls(
            id=d["id"], category=d["category"], statement=d["statement"],
            status=d.get("status", "active"), superseded_by=d.get("superseded_by"),
            first_seen=d.get("first_seen", ""), last_seen=d.get("last_seen", ""),
            provenance=prov,
        )

@dataclass
class Profile:
    subject: str
    facts: list[Fact] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {"subject": self.subject, "facts": [f.to_dict() for f in self.facts]}

    @classmethod
    def from_dict(cls, d: dict) -> "Profile":
        return cls(subject=d["subject"], facts=[Fact.from_dict(f) for f in d.get("facts", [])])

    def active(self) -> list[Fact]:
        return [f for f in self.facts if f.status == "active"]
