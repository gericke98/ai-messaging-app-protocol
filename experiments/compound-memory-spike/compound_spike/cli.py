import argparse, json
from pathlib import Path
from compound_spike.ingest import load_all
from compound_spike.distill import distill
from compound_spike.harness import run_all
from compound_spike.judge import build_blind_table
from compound_spike.models import Profile
from compound_spike.tokens import TokenCounter
from config import OUT_DIR, DATA_DIR

OUT_DIR.mkdir(exist_ok=True)

def _cmd_ingest(args):
    msgs = load_all(chatgpt_path=args.chatgpt, claude_path=args.claude)
    out = OUT_DIR / "messages.json"
    out.write_text(json.dumps([m.__dict__ for m in msgs], indent=2))
    print(f"ingested {len(msgs)} messages -> {out}")

def _cmd_distill(args):
    from compound_spike.models import Message
    msgs = [Message(**d) for d in json.loads((OUT_DIR / "messages.json").read_text())]
    profile = distill(msgs, TokenCounter())
    (OUT_DIR / "profile.json").write_text(json.dumps(profile.to_dict(), indent=2))
    print(f"distilled {len(profile.active())} active facts -> {OUT_DIR/'profile.json'}")

def _cmd_run(args):
    from compound_spike.models import Message
    msgs = [Message(**d) for d in json.loads((OUT_DIR / "messages.json").read_text())]
    profile = Profile.from_dict(json.loads((OUT_DIR / "profile.json").read_text()))
    results = run_all(task=args.task, profile=profile, messages=msgs, counter=TokenCounter())
    (OUT_DIR / "results.json").write_text(json.dumps({"task": args.task, "results": results}, indent=2))
    print(f"ran {len(results)} arm-runs -> {OUT_DIR/'results.json'}")

def _cmd_judge(args):
    payload = json.loads((OUT_DIR / "results.json").read_text())
    df, mapping = build_blind_table(payload["results"], seed=args.seed)
    df.to_csv(OUT_DIR / "blind_table.csv", index=False)
    (OUT_DIR / "blind_mapping.json").write_text(json.dumps(mapping, indent=2))
    print(f"task: {payload['task']}")
    print(f"score blind_table.csv (fill 1-5), then reveal with blind_mapping.json")

def main():
    p = argparse.ArgumentParser(prog="compound_spike")
    sub = p.add_subparsers(required=True)
    pi = sub.add_parser("ingest"); pi.add_argument("--chatgpt", type=Path); pi.add_argument("--claude", type=Path); pi.set_defaults(func=_cmd_ingest)
    pd_ = sub.add_parser("distill"); pd_.set_defaults(func=_cmd_distill)
    pr = sub.add_parser("run"); pr.add_argument("--task", required=True); pr.set_defaults(func=_cmd_run)
    pj = sub.add_parser("judge"); pj.add_argument("--seed", type=int, default=0); pj.set_defaults(func=_cmd_judge)
    args = p.parse_args(); args.func(args)

if __name__ == "__main__":
    main()
