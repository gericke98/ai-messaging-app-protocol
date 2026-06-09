import argparse, json
from pathlib import Path
from compound_spike.ingest import load_all
from compound_spike.distill import distill
from compound_spike.harness import run_all
from compound_spike.judge import build_blind_table
from compound_spike.models import Profile
from compound_spike.tokens import TokenCounter
from config import OUT_DIR

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

def _load_messages():
    from compound_spike.models import Message
    return [Message(**d) for d in json.loads((OUT_DIR / "messages.json").read_text())]

def _cmd_probes(args):
    from compound_spike.probes import generate_probes
    extra = []
    if args.extra:
        extra = [t.strip() for t in Path(args.extra).read_text().splitlines() if t.strip()]
    probes = generate_probes(_load_messages(), n=args.n, seed=args.seed, extra=extra)
    (OUT_DIR / "probes.json").write_text(json.dumps(probes, indent=2))
    print(f"generated {len(probes)} probes -> {OUT_DIR/'probes.json'}")

def _cmd_preflight(args):
    from compound_spike.llm import complete
    from config import CONSUMER_MODELS, DISTILL_MODEL, JUDGE_MODEL
    failures = 0
    for model in [DISTILL_MODEL, *CONSUMER_MODELS, JUDGE_MODEL]:
        try:
            complete(model=model, system="Reply with OK.", user="ping", temperature=0.0)
            print(f"  ok   {model}")
        except Exception as e:
            failures += 1
            print(f"  FAIL {model}: {e}")
    raise SystemExit(1 if failures else 0)

def _cmd_gate(args):
    from compound_spike.gate import run_gate
    from config import N_RUNS
    messages = _load_messages()
    profile = Profile.from_dict(json.loads((OUT_DIR / "profile.json").read_text()))
    probes = json.loads(Path(args.probes_file or OUT_DIR / "probes.json").read_text())
    report = run_gate(probes, profile, messages,
                      runs=args.runs or N_RUNS, out_dir=OUT_DIR / "gate")
    v = report["verdict"]
    print((OUT_DIR / "gate" / "report.md").read_text())
    print(f"verdict: {'PASS' if v['pass'] else 'FAIL'} -> {OUT_DIR/'gate'}")

def main():
    p = argparse.ArgumentParser(prog="compound_spike")
    sub = p.add_subparsers(required=True)
    pi = sub.add_parser("ingest"); pi.add_argument("--chatgpt", type=Path); pi.add_argument("--claude", type=Path); pi.set_defaults(func=_cmd_ingest)
    pd_ = sub.add_parser("distill"); pd_.set_defaults(func=_cmd_distill)
    pr = sub.add_parser("run"); pr.add_argument("--task", required=True); pr.set_defaults(func=_cmd_run)
    pj = sub.add_parser("judge"); pj.add_argument("--seed", type=int, default=0); pj.set_defaults(func=_cmd_judge)
    pp = sub.add_parser("probes"); pp.add_argument("--n", type=int, default=20); pp.add_argument("--seed", type=int, default=0); pp.add_argument("--extra", type=str, default=None); pp.set_defaults(func=_cmd_probes)
    pf = sub.add_parser("preflight"); pf.set_defaults(func=_cmd_preflight)
    pg = sub.add_parser("gate"); pg.add_argument("--runs", type=int, default=None); pg.add_argument("--probes-file", type=str, default=None); pg.set_defaults(func=_cmd_gate)
    args = p.parse_args(); args.func(args)

if __name__ == "__main__":
    main()
