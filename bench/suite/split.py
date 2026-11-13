#!/usr/bin/env python3
"""Sets each suite task's split (meta.json): "held-out" for about a third of each kind, drawn (seeded) only from tasks
nobody has read closely; "dev" for the rest. Held-out tasks are not looked at while improving tazuna: a change ships
only if it helps on dev and does not hurt on held-out (DESIGN.md). Rerun after adding tasks: a task keeps its split."""
import json, random
from pathlib import Path

TASKS = Path(__file__).resolve().parents[1] / "realwork" / "tasks-suite"
# read closely while building the pilots (2026-11-12/13): their prompts, answers, references and checks
SEEN = {"review-n8n-resume-limit", "review-n8n-entity-rename", "review-pydantic-final-default",
        "investigate-astro-18535", "investigate-astro-18608", "investigate-astro-18630",
        "plan-astro-18399", "plan-astro-18497", "plan-astro-18506",
        "question-astro-18535", "question-n8n-43437", "question-query-12525",
        "change-astro-18651", "change-astro-18500", "change-astro-18376", "change-astro-18488",
        "ops-combine-prs", "ops-rebase-conflict", "ops-ci-fix-same-pr",
        "followup-plan-go-astro-18488", "followup-clarify-n8n-43437", "followup-ci-same-pr"}


def main():
    metas = {d.name: json.loads((d / "meta.json").read_text()) for d in sorted(TASKS.iterdir()) if (d / "meta.json").exists()}
    for kind in sorted({m["kind"] for m in metas.values()}):
        tasks = [t for t, m in metas.items() if m["kind"] == kind]
        kept = [t for t in tasks if metas[t].get("split") == "held-out"]
        want = round(len(tasks) / 3) - len(kept)
        fresh = sorted(t for t in tasks if t not in SEEN and metas[t].get("split") != "held-out")
        random.Random(f"split-{kind}").shuffle(fresh)
        for t in fresh[:max(want, 0)]:
            metas[t]["split"] = "held-out"
        for t in tasks:
            metas[t].setdefault("split", "dev")
            (TASKS / t / "meta.json").write_text(json.dumps(metas[t], indent=2) + "\n")
        held = sorted(t for t in tasks if metas[t]["split"] == "held-out")
        print(f"{kind:12} {len(tasks):2} tasks, held out {len(held)}: {', '.join(held) or '-'}")


if __name__ == "__main__":
    main()
