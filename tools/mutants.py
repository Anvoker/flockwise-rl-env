"""Checks that the verifier rejects known-bad solutions and accepts valid
alternatives.

Each mutant is the reference solution with one bug put in. Every mutant must
fail at least one check. Each alternative is a valid solution that makes a
different reasonable design choice. Every alternative must pass. Usage:

    python -I tools/mutants.py <task dir> <work dir> [mutant name ...]
    python -I tools/mutants.py <task dir> <work dir> --alternatives

Deletes only inside <work dir>.
"""

import os
import shutil
import subprocess
import sys
from pathlib import Path

NL = "\n"

ARC_BEST = [
    ("swarm.rs", "    best: BestSoFar<O::Pos>," + NL, "    best: std::sync::Arc<std::sync::Mutex<BestSoFar<O::Pos>>>," + NL, 0),
    ("swarm.rs", "best: BestSoFar::new(),", "best: Default::default(),", 0),
    ("swarm.rs", "self.best = BestSoFar::new();", "*self.best.lock().unwrap() = BestSoFar::new();", 0),
    ("swarm.rs", "score_all(&self.objective, &mut self.best, particles);", "score_all(&self.objective, &mut self.best.lock().unwrap(), particles);", 0),
    ("swarm.rs", "let (best, r1) = (&self.best, &self.r1);", "let best_value = *self.best.lock().unwrap();" + NL + "        let (best, r1) = (&best_value, &self.r1);", 1),
    ("swarm.rs", "let (best, r1, r2) = (&self.best, &self.r1, &self.r2);", "let best_value = *self.best.lock().unwrap();" + NL + "        let (best, r1, r2) = (&best_value, &self.r1, &self.r2);", 1),
    ("swarm.rs", "        self.best" + NL + "    }", "        *self.best.lock().unwrap()" + NL + "    }", 0),
]

