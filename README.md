# flockwise: an RL environment for Rust architecture tasks

A PSO library design task I created for my own education on building RL environments. Solvable by Haiku.

Rust, a deterministic verifier, [Harbor](https://harborframework.com) format. Adapted from my experiences with [swarmkit](https://github.com/Anvoker/swarmkit), my own PSO library.

## The task

In [`nested-search`](tasks/nested-search), `flockwise` is a small particle swarm optimization library with one user, a drone delivery route planner. The planner needs nested searches: an outer search over a route's waypoints and, for every candidate route, an inner search over its leg timings, whose limits depend on the waypoints. The library as given runs on a single thread, and it can't nest searches: its searches borrow their parts, and inner searches are function pointers that can't capture anything.

The agent gets the whole repository, which doesn't compile. The instruction specifies only how users write code against the library. A search is assembled from components (an objective, a region, a movement rule and a seeder), each built from a closure that's prepared once per search with the outer particle's position, and combined with swarms, wrappers and nested searches. The internals, including the traits behind all of this, are the agent's to design.

What makes it hard is that the requirements fight each other. Components may borrow data the caller owns, yet compositions must still be stored, cloned and sent to threads. A seeded run must be bit-identical at any thread count, and each component's randomness must stay its own, even inside nested searches. And user code never names the library's types, so all of it has to work through type inference. I worked through the same problems in swarmkit over several months; the agent gets them without my answers.

## Results

Claude Code as the agent, run by Harbor 0.25.0 in Docker, with network access only to the model API and a 30-minute cap. One attempt per model on 2026-10-29, so these are single samples, not rates. Cost is what the tokens would cost at API prices; the trials ran on a Claude subscription.

| Model | Result | Agent time | Turns | Cost |
|---|---|---|---|---|
| Sonnet 5.5 | pass | 418 s | 15 | $0.97 |
| Haiku 5.5 | pass | 949 s | 22 | $0.33 |

Both pass well inside the cap, so as a training task it's too easy. With a small codebase, a precise specification and a local copy of the verifier's checks, both models converge. Making it harder fairly would mean weaker feedback, a larger codebase to search, or symptoms instead of a specification, and each of those is a different task.

## How it's graded

Only the agent's `src/` survives. The verifier rebuilds the repository around it in a fresh container without network, then runs the visible tests, hidden tests written against the same API, compile checks that mismatched compositions must fail, and Miri. The reward is 1 only if everything passes. The hidden tests use closures that record every call, compare runs at different thread counts bit for bit, and judge results with their own code. [DESIGN.md](DESIGN.md#verifier-coverage-v5) maps each requirement to the checks behind it.

No check reads the agent's source. Each hack is caught by the behaviour it breaks:

| Hack | What catches it |
|---|---|
| Editing tests, adding dependencies, `build.rs` | Only `src/` is taken from the agent |
| Fitting only the visible tests | Hidden tests use other position types, roles, field orders, depths, objectives and scenarios |
| Hardcoding answers instead of searching | Hidden objectives; recording closures count every score |
| Erasing types so everything compiles | Compile checks |
| Running serially to get reproducibility for free | The threads that do the work are recorded |
| `'static` bounds or copying instead of borrowing | Compositions over data that is neither `Clone` nor `'static` |
| Compiling its own lenient copies of the locked files (`#[path]`) | Hidden tests judge with their own code |
| Tampering from inside a test: rewriting later tests, writing `reward.json` | Compile first, run unprivileged, kill leftovers, root writes the reward last |

To check the checks, [`tools/mutants.py`](tools/mutants.py) puts one bug or hack at a time into the reference solution, and all 27 fail. Three other valid designs pass, two of them Haiku's and Sonnet's own unedited solutions. The reference fails none of 1,000 seeded runs per test file.

## Difficulty progression

The task started much easier. Version 1 handed the agent my API and asked for an implementation. Over five versions I took the design away and added requirements that interact. Same setup as above, one attempt each:

| Version | Model | Cap | Result | Agent time | Turns | Cost |
|---|---|---|---|---|---|---|
| v1: implement a given API | Sonnet 5.5 | 1 h | pass | 130 s | 7 | $0.40 |
| v2: more traits, route domain | Sonnet 5.5 | 1 h | pass | 153 s | 6 | $0.42 |
| v3: + reproducible parallelism | Sonnet 5.5 | 1 h | pass | 177 s | 9 | $0.46 |
| v4: agent designs the traits | Sonnet 5.5 | 30 min | pass | 375 s | 13 | $0.78 |
| v4 | Haiku 5.5 | 30 min | 0, pass on re-verification | 587 s | 37 | $0.35 |
| v5: + borrowing, per-component randomness | Sonnet 5.5 | 30 min | pass | 383 s | 11 | $0.85 |
| v5 | Haiku 5.5 | 30 min | 0, pass on re-verification | 609 s | 22 | $0.37 |

"0, pass on re-verification": the trial scored 0 because of an unfair check in the verifier. Once the check was fixed, the agent's saved solution passed ([DESIGN.md](DESIGN.md#calibration)).

Taking the design away (v4) roughly doubled the effort, but no model ever came near the time limit. The v5 rows predate the verifier's hardening; the results above are a re-run of v5 against the current verifier.

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

I designed the task and the verifier. Most of the code in this repository was written with Claude Code under my direction.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT), at your option.
