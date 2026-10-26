"""Checks that the verifier rejects known-bad solutions.

Each mutant is the reference solution with one bug put in. Every mutant must
fail at least one check. Usage:

    python -I tools/mutants.py <task dir> <work dir> [mutant name ...]
    python -I tools/mutants.py <task dir> <work dir> --alternatives
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

NL = "\n"

# name: (what the bug is, [(file, old, new, count)]); count 0 means all.
MUTANTS = {
    "no_best_reset": (
        "Searches never reset their best so far: the real bug from 2026-02-06 to 2026-04-22",
        [("swarm.rs", "fn begin(&mut self) {" + NL + "        self.best = BestSoFar::new();" + NL + "    }", "fn begin(&mut self) {}", 0)],
    ),
    "inner_sees_nothing": (
        "The inner search is never shown the outer particle",
        [("rules.rs", "        self.search.see_outer(*p.pos);" + NL, "", 1)],
    ),
    "outer_shown_after_search": (
        "The inner search is shown the outer particle only after it ran",
        [
            ("rules.rs", "        self.search.see_outer(*p.pos);" + NL + NL, "", 1),
            ("rules.rs", ".last();" + NL, ".last();" + NL + "        self.search.see_outer(*p.pos);" + NL, 1),
        ],
    ),
    "region_sees_nothing": (
        "Confined rules don't show the outer particle to their region",
        [("rules.rs", "        self.region.see_outer(outer);" + NL, "", 1)],
    ),
    "rules_see_nothing": (
        "Swarms show the outer particle to the objective only, not to their rules",
        [
            ("swarm.rs", "        self.r1.see_outer(outer);" + NL, "", 0),
            ("swarm.rs", "        self.r2.see_outer(outer);" + NL, "", 0),
        ],
    ),
    "one_iteration_short": (
        "A run does one iteration fewer than asked: the starting state's off-by-one",
        [("search.rs", "if self.done >= self.iterations", "if self.done + 1 >= self.iterations", 1)],
    ),
    "reversed_velocity": (
        "Velocity is set to minus the step taken: the starting state's PSO bug",
        [(
            "swarm.rs",
            "        *p.pos = *p.pos + *p.vel;" + NL,
            "        let old = *p.pos;" + NL + "        *p.pos = *p.pos + *p.vel;" + NL + "        *p.vel = old - *p.pos;" + NL,
            1,
        )],
    ),
    "scalar_random_factors": (
        "PSO draws one random factor per particle, not per coordinate: the starting state's version",
        [
            (
                "swarm.rs",
                "let cognitive = (*p.best_pos - *p.pos).map_scalars(|d| d * c1 * rng.random::<f64>());",
                "let (r1, r2) = (rng.random::<f64>(), rng.random::<f64>());" + NL + "        let cognitive = (*p.best_pos - *p.pos) * (c1 * r1);",
                1,
            ),
            (
                "swarm.rs",
                "let social = (shared.best_pos - *p.pos).map_scalars(|d| d * c2 * rng.random::<f64>());",
                "let social = (shared.best_pos - *p.pos) * (c2 * r2);",
                1,
            ),
        ],
    ),
    "field_ignores_best": (
        "A field-restricted rule is shown the particle's own best instead of the swarm's",
        [("rules.rs", "best_pos: <R::Pos as ReadFrom>::read(&shared.best_pos),", "best_pos: <R::Pos as ReadFrom>::read(&*p.best_pos),", 1)],
    ),
    "inner_writes_nothing": (
        "The inner search runs but its result is never written back",
        [("rules.rs", "            best.best_pos.write(p.pos);" + NL, "            let _ = best;" + NL, 1)],
    ),
    "ignores_given_rng": (
        "The inner search seeds from the thread RNG instead of the one it was given",
        [("rules.rs", "self.seeder.seed(*p.pos, &mut self.rng)", "self.seeder.seed(*p.pos, &mut rand::rng())", 1)],
    ),
    "planner_skips_timing": (
        "The planner's inner timing search runs zero iterations",
        [("routes/plan.rs", "            budget.inner_iterations," + NL, "            0," + NL, 1)],
    ),
    "shared_best_arc": (
        "The best so far lives behind an Arc<Mutex>, so clones share it",
        [
            ("swarm.rs", "    best: BestSoFar<O::Pos>," + NL, "    best: std::sync::Arc<std::sync::Mutex<BestSoFar<O::Pos>>>," + NL, 0),
            ("swarm.rs", "best: BestSoFar::new(),", "best: Default::default(),", 0),
            ("swarm.rs", "self.best = BestSoFar::new();", "*self.best.lock().unwrap() = BestSoFar::new();", 0),
            ("swarm.rs", "score(&self.objective, &mut self.best, p);", "score(&self.objective, &mut self.best.lock().unwrap(), p);", 0),
            ("swarm.rs", "self.r1.apply(&self.best, p.view());", "let b = *self.best.lock().unwrap();" + NL + "            self.r1.apply(&b, p.view());", 0),
            ("swarm.rs", "self.r2.apply(&self.best, p.view());", "let b = *self.best.lock().unwrap();" + NL + "            self.r2.apply(&b, p.view());", 0),
            ("swarm.rs", "        self.best" + NL + "    }", "        *self.best.lock().unwrap()" + NL + "    }", 0),
        ],
    ),
    "shared_best_rc": (
        "The best so far lives behind an Rc<RefCell>, so searches aren't Send",
        [
            ("swarm.rs", "    best: BestSoFar<O::Pos>," + NL, "    best: std::rc::Rc<std::cell::RefCell<BestSoFar<O::Pos>>>," + NL, 0),
            ("swarm.rs", "best: BestSoFar::new(),", "best: Default::default(),", 0),
            ("swarm.rs", "self.best = BestSoFar::new();", "*self.best.borrow_mut() = BestSoFar::new();", 0),
            ("swarm.rs", "score(&self.objective, &mut self.best, p);", "score(&self.objective, &mut self.best.borrow_mut(), p);", 0),
            ("swarm.rs", "self.r1.apply(&self.best, p.view());", "let b = *self.best.borrow();" + NL + "            self.r1.apply(&b, p.view());", 0),
            ("swarm.rs", "self.r2.apply(&self.best, p.view());", "let b = *self.best.borrow();" + NL + "            self.r2.apply(&b, p.view());", 0),
            ("swarm.rs", "        self.best" + NL + "    }", "        *self.best.borrow()" + NL + "    }", 0),
        ],
    ),
}


# Valid solutions that make other reasonable design choices than the reference.
# Every one of them must pass every check.
ALTERNATIVES = {
    "score_all_then_move_all": (
        "Each iteration scores every particle first, then moves every particle",
        [
            (
                "swarm.rs",
                "            score(&self.objective, &mut self.best, p);" + NL + "            self.r1.apply(&self.best, p.view());" + NL + "        }" + NL,
                "            score(&self.objective, &mut self.best, p);" + NL + "        }" + NL + "        for p in particles.iter_mut() {" + NL + "            self.r1.apply(&self.best, p.view());" + NL + "        }" + NL,
                1,
            ),
            (
                "swarm.rs",
                "            score(&self.objective, &mut self.best, p);" + NL + "            self.r1.apply(&self.best, p.view());" + NL + "            self.r2.apply(&self.best, p.view());" + NL,
                "            score(&self.objective, &mut self.best, p);" + NL + "        }" + NL + "        for p in particles.iter_mut() {" + NL + "            self.r1.apply(&self.best, p.view());" + NL + "            self.r2.apply(&self.best, p.view());" + NL,
                1,
            ),
        ],
    ),
    "arc_mutex_with_deep_clone": (
        "The best so far lives behind an Arc<Mutex> inside one swarm, and Clone copies it",
        [edit for edit in MUTANTS["shared_best_arc"][1]]
        + [
            ("swarm.rs", "#[derive(Clone, Debug)]" + NL + "pub struct Swarm1", "#[derive(Debug)]" + NL + "pub struct Swarm1", 1),
            ("swarm.rs", "#[derive(Clone, Debug)]" + NL + "pub struct Swarm2", "#[derive(Debug)]" + NL + "pub struct Swarm2", 1),
            (
                "swarm.rs",
                "fn score<O: Objective>(",
                "impl<O: Objective + Clone, R1: Clone> Clone for Swarm1<O, R1> {" + NL
                + "    fn clone(&self) -> Self {" + NL
                + "        let best = *self.best.lock().unwrap();" + NL
                + "        Swarm1 { objective: self.objective.clone(), best: std::sync::Arc::new(std::sync::Mutex::new(best)), r1: self.r1.clone() }" + NL
                + "    }" + NL
                + "}" + NL + NL
                + "impl<O: Objective + Clone, R1: Clone, R2: Clone> Clone for Swarm2<O, R1, R2> {" + NL
                + "    fn clone(&self) -> Self {" + NL
                + "        let best = *self.best.lock().unwrap();" + NL
                + "        Swarm2 { objective: self.objective.clone(), best: std::sync::Arc::new(std::sync::Mutex::new(best)), r1: self.r1.clone(), r2: self.r2.clone() }" + NL
                + "    }" + NL
                + "}" + NL + NL
                + "fn score<O: Objective>(",
                1,
            ),
        ],
    ),
    "fresh_inner_search_per_particle": (
        "The inner search is cloned from a pristine template for every particle, instead of being reset",
        [
            (
                "rules.rs",
                "        self.search.see_outer(*p.pos);" + NL,
                "        let mut search = self.search.clone();" + NL + "        search.see_outer(*p.pos);" + NL,
                1,
            ),
            ("rules.rs", "Run::new(self.iterations, &mut self.search, &mut particles)", "Run::new(self.iterations, &mut search, &mut particles)", 1),
            (
                "rules.rs",
                "impl<S, D, R, Outer> Rule for InnerSearch<S, D, R, Outer>" + NL + "where" + NL,
                "impl<S, D, R, Outer> Rule for InnerSearch<S, D, R, Outer>" + NL + "where" + NL + "    S: Clone," + NL,
                1,
            ),
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
        path.write_text(text, encoding="utf-8", newline=NL)


def run_alternatives(task: Path, work: Path, bash: str, verify: Path) -> None:
    rejected = []
    for name, (why, edits) in ALTERNATIVES.items():
        src = work / "src" / name
        shutil.rmtree(src, ignore_errors=True)
        shutil.copytree(task / "solution" / "src", src)
        apply(src, edits)
        run = subprocess.run(
            [bash, verify.as_posix(), task.as_posix(), src.as_posix(), (work / "runs" / name).as_posix()],
            capture_output=True,
            text=True,
        )
        failed = [line.split()[1] for line in run.stdout.splitlines() if line.startswith("FAIL")]
        if failed:
            print(f"REJECTED {name}: {why}: {', '.join(failed)}")
            rejected.append(name)
        else:
            print(f"accepted {name}")
        sys.stdout.flush()
    print(f"{len(ALTERNATIVES) - len(rejected)}/{len(ALTERNATIVES)} alternatives accepted")
    sys.exit(1 if rejected else 0)


def main() -> None:
    task = Path(sys.argv[1]).resolve()
    work = Path(sys.argv[2]).resolve()
    verify = Path(__file__).resolve().parent / "local_verify.sh"
    # The bash that runs this script (Git Bash on Windows), not WSL's.
    bash = os.environ.get("BASH_EXE") or shutil.which("bash")
    if sys.argv[3:] == ["--alternatives"]:
        run_alternatives(task, work, bash, verify)
    names = sys.argv[3:] or list(MUTANTS)
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
