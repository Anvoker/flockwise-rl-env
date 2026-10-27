# flockwise: an RL environment for Rust architecture tasks

An agentic coding task in Rust with a deterministic verifier, in the [Harbor](https://harborframework.com) task format. It's built from real problems in the history of a particle swarm optimization (PSO) library, [swarmkit](https://github.com/Anvoker/swarmkit), cut down and renamed `flockwise` for the environment.

| Task | What the agent must do |
|---|---|
| [`nested-search`](tasks/nested-search) | Design and build a library architecture for composable, nested, parallel and exactly reproducible searches, given only how users call it |

The design decisions, the reasons for them, and how the task got harder version by version are in [DESIGN.md](DESIGN.md).

## `nested-search`

`flockwise` is a small PSO library, together with its main user: a drone delivery route planner. The planner needs an outer search over a route's waypoints and, for every candidate route, an inner search over its leg timings, whose limits depend on the waypoints. The library is an earlier, sequential attempt that can't do this. Its searches borrow their parts, and inner searches come from function pointers that can't capture anything.

The agent is given only how users write code against the library: components built from closures, prepared once per search with the position of the particle that search runs for, combined with the library's swarms, wrappers and nested searches. The traits behind them are the agent's to design. The design has to make all of this work at once:

- **Composition:** searches and rules are values that can be built in functions, returned, stored, cloned and sent to threads. Nesting goes to any depth, and rules can be restricted to one field of a position.
- **Context:** every component of an inner search is prepared exactly once for that search's outer particle, before the search starts. Components that ignore the outer position compose anywhere, including the library's own.
- **Inference:** user code never names the library's types. Every composition, including ones the agent has never seen, must infer its types from the arguments, and compositions that don't fit must fail to compile.
- **Borrowed data:** components may capture references to data the caller owns, which is neither `Clone` nor `'static`, and their compositions still return from functions, nest, clone and run on threads. So lifetimes have to flow through every generic type, and the easy ways out (`'static` bounds, boxing, spawned threads) fail.
- **Parallelism with exact reproducibility:** per-particle work runs in parallel on rayon, and a seeded run gives bit-identical results at any thread count.
- **Randomness that belongs to each component:** how much randomness one component draws never changes what any other component receives. Adding a rule that logs particles picked at random leaves the run bit-identical, and so does a seeder that draws more than it needs. A single random stream per particle fails this.
- **A working planner,** which must find feasible routes that score well on scenarios the agent hasn't seen.

The requirements were chosen to interact: the obvious way to meet one breaks another. Borrowing conflicts with storing and threading compositions, and random streams conflict with nesting and parallelism.

This is the problem the library's author worked through in February 2026 (composition and context) and April-May 2026 (parallelism), here without the author's answers.

### How the verifier works

Only the agent's `src/` reaches the verifier, which runs in a fresh container without network. The tests, `Cargo.toml` and the locked files (`api.rs`, the route model) are restored there. The agent's `lib.rs` still decides which files get compiled, so the hidden tests don't rely on the restored files to judge. They compare results field by field, and score routes with their own copy of the model. A hidden test also checks that the library's model gives the same results. The verifier runs:

1. **Tests:** the visible ones plus hidden ones written against the same public API. The hidden tests cover the PSO formula, three levels of nesting, swapped roles and field orders, every combination of components that use or ignore the outer position, user-written rules, recording closures that log every preparation and call, searches that are cloned, reused and run side by side or on other threads, results compared across 1 to 8 threads, the threads that actually do the work, ties between particles, and other route scenarios.
2. **Compile checks:** compositions that don't fit must be rejected by the type system. Each case has a one-line twin that must compile, so a passing check proves the rejection comes from the intended mismatch.
3. **Miri** on a small composition in a 2-thread pool, so that undefined behaviour and data races fail.

The agent's code is treated as untrusted:

- Everything is compiled before any of the agent's code runs.
- The tests run as an unprivileged user that can't write to the repository or the tests. Its leftover processes are killed after every step.
- Miri's isolation keeps the interpreted code away from the file system.
- Root writes the reward last, after deleting any reward file left behind. Harbor mounts the log directory from the host, writable by any user, and it reads `reward.json` before `reward.txt`, so this deletion is what protects the reward.

Each test file has a 3-minute limit, so a deadlock fails instead of stalling. The reward is 1 only if every check passes. The speedup of 8 threads over 1 is measured and reported, but not scored. The container's CPU limit isn't always enforced, so the number varies between machines.

### How it resists reward hacking

No check scans the agent's source code. Each hack is caught by the behaviour it breaks.

| Hack | What catches it |
|---|---|
| Editing tests, adding dependencies, `build.rs` | Only `src/` is taken from the agent |
| Implementing only for the visible compositions | Hidden tests use other position types, roles, field orders and depths |
| Computing answers instead of searching | Hidden objectives differ; recording closures count every score and preparation |
| Erasing types so that everything compiles | Compile checks: compositions that don't fit must fail to compile |
| A best so far that's stale or shared between searches | Independence tests, interleaved clones, reused searches |
| Preparing components too often, too rarely, or for the wrong particle | Recording closures check one preparation per search, for the right outer position, before any use |
| Serializing everything | The threads that score positions and run inner searches are recorded |
| Results that depend on thread scheduling | Bit-identical results across thread counts, in several compositions |
| `'static` bounds or copying instead of borrowing | Compositions over data that is neither `Clone` nor `'static`, returned, nested, cloned and sent to scoped threads |
| One random stream shared by a particle's components | A rule or seeder that draws extra numbers must leave the rest of the run bit-identical |
| `unsafe` with undefined behaviour, data races | Miri |
| Compiling its own copies of the locked files (`#[path]` in `lib.rs`), so the tests' judges agree with anything | The hidden tests judge with their own code: field-by-field equality and their own copy of the route model, which the library's must match |
| Tampering with the verifier from inside a test: rewriting tests not yet compiled, writing `reward.json` | Everything is compiled first; tests run unprivileged and their leftover processes are killed; root deletes stray reward files and writes the reward last |

### Evidence that the verifier works

- **Valid designs are accepted:** besides the reference, a second solution designed independently, with the outer position as a trait parameter instead of an associated type, components prepared into separate values instead of mutated copies, no bounds on its constructors, and hashed random streams, passes every check ([`tools/alternatives`](tools/alternatives)). So do two smaller variants of the reference.
- **Known-bad solutions are rejected:** [`tools/mutants.py`](tools/mutants.py) puts one bug at a time into the reference and runs the verifier. All 24 are caught, among them a stale best so far carried between searches (a real bug in the library's history), components prepared for the wrong particle or too often, preparation skipped on a rerun, one random stream shared by all particles or by one particle's components, the inner seed drawn after the seeder, `'static` bounds on components, serial execution, and a best that's shared between clones behind an `Arc<Mutex>`. Two of them are hacks rather than bugs: a library that compiles its own lenient copy of the route model, or of `api.rs` to make all results compare equal.
- **Correct solutions aren't rejected by bad luck:** [`tools/calibrate.sh`](tools/calibrate.sh) runs the tests over many disjoint seed ranges. A run counts as failed if its test binary exits non-zero for any reason, including a crash or a timeout. The reference fails no run in 1,000 per test file, so each file's false-failure rate is below 0.3% at 95% confidence. The alternative design fails none in 333. These runs are native, with the test harness's default threading, not inside the 2-CPU container. Calibration found false failures in three tests before that. Their thresholds were then set from each test's measured distribution over the same seeds, not moved until the failures stopped ([DESIGN.md](DESIGN.md#calibration)).
- **Harbor end to end:** the reference solution gets reward 1.0, and an agent that does nothing gets 0.0. A solution with a real bug, plus code that rewrites a later test and writes `reward.json` from inside a test, gets 0.0. Under the earlier, unhardened `test.sh` it got 1.0 ([DESIGN.md](DESIGN.md#hardening-the-verifier-2026-10-27)).

### Pass rates

Setup: Claude Code as the agent, run by Harbor 0.25.0 in Docker, with network access only to the model API. One attempt per model and version, so each row is a single sample, not a rate. Cost is at API prices.

| Version | Model | Agent time cap | Result | Agent time | Turns | Cost |
|---|---|---|---|---|---|---|
| v1: implement a given API | Sonnet 5.5 | 1 h | pass | 130 s | 7 | $0.40 |
| v2: more traits, route domain | Sonnet 5.5 | 1 h | pass | 153 s | 6 | $0.42 |
| v3: + reproducible parallelism | Sonnet 5.5 | 1 h | pass | 177 s | 9 | $0.46 |
| v4: agent designs the traits | Sonnet 5.5 | 30 min | pass | 375 s | 13 | $0.78 |
| v4 | Haiku 5.5 | 30 min | pass* | 587 s | 37 | $0.35 |
| v5: + borrowing, per-component randomness | Sonnet 5.5 | 30 min | pass | 383 s | 11 | $0.85 |
| v5 | Haiku 5.5 | 30 min | pass* | 609 s | 22 | $0.37 |

\* Scored 0 in the trial because of unfair checks in the verifier: two in v4, one in v5. The checks were fixed and the agent's saved source passed on re-verification. Each case is described in [DESIGN.md](DESIGN.md#calibration).

What this shows: taking the design away (v4) roughly doubled the effort, but no version came near the time cap. Turn counts are a weak measure: Haiku's drop from 37 to 22 turns came from batching file reads into one shell command, while its time and tokens stayed about the same.

The task is too easy for a training set as it stands. With a small codebase, a precise specification and a local copy of the verifier's checks, both models converge. Every failure the trials did produce was the verifier's fault, and each was caught by a trial. The levers that would raise the difficulty fairly are weaker feedback (no local copy of the checks), a larger codebase to search, and symptoms instead of a specification. Each is a separate task design.

## Running it

With [Harbor](https://harborframework.com) and Docker:

```bash
harbor run -p tasks/nested-search -a oracle      # reference solution: reward 1
harbor run -p tasks/nested-search -a nop         # does nothing: reward 0
harbor run -p tasks/nested-search -a claude-code -m anthropic/claude-sonnet-5-5
```

Without Docker, the tools run the verifier's checks natively (without Miri):

```bash
tools/local_verify.sh tasks/nested-search tasks/nested-search/solution/src
tools/selftest.sh tasks/nested-search /tmp/selftest 1000   # alternatives, mutants, calibration
```

## Provenance

The task and verifier were designed by Andrei Ionescu, from problems in the history of swarmkit, Andrei's own library. Most of the code in this repository was written with Claude Code under that direction. DESIGN.md records the decisions and the evidence behind them.
