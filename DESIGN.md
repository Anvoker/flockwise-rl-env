# Design

This file records the design decisions behind the environment and the reasons for them. The code in this repository is mostly written by Claude Code under the author's direction. The task design and the decisions recorded here are the author's.

## The environment

One Rust codebase: a small particle swarm optimization (PSO) library, cut down from an early version of [swarmkit](https://github.com/Anvoker/swarmkit). Two tasks are built on it, and both come from real problems in that library's history:

| Task | Real history | What the agent must do |
|---|---|---|
| 1. Composable nested search | 2026-02-03 to 2026-02-07 | Make a composable searcher architecture compile and work: nested searches, movers on parts of a particle, context passed from the outer search to the inner one. |
| 2. Reproducible parallel search | 2026-04-23 to 2026-05-08 | Parallelize the library so that seeded runs give identical results at any thread count. To be designed after task 1 ships. |

Task 1's reference solution becomes task 2's starting state, as it did in the real history.

No difficulty knobs in the first version: each task ships in one setting.

## Task 1: composable nested search

### Where it comes from

On 2026-02-03 the author committed a design for running a PSO inside a PSO. Sailing routes were optimized in space by an outer search, and each candidate route's timing was optimized by an inner search. The design didn't compile. It took until 2026-02-06 to compile and 2026-02-07 to work fully, with Claude Code fixing compile errors and type inference on the author's direction. The design calls were the author's: a factory for inner searches, keying the part-projection trait on its output type, owning values instead of borrowing them, and passing context to inner components.

The task starts from the 2026-02-03 state, with the sailing domain replaced (see [Domain](#domain)).

### What the change makes possible

Each capability below was impossible in the starting state. Verification is built from this list: each one is proven by code that compiles and runs, not by inspecting the agent's source.

| # | Capability | Why it fails in the starting state |
|---|---|---|
| C1 | A searcher is a value: it can be built in a function, returned, stored and moved into another component. | Searchers hold `&'a mut` borrows of their global best and fitness function, so a searcher built from locals can't leave the function. |
| C2 | A searcher can run inside a mover: one full inner search per outer particle. | The inner search comes from a `fn() -> SearcherIter<'a, …>` pointer. It can't capture state or receive the particle, and the iterator borrows locals the factory would have to create. |
| C3 | Nesting recurses: three levels work. | Follows from C1 and C2. |
| C4 | Mix and match on parts: a mover works on one part of the unit and sees the matching part of the shared state, in either role. | The two-mover searcher splits the particle but passes the whole global best, so the types don't line up. |
| C5 | The inner fitness function sees the current outer particle. | Nothing exists for it. |
| C6 | Context reaches more than one inner component: the inner boundary depends on the outer particle too. | Same as C5. This is the deepest part of the original problem (trait coherence when several components may or may not take context). |
| C7 | Boundaries compose: any mover can be wrapped with any boundary, including movers defined outside the library. | The boundary is built into the PSO mover as a reference. |

### Domain

The sailing model (about 800 lines) is replaced by a synthetic objective with a closed-form optimum. The *shape* of the original stays: a unit made of two parts, like `Path = (xy, t)`, so the part machinery still matters.

Whole unit `(x, t)` with `x, t ∈ Rᵏ`:

- fitness `f(x, t) = -Σ(xᵢ - aᵢ)² - Σ(tᵢ - (xᵢ + cᵢ))²`
- the inner boundary depends on the outer particle: `tᵢ ∈ [xᵢ - w, xᵢ + w]`

For a given `x`, the inner optimum is `tᵢ* = xᵢ + clamp(cᵢ, -w, w)`. The penalty left over doesn't depend on `x`, so the joint optimum is `x* = a`, `tᵢ* = aᵢ + clamp(cᵢ, -w, w)`, in closed form.

Choosing some `|cᵢ| > w` makes the boundary active. Then each failure has its own signature:

- **Inner fitness ignores context:** `t` converges towards a target computed for the wrong `x`.
- **Inner boundary ignores context:** `t` ends up outside `[xᵢ - w, xᵢ + w]` for the real `x`.
- **Inner search skipped or replaced:** see the evaluation counts below.

Hidden tests use different `a`, `c`, `w` and `k`, so solving the visible objective analytically gains nothing.

Particle storage changes from the `soa_rs` crate to a plain `Vec` with a hand-written mutable particle view. The real library made the same move on 2026-05-07. The reasons here: one fewer dependency, and unit types defined by the tests stay easy to write.

### What's locked and what's free

Hidden tests have to compile against the agent's library without knowing its design. So the task fixes a **public vocabulary**, and nothing else:

- **Traits that users implement are locked in full.** The hidden tests implement them for their own types. They live in `src/api.rs`, which is restored from the template before verification. Provisional list:
  - `Unit`: arithmetic for a particle's position type, plus `map_scalars` (see below).
  - `FitCalc`: the fitness function.
  - `Boundary`: maps a position back into the feasible region.
  - `PartOf<W>`: one trait saying where a part lives inside its whole (`get` and `get_mut`). The library builds particle views, projected global bests and write-back from it.
  - `Contextful`: a supertrait of `FitCalc`, `Boundary` and `Mover`, with a `TContext` type and `fn set_context(&mut self, context)`. Every component accepts context, and components that don't need it ignore it.
  - `Mover`: updates one particle (needed for C7).
  - The dependent initializer: creates inner particles from an outer one.

  The locked traits are the maintainer's spec, so they're the version a maintainer would settle on. The starting state is the 2026-02-03 implementation with these public traits already in place. The agent's work is the implementation, which is where the original difficulty was.

  **Why one part trait** (decided 2026-10-26): the 2026-02-03 code had only a particle-view trait. The projection and write-back traits were added during the fix, on 2026-02-05 and 2026-02-06, so locking all three would hand over two pieces of the solution. One accessor trait is also the better public contract: users write two one-line functions per part, the view and the projection can't disagree, and new topologies cost users nothing. The cost is that parts must be stored values inside the whole, which a PSO over parts doesn't need to go beyond.

  **Why every component takes context** (decided 2026-10-26): forwarding context only to components that implement a trait would need specialization, which is unstable. The marker-type approach the original code tried on 2026-02-07 is fragile, and under the annotation rule below some compositions might have no valid solution at all. The library's own design converged on "every component is `Contextful`" by 2026-05-04, so it's known to work in stable Rust. The agent still has to thread context through owned searchers, adapters and bounded movers, and through the nested mover, which receives one context type and supplies another.
- **Names that users call are locked by name and call shape.** These are the constructors and run methods that appear in the visible usage (searchers, the bounded mover, the part adapter, the nested mover, the search iterator). The agent may change their generics, bounds, ownership and internals, and may add methods.
- **Everything else is free:** internal traits, type parameters, how context is threaded through, how coherence problems are solved.

**Requirement on type annotations.** User code may name only public types and its own types. It never names the library's internal type parameters, marker types or helper traits. Naming *domain* types is allowed (for example, which part a mover works on), because that's information only the user has. This isn't a style rule. It follows from "users only know the public API": the hidden tests are written before the agent's design exists, so they *can't* annotate its internals. Two cases where annotating is impossible, not just verbose:
- searchers returned from helper functions as `impl Searcher<…>`, then nested, where only `_` can stand for their type;
- fitness functions and boundaries defined by the tests, which the library has never seen.

The historical working version still needed `NestedMover::<N, Path<N>, Time<N>, _, _, _>::new(…)` at one call site. The reference solution must remove that.

**Fairness rule for hidden tests:** each one uses only names and patterns from the visible usage, and the reference solution passes it.

### Verification

Verification runs in a clean tree: the template repository plus the agent's `src/` only, with `src/api.rs`, `tests/`, `examples/` and `Cargo.toml` restored from the template. So edits to tests, added dependencies, a `build.rs` or a `.cargo/config` from the agent never reach the verifier. Dependencies are vendored in the image, and there's no network.

Checks, by capability:

| Check | Kind | Proves |
|---|---|---|
| V1. A helper builds a full composition from locals and returns it as `impl Searcher<…>`. | compile | C1 |
| V2. Two-level composition (PSO on `x`, nested search on `t`) converges to `(x*, t*)` within tolerance. | run | C2, C4, C5, C6 |
| V3. Three-level composition (`x`, then `y`, then `t`) compiles and converges. | compile + run | C3 |
| V4. Roles swapped: PSO on `t` outside, nested search on `x` inside. | compile + run | C4 |
| V5. Fitness, boundary and mover defined by the test, composed with library parts. | compile + run | C7, C5 |
| V6. A test-defined inner fitness records each context it receives and counts evaluations. Context must equal the current outer particle, and the counts must match the stated particle and iteration numbers. | run | C5, C2, and that the inner search really runs |
| V7. Independence: outer particle A's context can reach a much higher fitness than B's. B's inner result must lie within B's boundary and near B's optimum. | run | each inner search starts fresh |
| V8. A composed searcher is cloned. The two copies run on different objectives and seeds, and neither leaks into the other. | run | independent copies |
| V9. Composed searchers are `Send`. | compile | no `Rc`/`RefCell` sharing (prepares task 2) |
| V10. Mismatched compositions must not compile, e.g. a mover for the `t` part placed on `x`, or an inner component whose context type isn't the outer unit. Each case has a one-line twin that must compile (see below). | compile-fail | the type safety survives; no erasure to `Vec<f64>` or `dyn Any` |
| V11. Every combination of context-taking and context-ignoring components in an inner searcher compiles without annotations: fitness, boundary and mover each in either role, including all of them ignoring it. | compile | context forwarding works for every mix, not only the visible one |
| V12. Recording components (fitness, boundary and a test-defined mover), placed at different depths inside wrappers. Each logs the context active at every call. For every inner search, all calls must see the outer particle that search runs for: set before the search starts, unchanged until it ends. In three-level compositions, each level sees the particle of the level directly above. | run | context reaches every component that takes it, at the right time |
| V13. One small composition runs under Miri. | run (Miri) | no undefined behaviour, e.g. a lifetime extended with `unsafe` to escape the borrow checker |
| V14. One PSO update on a hand-made particle: at both bests only inertia remains (exactly), and with equal pulls in every coordinate the new velocity's coordinates differ. Visible, plus a hidden variant on a two-part unit and through a part adapter. | run | the stated PSO formula, deterministically (convergence tests only notice a wrong formula by luck) |

**Context as a functional requirement.** The requirement is: every component of an inner search gets the outer particle that search runs for, before the search begins, and keeps it until the search ends. Components that ignore context compose the same way. V11 proves the "compose the same way" part at compile time. V12 proves delivery and timing at run time. The numeric checks (V2-V5, V7) prove the effect on results.

**Miri** is an interpreter for Rust's intermediate representation that runs tests while checking every operation for undefined behaviour: use after free, out-of-bounds access, broken aliasing rules, uninitialized reads, data races. It catches `unsafe` that actually misbehaves without banning `unsafe` that's sound, so it fits the no-source-scans rule. It needs a pinned nightly toolchain, with the Miri sysroot built when the image is built (no network later), and it's slow, so it runs on one small composition only.

**No source scans.** No check greps the agent's code for `unsafe`, `Mutex`, `RefCell` or anything else. Each hack must be caught by the behaviour it breaks. `Arc<Mutex<…>>` used carefully inside one searcher, with a `Clone` that deep-copies, passes, and should.

**Warm starts are allowed.** Seeding an inner search with positions from an earlier one is legitimate. Carrying over a *fitness score* computed under a different context is the bug V7 catches. The real library had exactly this bug from 2026-02-06 until 2026-04-22: one inner searcher was reused for every outer particle, and its global best was never reset. The prompt states that each inner search optimizes for the current outer particle.

**Stochastic tests.** All RNGs are passed in by the tests and seeded. Each numeric check runs over several seeds. Before shipping, the reference solution is run about 1,000 times per check to measure its false-failure rate, and thresholds are set from that measurement, not by guess.

### Reward hacks and what blocks them

| Hack | Blocked by |
|---|---|
| Edit or delete tests | Verifier restores tests from the template |
| Add dependencies, `build.rs`, `.cargo/config` | Only `src/` is taken from the agent; `Cargo.toml` is restored |
| Implement only for the visible types | V3, V4, V5: other unit types, roles and components |
| Compute the answer instead of searching | Hidden objectives differ; V6 counts inner evaluations |
| Erase types to make everything compile | V10 compile-fail tests |
| Share state through `Rc<RefCell>` | V9 |
| Shared or stale global best (no reset, shallow `Clone`, shared `Arc`) | V7, V8 |
| Context reaches the fitness but not the boundary | Domain: active boundary, checked numerically |
| Require annotations of internals at call sites | Hidden call sites are written against the public vocabulary only |

### Presentation

The instruction is part of the difficulty. Choices:

| Choice | Setting | Why |
|---|---|---|
| How the problem is described | Design intent, no solution hints | Symptom-only ("doesn't compile") is vague; hints give the design away. |
| Visible usage | A few examples, one per category, including a role swap and a contextful boundary | Generality is clearly required without showing the hidden tests. |
| Hidden tests | The prompt says other compositions are tested and lists the categories (deeper nesting, swapped roles, components defined outside the library, cloned searchers), without code | Otherwise a failure comes from vagueness, not difficulty. |
| Type annotations | None of library internals at call sites | Inference was a real part of the original difficulty, and annotations would pin the agent's internal design. |
| Starting code | The real 2026-02-03 attempt, sailing replaced | The `fn`-pointer factory and the `&'a mut` fields are realistic dead ends. |
| What may change | Stated exactly: everything in `src/` except `src/api.rs`; tests, examples and `Cargo.toml` are read-only | No guessing about the rules. |
| The checks themselves | The repository has a `ci.sh` that runs what the verifier runs, on the visible tests: the test suite, the compile-fail tests and the Miri check. The prompt says the verifier runs the same steps on more tests. Nightly and Miri are installed in the image so the agent can run it. | A human contributor would see the CI. Knowing that Miri runs only discourages undefined behaviour, which is the goal. The categories are shared, the hidden tests aren't. |

The instruction states the requirements as capabilities (C1-C7 in user terms), the public vocabulary, the particle and iteration counts that V6 relies on, and that each inner search optimizes for the current outer particle.

It must **not** mention: owning instead of borrowing, how context is threaded, marker types, resetting the global best, or any trait the reference solution adds internally.

### Reference solution

Adapted from the library's own history after 2026-02-06 (owned searchers, the nested mover, context threading), then extended where the history fell short: no annotations of internals at call sites, and fresh inner searches. It must pass every check, and its false-failure rate is measured as described above.

### Decisions made while building (2026-10-26)

- **Crate:** `nestswarm`, depending only on `rand` 0.9.2. The name is deliberately not swarmkit's.
- **Naming a part at a call site:** `mover.on_part::<X>()`, from an `OnPart` extension trait. A turbofish on a struct (`PartMover::<X, _, _>`) would force `_` placeholders for internal type parameters, which the annotation rule forbids. A method with one generic parameter names only the domain type. The library's own history had chained methods like this by 2026-04-30.
- **Compile checks without `trybuild`:** `trybuild` compares exact compiler messages, which change with the agent's design. Instead, each case is a file with a `//CASE` line and a `//FAIL` line. The `CASE` form must compile and the `FAIL` form must not. Because the two forms differ in one line, a passing check proves the rejection comes from the intended mismatch and not from something unrelated.
- **The starting state keeps two real bugs from 2026-02-03:** the PSO mover sets the velocity to minus the step it just took, and `SearcherIter` runs one iteration too few. The instruction states the PSO formula and the iteration count, so both are deviations from a stated spec, not hidden traps. They add a small bug-fixing part to a mostly architectural task.
- **Mover order and convergence:** with the nested mover *first* in a two-mover searcher, the inner result is computed for the outer particle's position before the outer mover moves it, so every evaluated pair holds a slightly stale inner part. The outer search then settles away from the optimum. The reference behaves the same, so with the nested mover first the test asserts a calibrated bound instead of the optimum: over 900 runs of the reference, the final fitness gap was at most 0.37 and the starting gap at least 5.4, so a bound of 1.0 still rejects a search that doesn't work. Context delivery is checked for both orders.
- **Calibration: v1 was too easy (2026-10-26).** Sonnet 5.5 solved it in 7 turns. Diagnosis: the locked API carried the author's February design decisions, and the instruction read like a spec of the solution.
- **"Give the design decision back" is impossible (tested 2026-10-26).** The idea was to let components that don't need context implement nothing, and make the library deliver context only to components that implement `Contextful`. In stable Rust that needs either specialization (unstable) or a marker type the compiler infers. A minimal test of the marker approach fails with E0283 ("type annotations needed"): for a component that takes context, both the "forward" and the "ignore" impl apply. The only fix is naming the marker at the call site, which the annotation rule forbids. This reproduces the author's own conclusion from 2026-02-07. The lesson: a verifier with hidden tests needs a fixed public API, so the task author has to make the API's design decisions. Design freedom and checkable composability pull in opposite directions.
- **Instruction cut back to intent (2026-10-26):** the step-by-step description of the nested mover, the PSO formula and the iteration count were removed. Each removed behaviour still follows from a stated requirement or a visible test (`tests/pso_update.rs` pins the formula; `tests/single.rs` pins the count). Kept: exact evaluation counts and the evaluation order ("at the position the particle had when the iteration began"), because a valid PSO may legitimately differ there, and the randomness and clone rule. The evaluation order was a gap even in v1: two checks assumed it without the instruction saying so.
- **Knob for later versions:** the two bugs kept in the starting state can be removed for an easier variant, or more real bugs added for a harder one. Not used in v1.
- **Per-coordinate randomness, so `Unit` gained `map_scalars`:** calibration showed the reference failing up to 21% of seeds on one test. The cause was particles stuck on the face of a box. With one random factor per particle (all that `Mul<f64>` allows), a swarm collapses into a lower-dimensional subspace and can't leave a face. Measured on one inner search over 2,000 seeds: 73 failures with one factor per particle, 2 with one per coordinate. Setting the velocity to the step actually taken barely helped (61), and reflecting boundaries instead of clamping made it much worse (1,317). So the locked `Unit` trait has one more method, `map_scalars`, which applies a function to every coordinate. The library's own history had a similar trait for the same reason by 2026-05-04 (`Kickable`, which exposes a unit's scalar slots). The instruction states that `r1` and `r2` are drawn per coordinate.
- **Test boxes:** boxes that aren't meant to bind are much wider than the region where particles start, so particles rarely reach their faces. The inner boxes, which bind on purpose, keep clamping, since the optimum lies on their faces in some dimensions.
- **Agent network:** while the agent runs, it can reach only the model APIs (`[agent] network_mode = "allowlist"`). The public swarmkit repository contains a solution, so open network access would let an agent look it up. Setup keeps public network, because Harbor installs the agent's harness then (for Claude Code: apt packages and a download from claude.ai). Checked with a probe run: the install succeeds, the API is reachable, and Harbor turns off the agent's web search on its own.
- **Verifier isolation in Harbor:** the verifier runs in a fresh copy of the environment (`environment_mode = "separate"`), with `/app/src` as the only artifact carried over, and restores `src/api.rs` from a pristine copy.

### Testing the verifier

`tools/mutants.py` takes the reference solution, puts in one known bug at a time, and runs the full verifier. Every mutant must fail at least one check. The mutants include the real stale-global-best bug, context never set, set too late, or not forwarded to boundaries or movers, both bugs left in the starting state, one random factor per particle instead of per coordinate, a part adapter shown the wrong best, a nested mover that never writes back, ignoring the given RNG, and a global best shared through `Arc<Mutex>` or `Rc<RefCell>`.

`tools/calibrate.sh` runs the reference's tests over many disjoint seed ranges and counts how often each one fails by bad luck. Budgets and thresholds were adjusted until every test file had 0 failures in 1,000 runs (the independence test in 3,000). By the rule of three, that puts each file's false-failure rate below 0.3% with 95% confidence.

### Build risks

- The reference solution is built before the tests are final. If it can't satisfy a locked call site without annotations of internals, the call site changes, not the rule.
- Miri must work offline in the image: install the nightly with `rust-src` and run `cargo miri setup` while the image is built. Also set `MIRI_SYSROOT`, or cargo-miri tries to rebuild the sysroot in every container and fails over the image layer.
- Rust doesn't guarantee the precision of float functions like `powi`, and Miri varies it on purpose. Tests that compare results exactly must use only operations IEEE 754 makes exact (`d * d`, not `powi(2)`). Found when the clone-equality check failed under Miri only.

## Calibration

### Log

| Date | Task | Model | Harness | Budget | Attempts | Result |
|---|---|---|---|---|---|---|
| 2026-10-26 | nested-search v1 | Claude Sonnet 5.5 (subscription) | Claude Code via Harbor 0.25.0 | 1 h agent time, network limited to the model API | 1 | Pass: 7 turns, 130 s agent time, about $0.40 at API prices. A genuine solution, close to the reference's design, with no hacks. |

One attempt isn't a pass rate, but it shows v1 is far too easy for current mid-size models: the solve took no exploration and one fix.

### Plan

To be fixed before any runs: the models (at least two sizes, including DeepSeek through an agent harness and Claude through Claude Code headless), the harness, the turn, token and time caps, and the number of attempts per model. Pass rates are reported only together with that setup. Cost is estimated before spending.

## Format

Harbor / Terminal-Bench style: a Dockerfile, an instruction, tests and a reference solution per task. The current spec gets checked before building.
