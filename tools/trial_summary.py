"""Summarizes one Harbor trial of the claude-code agent: effort, reward, and
what the agent's source contains (for spotting hacks by eye; this is a
reading aid, not part of the verifier).

Usage: python -I tools/trial_summary.py <trial dir>
"""

import json
import re
import sys
from pathlib import Path

trial = Path(sys.argv[1])

log = (trial / "agent" / "claude-code.txt").read_text(encoding="utf-8").splitlines()
final = json.loads(log[-1])
usage = final.get("usage", {})
print(f"turns {final.get('num_turns')} | agent time {round(final.get('duration_ms', 0) / 1000)} s"
      f" | API-equivalent ${final.get('total_cost_usd', 0):.2f}")
print(f"tokens: output {usage.get('output_tokens')}, cache read {usage.get('cache_read_input_tokens')},"
      f" cache write {usage.get('cache_creation_input_tokens')}")

tools = {}
for line in log:
    for name in re.findall(r'"type":"tool_use","id":"[^"]*","name":"([A-Za-z]+)"', line):
        tools[name] = tools.get(name, 0) + 1
print("tool calls:", ", ".join(f"{k} {v}" for k, v in sorted(tools.items(), key=lambda kv: -kv[1])))

reward = (trial / "verifier" / "reward.txt")
print("reward:", reward.read_text().strip() if reward.exists() else "missing")

src = trial / "artifacts" / "app" / "src"
files = sorted(p for p in src.rglob("*.rs"))
print("source files:", ", ".join(f"{p.relative_to(src)} ({len(p.read_text(encoding='utf-8').splitlines())})" for p in files))
patterns = ["unsafe", "Rc<", "RefCell", "Arc<", "Mutex", "static mut", "thread_rng", "rand::rng()", "transmute"]
for p in files:
    text = p.read_text(encoding="utf-8")
    hits = [pat for pat in patterns if pat in text]
    if hits:
        print(f"  {p.relative_to(src)} mentions: {', '.join(hits)}")