# name: (what the bug is, [(file, old, new, count)]); count 0 means all.
MUTANTS = {
    "no_best_reset": (
        "Searches never reset their best so far: the real bug from 2026-02-06 to 2026-04-22",
        [("swarm.rs", "fn begin(&mut self) {" + NL + "        self.best = BestSoFar::new();" + NL + "    }", "fn begin(&mut self) {}", 0)],
    ),
    "inner_sees_nothing": (
        "The inner search is never shown the outer particle",
        [("rules.rs", "        search.see_outer(*p.pos);" + NL, "", 1)],
    ),
    "outer_shown_after_search": (
        "The inner search is shown the outer particle only after it ran",
        [
            ("rules.rs", "        search.see_outer(*p.pos);" + NL + NL, "", 1),
            ("rules.rs", ".last();" + NL, ".last();" + NL + "        search.see_outer(*p.pos);" + NL, 1),
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
    "seeder_ignores_given_rng": (
        "The inner search's particles are seeded from the thread RNG",
        [("rules.rs", "self.seeder.seed(*p.pos, rng)", "self.seeder.seed(*p.pos, &mut rand::rng())", 1)],
    ),
    "inner_seed_from_thread_rng": (
        "The inner search's run seed comes from the thread RNG",
        [("rules.rs", "let seed = rng.random::<u64>();", "let seed = rand::rng().random::<u64>();", 1)],
    ),
    "planner_skips_timing": (
        "The planner's inner timing search runs zero iterations",
        [("routes/plan.rs", "            budget.inner_iterations," + NL, "            0," + NL, 1)],
    ),
    "shared_best_arc": (
        "The best so far lives behind an Arc<Mutex>, so clones share it",
        ARC_BEST,
    ),
    "one_rng_for_all_particles": (
        "All particles draw from one RNG behind a lock, so results depend on thread scheduling",
        [
            (
                "swarm.rs",
                "        particles.par_iter_mut().zip(rngs.par_iter_mut()).for_each(|(p, rng)| {" + NL + "            r1.apply(best, rng, p.view());" + NL + "        });",
                "        let shared = std::sync::Mutex::new(rngs[0].clone());" + NL + "        particles.par_iter_mut().for_each(|p| {" + NL + "            let mut rng = shared.lock().unwrap();" + NL + "            r1.apply(best, &mut *rng, p.view());" + NL + "        });",
                1,
            ),
            (
                "swarm.rs",
                "        particles.par_iter_mut().zip(rngs.par_iter_mut()).for_each(|(p, rng)| {" + NL + "            r1.apply(best, rng, p.view());" + NL + "            r2.apply(best, rng, p.view());" + NL + "        });",
                "        let shared = std::sync::Mutex::new(rngs[0].clone());" + NL + "        particles.par_iter_mut().for_each(|p| {" + NL + "            let mut rng = shared.lock().unwrap();" + NL + "            r1.apply(best, &mut *rng, p.view());" + NL + "            r2.apply(best, &mut *rng, p.view());" + NL + "        });",
                1,
            ),
        ],
    ),
    "all_sequential": (
        "Nothing runs in parallel",
        [("swarm.rs", "par_iter_mut()", "iter_mut()", 0)],
    ),
    "later_particle_wins_ties": (
        "When two particles score the same, the later one becomes the best so far",
        [("swarm.rs", "        if p.score > best.best_score {", "        if p.score >= best.best_score {", 1)],
    ),
}

# Valid solutions that make other reasonable design choices than the reference.
ALTERNATIVES = {
    "hashed_particle_rngs": (
        "Each particle's RNG is seeded by hashing the run seed with the particle's index",
        [(
            "search.rs",
            "        let mut master = StdRng::seed_from_u64(seed);" + NL + "        let rngs = (0..particles.len()).map(|_| StdRng::seed_from_u64(master.random())).collect();",
            "        let rngs = (0..particles.len() as u64)" + NL + "            .map(|i| StdRng::seed_from_u64(seed ^ i.wrapping_add(1).wrapping_mul(0x9E37_79B9_7F4A_7C15)))" + NL + "            .collect();",
            1,
        )],
    ),
    "parallel_reduce_for_best": (
        "The best so far is found by a parallel reduction with an index tie-break",
        [(
            "swarm.rs",
            "    for p in particles.iter_mut() {" + NL
            + "        if p.score > p.best_score {" + NL
            + "            p.best_score = p.score;" + NL
            + "            p.best_pos = p.pos;" + NL
            + "        }" + NL + NL
            + "        if p.score > best.best_score {" + NL
            + "            best.best_score = p.score;" + NL
            + "            best.best_pos = p.pos;" + NL
            + "        }" + NL
            + "    }",
            "    particles.par_iter_mut().for_each(|p| {" + NL
            + "        if p.score > p.best_score {" + NL
            + "            p.best_score = p.score;" + NL
            + "            p.best_pos = p.pos;" + NL
            + "        }" + NL
            + "    });" + NL + NL
            + "    let (score, index) = particles" + NL
            + "        .par_iter()" + NL
            + "        .enumerate()" + NL
            + "        .map(|(i, p)| (p.score, i))" + NL
            + "        .reduce(|| (f64::NEG_INFINITY, usize::MAX), |a, b| if b.0 > a.0 || (b.0 == a.0 && b.1 < a.1) { b } else { a });" + NL
            + "    if index != usize::MAX && score > best.best_score {" + NL
            + "        best.best_score = score;" + NL
            + "        best.best_pos = particles[index].pos;" + NL
            + "    }",
            1,
        )],
    ),
    "arc_mutex_with_deep_clone": (
        "The best so far lives behind an Arc<Mutex> inside one swarm, and Clone copies it",
        ARC_BEST
        + [
            ("swarm.rs", "#[derive(Clone, Debug)]" + NL + "pub struct Swarm1", "#[derive(Debug)]" + NL + "pub struct Swarm1", 1),
            ("swarm.rs", "#[derive(Clone, Debug)]" + NL + "pub struct Swarm2", "#[derive(Debug)]" + NL + "pub struct Swarm2", 1),
            (
                "swarm.rs",
                "/// Scores every particle in parallel,",
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
                + "/// Scores every particle in parallel,",
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


def verify_variant(task: Path, work: Path, bash: str, verify: Path, name: str, edits):
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
    if not failed and "failed: 0" not in run.stdout:
        raise SystemExit(f"{name}: the verifier didn't run:\n{run.stdout}\n{run.stderr}")
    return failed


def main() -> None:
    task = Path(sys.argv[1]).resolve()
    work = Path(sys.argv[2]).resolve()
    work.mkdir(parents=True, exist_ok=True)
    verify = Path(__file__).resolve().parent / "local_verify.sh"
    # The bash that runs this script (Git Bash on Windows), not WSL's.
    bash = os.environ.get("BASH_EXE") or shutil.which("bash")

    if sys.argv[3:] == ["--alternatives"]:
        rejected = []
        for name, (why, edits) in ALTERNATIVES.items():
            failed = verify_variant(task, work, bash, verify, name, edits)
            if failed:
                print(f"REJECTED {name}: {why}: {', '.join(failed)}")
                rejected.append(name)
            else:
                print(f"accepted {name}")
            sys.stdout.flush()
        print(f"{len(ALTERNATIVES) - len(rejected)}/{len(ALTERNATIVES)} alternatives accepted")
        sys.exit(1 if rejected else 0)

    names = sys.argv[3:] or list(MUTANTS)
    escaped = []
    for name in names:
        why, edits = MUTANTS[name]
        failed = verify_variant(task, work, bash, verify, name, edits)
        if failed:
            print(f"caught   {name}: {', '.join(failed)}")
        else:
            print(f"ESCAPED  {name}: {why}")
            escaped.append(name)
        sys.stdout.flush()

    print(f"{len(names) - len(escaped)}/{len(names)} mutants caught")
    sys.exit(1 if escaped else 0)


if __name__ == "__main__":
    main()
