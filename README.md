# nestswarm: an RL environment for Rust architecture tasks

Agentic coding tasks in Rust, each with a deterministic verifier, in the [Harbor](https://harborframework.com) task format. The tasks are built from real problems in the history of a particle swarm optimization (PSO) library, [swarmkit](https://github.com/Anvoker/swarmkit). The library is cut down and renamed `nestswarm` for the environment.

| Task | What the agent must do | Status |
|---|---|---|
| [`nested-search`](tasks/nested-search) | Make a PSO library support composable, nested searches, with context passed between levels | Ready |
| Reproducible parallel search | Parallelize the library so seeded runs give identical results at any thread count | Planned |

The design decisions, and the reasons for them, are in [DESIGN.md](DESIGN.md).

## `nested-search`

The library's public traits describe a design where searches are built from interchangeable parts: a mover can run a whole inner search for every particle it moves, movers can be restricted to one part of a particle, and every component of an inner search sees the outer particle it runs for. The implementation is an earlier attempt that doesn't support this. Its searchers borrow their parts, and inner searches come from function pointers that can't capture anything. The agent has to make the design work.

This is what the original author faced in February 2026: the design was clear, and the compiler wouldn't accept it. Getting there takes:

- owning components instead of borrowing them, so searches can be nested, stored, cloned and sent to threads;
- threading context through wrappers and nesting levels, to every component that may use it;
- APIs where every type is inferred from the arguments, because user code may not name the library's internal type parameters;
- fixing two real bugs from the original code, which the visible tests expose.

### How the verifier works

Only the agent's `src/` reaches the verifier, which runs in a fresh container. The locked API file, the tests and `Cargo.toml` are restored from the template there. The verifier runs:

1. **Tests:** the visible ones plus hidden ones in the same vocabulary. The hidden tests cover three levels of nesting, swapped roles and part orders, components written outside the library, recording components that log every context they receive and count their calls, cloned searchers run side by side, and searchers moved to other threads.
2. **Compile checks:** mismatched compositions must be rejected by the type system. Each case has a one-line twin that must compile, so a passing check proves the rejection comes from the intended mismatch.
3. **Miri,** on a small composition, so that `unsafe` shortcuts with undefined behaviour fail.

The reward is 1 only if every check passes.

### How it resists reward hacking

No check scans the agent's source code. Each hack is caught by the behaviour it breaks.

| Hack | What catches it |
|---|---|
| Editing tests, adding dependencies, `build.rs` | Only `src/` is taken from the agent |
| Implementing only for the visible types or arrangement | Hidden tests use other unit types, roles, part orders and depths |
| Computing answers instead of searching | Hidden objectives differ; recording components count every evaluation |
| Erasing types so that everything compiles | Compile checks: mismatched compositions must fail to compile |
| A global best that's stale or shared (no reset, shallow `Clone`, `Arc`) | Independence tests, interleaved clones |
| Context delivered to some components and not others, or too late | Recording components check every call's context |
| Sharing through `Rc<RefCell>` | Composed searchers must be `Send` |
| `unsafe` that breaks Rust's aliasing rules | Miri |
| A PSO that converges, but not by the stated formula | One update is checked exactly, using an RNG whose draws are fixed |

### Evidence that the verifier works

- **Known-bad solutions are rejected:** [`tools/mutants.py`](tools/mutants.py) puts one bug at a time into the reference solution and runs the verifier. All 13 mutants are rejected. They include a real bug from the original library, a stale global best, which was in it from 2026-02-06 to 2026-04-22. A 14th mutant, an aliasing violation, passes every normal test and is rejected by Miri.
- **The reference isn't rejected by bad luck:** [`tools/calibrate.sh`](tools/calibrate.sh) runs the reference's tests over 1,000 disjoint seed ranges. Every test file had 0 failures, which puts the false-failure rate under 0.3% per file at 95% confidence.
- **Harbor end to end:** the reference solution gets reward 1.0, and an agent that does nothing gets 0.0.

### Pass rates

Not measured yet. Pass rates will be reported together with the setup they were measured with: model, agent harness, limits and number of attempts.

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
python -I tools/mutants.py tasks/nested-search /tmp/mutants
tools/calibrate.sh tasks/nested-search 1000
```

## Provenance

The tasks and verifiers were designed by Andrei Ionescu, from problems in the history of swarmkit, Andrei's own library. Most of the code in this repository was written with Claude Code under that direction. DESIGN.md records the decisions and the evidence behind them.
