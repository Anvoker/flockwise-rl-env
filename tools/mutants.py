"""Checks that the verifier rejects known-bad solutions.

Each mutant is the reference solution with one bug put in. Every mutant must
fail at least one check. Usage:

    python -I tools/mutants.py <task dir> <work dir> [mutant name ...]
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

# name: (what the bug is, [(file, old, new, count)]); count 0 means all.
MUTANTS = {
    "no_gbest_reset": (
        "Searchers never reset their global best: the real bug from 2026-02-06 to 2026-04-22",
        [("gbest.rs", "fn begin(&mut self) {\n        self.gbest = GBest::new();\n    }", "fn begin(&mut self) {}", 0)],
    ),
    "nested_sets_no_context": (
        "The nested mover never passes the outer particle to the inner search",
        [("movers.rs", "        self.searcher.set_context(*p.pos);\n", "", 1)],
    ),
    "context_after_search": (
        "The nested mover passes the outer particle only after the inner search ran",
        [
            ("movers.rs", "        self.searcher.set_context(*p.pos);\n\n", "", 1),
            ("movers.rs", ".last();\n", ".last();\n        self.searcher.set_context(*p.pos);\n", 1),
        ],
    ),
    "boundary_gets_no_context": (
        "Bounded movers don't forward context to their boundary",
        [("movers.rs", "        self.boundary.set_context(context);\n", "", 1)],
    ),
    "movers_get_no_context": (
        "Searchers forward context to the fitness only, not to their movers",
        [
            ("gbest.rs", "        self.tu1.set_context(context);\n", "", 0),
            ("gbest.rs", "        self.tu2.set_context(context);\n", "", 0),
        ],
    ),
    "one_iteration_short": (
        "A search runs one iteration fewer than asked: the starting state's off-by-one",
        [("searcher.rs", "if self.iteration >= self.max_iteration", "if self.iteration + 1 >= self.max_iteration", 1)],
    ),
    "reversed_velocity": (
        "Velocity is set to minus the step taken: the starting state's PSO bug",
        [("gbest.rs", "        *p.pos = *p.pos + *p.vel;\n", "        let old = *p.pos;\n        *p.pos = *p.pos + *p.vel;\n        *p.vel = old - *p.pos;\n", 1)],
    ),
    "scalar_random_factors": (
        "PSO draws one random factor per particle, not per coordinate: the starting state's version",
        [
            ("gbest.rs", "let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * c1 * rng.random::<f64>());",
             "let (r1, r2) = (rng.random::<f64>(), rng.random::<f64>());\n        let cognitive = (*p.best_pos - *p.pos) * (c1 * r1);", 1),
            ("gbest.rs", "let social = (common.best_pos - *p.pos).map_scalars(|d| d * c2 * rng.random::<f64>());",
             "let social = (common.best_pos - *p.pos) * (c2 * r2);", 1),
        ],
    ),
    "part_ignores_global_best": (
        "A part-restricted mover is shown the particle's personal best instead of the global best",
        [("movers.rs", "best_pos: TMover::TUnit::get(&common.best_pos),", "best_pos: TMover::TUnit::get(&*p.best_pos),", 1)],
    ),
    "nested_writes_nothing": (
        "The nested mover runs the inner search but never writes the result back",
        [("movers.rs", "            *TInit::TInner::get_mut(p.pos) = best.best_pos;\n", "            let _ = best;\n", 1)],
    ),
    "ignores_given_rng": (
        "The nested mover uses the thread RNG instead of the one it was given",
        [("movers.rs", "self.init.init(*p.pos, &mut self.rng)", "self.init.init(*p.pos, &mut rand::rng())", 1)],
    ),
    "shared_gbest_arc": (
        "The global best lives behind an Arc<Mutex>, so clones share it",
        [
            ("gbest.rs", "    gbest: GBest<TFit::T>,\n", "    gbest: std::sync::Arc<std::sync::Mutex<GBest<TFit::T>>>,\n", 0),
            ("gbest.rs", "gbest: GBest::new(),", "gbest: Default::default(),", 0),
            ("gbest.rs", "self.gbest = GBest::new();", "*self.gbest.lock().unwrap() = GBest::new();", 0),
            ("gbest.rs", "evaluate(&self.fit_calc, &mut self.gbest, p);", "evaluate(&self.fit_calc, &mut self.gbest.lock().unwrap(), p);", 0),
            ("gbest.rs", "self.tu1.update(&self.gbest, p.as_mut());", "let g = *self.gbest.lock().unwrap();\n            self.tu1.update(&g, p.as_mut());", 0),
            ("gbest.rs", "self.tu2.update(&self.gbest, p.as_mut());", "let g = *self.gbest.lock().unwrap();\n            self.tu2.update(&g, p.as_mut());", 0),
            ("gbest.rs", "        self.gbest\n    }", "        *self.gbest.lock().unwrap()\n    }", 0),
        ],
    ),
    "shared_gbest_rc": (
        "The global best lives behind an Rc<RefCell>, so searchers aren't Send",
        [
            ("gbest.rs", "    gbest: GBest<TFit::T>,\n", "    gbest: std::rc::Rc<std::cell::RefCell<GBest<TFit::T>>>,\n", 0),
            ("gbest.rs", "gbest: GBest::new(),", "gbest: Default::default(),", 0),
            ("gbest.rs", "self.gbest = GBest::new();", "*self.gbest.borrow_mut() = GBest::new();", 0),
            ("gbest.rs", "evaluate(&self.fit_calc, &mut self.gbest, p);", "evaluate(&self.fit_calc, &mut self.gbest.borrow_mut(), p);", 0),
            ("gbest.rs", "self.tu1.update(&self.gbest, p.as_mut());", "let g = *self.gbest.borrow();\n            self.tu1.update(&g, p.as_mut());", 0),
            ("gbest.rs", "self.tu2.update(&self.gbest, p.as_mut());", "let g = *self.gbest.borrow();\n            self.tu2.update(&g, p.as_mut());", 0),
            ("gbest.rs", "        self.gbest\n    }", "        *self.gbest.borrow()\n    }", 0),
        ],
    ),
}


def apply(src: Path, edits) -> None:
    for name, old, new, count in edits:
        path = src / name
        text = path.read_text(encoding="utf-8")
        found = text.count(old)
        if found == 0 or (count and found < count):
            raise SystemExit(f"{name}: pattern not found: {old!r}")
        text = text.replace(old, new) if count == 0 else text.replace(old, new, count)
        path.write_text(text, encoding="utf-8")


def main() -> None:
    task = Path(sys.argv[1]).resolve()
    work = Path(sys.argv[2]).resolve()
    names = sys.argv[3:] or list(MUTANTS)
    verify = Path(__file__).resolve().parent / "local_verify.sh"
    # The bash that runs this script (Git Bash on Windows), not WSL's.
    bash = os.environ.get("BASH_EXE") or shutil.which("bash")
    escaped = []

    for name in names:
        why, edits = MUTANTS[name]
        src = work / "src" / name
        shutil.rmtree(src, ignore_errors=True)
        shutil.copytree(task / "solution" / "src", src)
        apply(src, edits)

        run = subprocess.run(
            [bash, verify.as_posix(), task.as_posix(), src.as_posix(), (work / "runs" / name).as_posix()],
            capture_output=True,
            text=True,
        )
        caught = [line.split()[1] for line in run.stdout.splitlines() if line.startswith("FAIL")]
        if caught:
            print(f"caught   {name}: {', '.join(caught)}")
        elif "failed: 0" not in run.stdout:
            raise SystemExit(f"{name}: the verifier didn't run:\n{run.stdout}\n{run.stderr}")
        else:
            print(f"ESCAPED  {name}: {why}")
            escaped.append(name)
        sys.stdout.flush()

    print(f"{len(names) - len(escaped)}/{len(names)} mutants caught")
    sys.exit(1 if escaped else 0)


if __name__ == "__main__":
    main()
