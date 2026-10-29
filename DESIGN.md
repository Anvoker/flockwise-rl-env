# Design

This file records the design decisions behind the environment and the reasons for them. The code in this repository is mostly written by Claude Code under the author's direction. The task design and the decisions recorded here are the author's.

## The environment

One Rust codebase: a small particle swarm optimization (PSO) library, cut down from an early version of [swarmkit](https://github.com/Anvoker/swarmkit). It started as two planned tasks, both from real problems in that library's history: composable nested search (2026-02-03 to 2026-02-07) and reproducible parallel search (2026-04-23 to 2026-05-08). Version 3 merged the second into the first, so the environment ships one task, `nested-search`, in one setting.

**How to read this file.** It's a log, in the order decisions were made. The sections from [What the change makes possible](#what-the-change-makes-possible) to [Testing the verifier](#testing-the-verifier) record version 1's design and use its names (`nestswarm`, `Unit`, `FitCalc`, `Contextful`, `examples/`, `tests/pso_update.rs`). The version sections say what changed after that. The current task is in [`tasks/nested-search`](tasks/nested-search), and the [README](README.md) describes it.

## Task 1: composable nested search

### Where it comes from

On 2026-02-03 the author committed a design for running a PSO inside a PSO. Sailing routes were optimized in space by an outer search, and each candidate route's timing was optimized by an inner search. The design didn't compile. It took until 2026-02-06 to compile and 2026-02-07 to work fully, with Claude Code fixing compile errors and type inference on the author's direction. The design calls were the author's: a factory for inner searches, keying the part-projection trait on its output type, owning values instead of borrowing them, and passing context to inner components.

The task starts from the 2026-02-03 state, with the sailing domain replaced (see [Domain](#domain)).

### Version 2 (2026-10-26)

Version 1 turned out far too easy: Claude Sonnet 5.5 solved it in 7 turns (see [Calibration](#calibration)). Version 2 changes these things. The sections below still describe the design in general and use v1's names where they discuss v1.

- **New vocabulary.** swarmkit is public, including on crates.io, so v1's names (`GBestSearcherOne`, `NestedMover`, `Contextful`, ...) were cues to a published solution. Every name is now plain and new: crate `flockwise`, `Point`, `OuterAware`/`see_outer`, `Objective`/`score`, `Region`/`confine`, `Rule`/`apply`, `Seeder`, `BestSoFar`, `ParticleView`, `PsoParams`, `PsoRule`, `Swarm1`, `Swarm2`, `Confined`, `ForField`/`for_field`, `InnerSearch`, `Search`, `Run`.
- **Three field traits instead of one.** `SplitFrom` (a view of one field of a particle), `ReadFrom` (read a field out of a whole position) and `WriteInto` (write a field back), each with its own `Whole`. That is the shape of the original code's three traits. The library has to tie them together itself, which brings back the inference work of 2026-02-05. With one trait, the library could derive all three operations from a single accessor.
- **A real user of the library: drone delivery routes** (`src/routes/`). An outer search over waypoints, and for every candidate route an inner search over leg timings. Each leg's fastest possible duration depends on the wind where and when it's flown, so it depends on the waypoints and on how long the earlier legs took. Its region therefore has to be shown the outer route and confine legs in order. That's the structure of the original sailing code, written fresh, with its own physics and names. The model (`src/routes/model.rs`) is locked like `api.rs`, so that the routes have one definition of feasible and good. (Since 2026-10-27 the hidden tests score routes with their own copy of it; see [Hardening the verifier](#hardening-the-verifier-2026-10-27).) The planner (`src/routes/plan.rs`) is agent code. In the starting state it's written against the old design, function-pointer inner search included, so the library doesn't compile, as at `5e45776`, where all 15 errors were in the sailing module.
- **Fewer visible examples.** The role-swap test is hidden now. Visible: single search, the PSO update, two-level nesting, routes, and the Miri composition.
- **Offline docs.** The standard library's docs (the `rust-docs` component) and the dependencies' docs (`cargo doc`) are in the image. docs.rs stays unreachable, because it hosts swarmkit's docs.
- **The instruction states intent,** as described under the decisions below.

### Version 3: task 2 merged in (2026-10-26)

Version 2 was solved just as fast as v1 (6 turns). As agreed beforehand, task 2, reproducible parallelism, was merged into this task. It comes from the same library's April-May 2026 history: rayon, one random stream per particle derived from a master seed, and a serial reduction into the best so far.

- **New requirements:** searches do their per-particle work (scoring, applying rules, whole inner searches) in parallel on the current rayon pool. Given the same seed, results are bit-for-bit identical whatever the number of threads.
- **Locked API changes, because parallelism needs them:** a rule takes `&self` and gets its randomness from an `rng` argument that belongs to the particle being moved. Every objective, region, rule and seeder is `Send + Sync`. `Run::new` takes the seed, and `PsoRule::new` and `InnerSearch::new` no longer take RNGs. These are the only shapes the agent is given. How random streams are derived, and how inner searches get their own state on each thread, are left to it.
- **Order within an iteration:** score every particle, then update the bests in particle order (the earlier particle wins a tie), then apply the rules to every particle. This was already one of v2's accepted alternatives. Now it's the specification, because applying rules while the best so far is still changing can't be made independent of thread scheduling.
- **The starting state stays the February code,** ported to compile against the new traits. Where the old code needs randomness it uses the thread RNG, as the original did (`ThreadRng::default()`). That's itself a reproducibility bug to find.
- **Checks:** visible `parallel.rs`: the nested composition and `routes::plan` give identical results in pools of 1, 2, 4 and 8 threads. Hidden `h_parallel.rs`: another composition and a user-written rule across thread counts; more than one thread must score outer positions and run inner searches in an 8-thread pool; a flat objective, where the first particle must win the tie at every thread count. Miri now also catches data races.
- **Parallelism is proven by observed threads, not by timing** (agreed): recording components note which threads called them. The speedup of 8 threads over 1 is measured on the route planner and written to `results.json`, without counting toward the reward. On a 32-core machine the reference shows about 6.4×. A first version of the measurement used the recording components and showed 0.29×, because every score took a global lock: a measurement problem, not a library one.

**Calibrating the routes checks.** In the first version of the model, drag was so small that flying every leg as fast as possible was optimal, so the inner timing search had nothing to do (the same median score with and without it). Raising drag to 0.2 puts the optimal airspeed at roughly `((1 + w·hover) / (2·w·drag))^(1/3)`, inside the limits. Over 300 runs per scenario, the planner's results also turned out bimodal in one scenario: some runs go around a zone on the worse side. So each scenario checks what it can tell apart, using the best of 3 runs:

| Scenario | Reference, worst best-of-3 | Same waypoints, fastest timings | Threshold | What it checks |
|---|---|---|---|---|
| B (visible) | −71.3 | about −190 | −80 | timing quality |
| C (hidden) | −9.85 | −11.96 | −11.0 | timing quality under other wind and weights |
| A (hidden) | −25.4 (−25.6 over 1,000 runs, 2026-10-27) | overlaps | −35 | routing around zones |

Every run must also return a feasible route, within a tolerance of 0.001. A stricter tolerance (0.000001) caused false failures: the objective penalizes zone intrusion at 1,000 per unit, so cutting a corner by 0.0001 costs only 0.1 points and saves distance, and the optimizer takes it. The largest cut seen in 6,000 plans was about 0.00012. The timing threshold in C was also moved from −10.5 to −11.0 after one false failure in 1,000 runs, which still leaves it about 1 point above the fastest-timings mode.

### Version 4: the agent designs the architecture (2026-10-26)

Version 3 was solved just as easily (9 turns). Every version so far handed the agent the design: `api.rs` held the author's traits (`OuterAware`, `Objective`, `Rule`, ...), and the task was to implement them. In February the hard part was finding that design, not writing it. Version 4 takes the design away and fixes only what a user of the library writes.

**What's fixed.**

- **The call sites.** Components are built from closures: `objective(f)`, `region(f)`, `rule(f)`, `seeder(f)`. Each closure is given the *outer position* (the position of the particle its search runs for, `()` at the top level) and returns what the component uses during that search. The library's own components (`PsoRule`, `Swarm1`, `Swarm2`, `Confined`, `for_field`, `InnerSearch`, `Run`) keep their v3 call shapes. Users name searches as `impl Search<Pos = P, Outer = O>`.
- **The preparation contract.** A component's closure is called exactly once for every search it takes part in, with that search's outer position, before the search scores anything. What it returns serves that search and no other.
- **`api.rs` shrinks to data:** positions (`Point`), particles, `BestSoFar`, and the three field declarations. No behaviour trait is given.
- **The route model no longer implements library traits.** It offers plain domain methods (`score`, `confine_waypoints`, `leg_limits`, `random_timings`, ...), and the planner wires them into closures. `leg_limits` computes a route's points once, which is exactly the kind of per-search preparation the contract exists for.

**How the author's design properties are forced without describing them.**

| Property of the author's design | What forces it | Checked by |
|---|---|---|
| Components are owned values | Compositions are built in helper functions, returned as `impl Search`, stored, cloned, and moved to threads | `h_values` |
| Every component accepts the outer position, and those that ignore it compose anywhere | Closures that ignore it (`objective(\|_\| ...)`) and the library's components appear in inner searches at every depth; the outer type is inferred from where a component is used | `h_context_mix` (all 8 combinations), compile checks |
| Context is set before a search begins and fixed until it ends, on a fresh copy per search | The preparation contract | Recording closures count preparations (exactly one per search, for the right outer position) and check that each component is prepared before it's used: `h_record`, `h_three_levels`, `h_swapped_other`, visible `nested.rs` |
| Nesting recurses; each level sees the level directly above | Three-level compositions | `h_three_levels` |
| Types are inferred from the arguments | Hidden call sites never name library types; closure parameters are annotated only with public and user types | every hidden test |
| Mismatched compositions are type errors | Compile checks, now also for a search that needs an outer position being run at the top level | `compile_checks/` |
| Per-particle randomness, so results don't depend on threads | Bit-identical results across thread counts | `h_parallel` |

The instruction never mentions `OuterAware`, `see_outer`, associated versus generic outer types, cloning per particle, preparing into separate values, per-particle random streams or a serial reduction. Each of those is a design decision left to the agent.

**Fairness: a second, independent design passes the verifier.** `tools/alternatives/nested-search/prepared/` differs from the reference on every axis that matters. The outer position is a trait *parameter* (`Rule<O>`), so context-free components need no phantom type. Preparing a component builds a separate value (`Rule::Ready`) instead of setting state on a fresh clone. Only some constructors are unbounded (`Confined::new`, `PsoRule::new` and the closure builders); `Swarm1::new`, `Swarm2::new` and `InnerSearch::new` have bounds. (An earlier version of this sentence said none had bounds. The second audit corrected it, and the v5 trial solutions, whose constructors have none, were added as alternatives.) Random streams come from a hash of the seed and the particle index, and the best so far from a parallel reduction. The reference planner compiles against it unchanged. It passes every check, and its false-failure rate is measured like the reference's.

Other fairness decisions:

- The PSO formula is now stated in the instruction, because the visible test that pinned it is hidden. The hidden test can no longer inject a fixed RNG into a rule, since rules' interfaces are the agent's. Instead it checks the formula through one-iteration runs: exact inertia when a particle sits at both bests, and the cognitive and social pulls isolated and bounded per coordinate.
- The instruction states that closures and what they return are `Clone + Send + Sync`, and the tests only use such closures.
- The rule closure's signature is fixed (`Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>)`), so user rules can take randomness without knowing the library's RNG type.

**The earlier lesson, refined.** Version 1 concluded that "a verifier with hidden tests needs a fixed public API, so the task author has to make the API's design decisions". That holds for the API users *call*, not for the traits behind it. With closures, every user component takes the outer position in its signature and ignores it with `|_|`, so the coherence problem the author met in February (components that may or may not take context) moves inside the library, where the agent has to solve it.

**Calibration plan for v4 (decided 2026-10-26 18:40):** Claude Haiku 5.5 and Claude Sonnet 5.5, one attempt each, 30-minute agent cap (halved from v1-v3's hour, so a model that flails doesn't burn tokens).

### Version 5: requirements that interact (2026-10-26)

v4 doubled Sonnet's effort but was still easy for both models. More requirements of the same kind add work, not difficulty. The author asked instead for requirements that interact, so that difficulty grows faster than work: each one cheap to state and test, but the obvious way to meet one breaks another. Of the candidates, the author kept only those a real user of the library would want:

- **Components may borrow data** that outlives the run, such as a large table the caller owns. A real need: objectives often read large data, and capturing `&data` in a closure is how Rust code is normally written. It interacts with returning compositions from functions, nesting, cloning and parallelism. The easy ways out all fail it: `'static` bounds (Haiku's v4 design), boxing into `dyn` objects, spawned threads. Lifetimes have to flow through every generic type.
- **Each component's randomness is its own:** changing how much randomness one component draws never changes what any other receives. The real needs behind it: observing a run mustn't change it (a logger that samples particles at random), and comparing two configurations under the same random numbers is a standard way to separate real improvements from noise. It interacts with per-particle streams, the design every solution so far used, including both trial solutions and both of the author's designs: with one stream per particle, a swarm's second rule receives whatever the first left over. The fix needs streams per component, and they must stay separate through wrappers, nesting and seeders.
- **Rejected:** sharing one inner search between particles at the same position. Memoizing inner solves is real in bilevel optimization, but PSO positions are continuous, so exact duplicates almost never occur. It would read as a requirement invented to make the task hard.

Both requirements are stated as behaviour, with the realistic example, and the instruction says nothing about how to meet them. Visible: one search that borrows its problem (`nested.rs`) and one random-sampling logger (`single.rs`). Hidden: `h_borrowed` (a non-`Clone` table borrowed by every kind of component, results identical to an owning composition, thread pools, a scoped thread, a struct holding the search next to its data) and `h_streams` (a rule drawing 0, 1, 3 or 8 numbers before or after PSO, next to an inner search, inside one, and a seeder that draws more after making the same particles).

**Checked before any trial:** both v4 trial solutions, and the v4 reference and alternative, fail the new checks, so the requirements bite. The updated reference derives one stream per (rule, particle, iteration). The updated alternative keeps a persistent stream per (rule, particle) for the whole run. That exposed the subtlest interaction: its inner search lent its stream to the seeder, so a seeder that drew more shifted every later inner search's seed. Giving the seeder its own stream fixed it. The reference escapes this only because its streams are fresh every iteration. New mutants: both rules sharing one stream, the inner seed drawn after the seeder, closure components required to be `'static`.

### What the change makes possible

*Version 1's design, kept as the record. Later versions changed the names and files; see the version sections above.*

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
- **Compile checks without `trybuild`:** `trybuild` compares exact compiler messages, which change with the agent's design. Instead, each case is a file with a `//CASE` line and a `//FAIL` line. The `CASE` form must compile and the `FAIL` form must not. Because the two forms differ in one line (two in `inner_unit_not_a_part`, both for the same mismatch), a passing check proves the rejection comes from the intended mismatch and not from something unrelated.
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

The same tool also runs **alternative valid solutions** (`--alternatives`), which must all pass: scoring every particle before moving any, an `Arc<Mutex>` best so far with a deep `Clone`, and an inner search cloned fresh for every particle instead of being reset. The mutants show the verifier rejects what's wrong; the alternatives show it accepts reasonable designs that differ from the reference.

`tools/calibrate.sh` runs the reference's tests over many disjoint seed ranges and counts how often each one fails by bad luck. Budgets and thresholds were adjusted until every test file had 0 failures in 1,000 runs (the independence test in 3,000). By the rule of three, that puts each file's false-failure rate below 0.3% with 95% confidence.

### Build risks

- The reference solution is built before the tests are final. If it can't satisfy a locked call site without annotations of internals, the call site changes, not the rule.
- Miri must work offline in the image: install the nightly with `rust-src` and run `cargo miri setup` while the image is built. Also set `MIRI_SYSROOT`, or cargo-miri tries to rebuild the sysroot in every container and fails over the image layer.
- Never share one Cargo target directory between different source trees (the reference, mutants, alternatives). Test binaries get the same file names in both, so a calibration run can silently execute a mutant's binary. This happened once: the reference seemed to fail 1,997 of 2,000 routes runs, at exactly the scores of the timing-skip mutant built just before. `tools/selftest.sh` gives each part its own directory.
- **Miri with rayon** (found by the v3 smoke test, where the reference scored 0 in Docker): rayon's dependency crossbeam-epoch breaks Miri's default aliasing model (Stacked Borrows), which its own diagnostic calls "still experimental". The check now uses Tree Borrows (`-Zmiri-tree-borrows`), plus `-Zmiri-ignore-leaks` because rayon's pool threads outlive the test. Data races are checked under either model. Miri also reports one CPU, so rayon's global pool would have a single worker and nothing to race. The Miri test therefore runs in an explicit 2-thread pool. Checked: a copy of the reference with an unsynchronized counter written while scoring is reported as a data race between the two workers. `tools/local_verify.sh` doesn't run Miri, so only the Docker smoke test could catch this.
- **Deadlocks** (found by the self-test): the mutant whose best so far is behind an `Arc<Mutex>` deadlocks under rayon (a thread holding the lock waits on parallel work that needs it), and the self-test hung on it for an hour. Each test file's binary now runs under a 3-minute limit, about 58 times the slowest reference file in Docker (3.1 s), and the instruction states the limit. The verifier's overall timeout went up to 1 hour, so a solution that deadlocks in every file still gets a clean 0 rather than a Harbor timeout.
- Rust doesn't guarantee the precision of float functions like `powi`, and Miri varies it on purpose. Tests that compare results exactly must use only operations IEEE 754 makes exact (`d * d`, not `powi(2)`). Found when the clone-equality check failed under Miri only.

### Hardening the verifier (2026-10-27)

Before publishing, the author had a second Claude instance audit the repository, without access to the conclusions reached so far. It found the trial results genuine. No agent touched the hidden tests, the reference solution or the verifier, and every agent's copies of the locked files were byte-identical to the originals. But it found two holes in the verifier, and both turned out to be real:

1. **"Locked" files that weren't locked.** `test.sh` restored `api.rs` and `routes/model.rs`, but the agent's `lib.rs` decides which files are compiled. A `#[path]` attribute can point `mod api` or `mod model` at the agent's own copies. Two new mutants do exactly that. `fake_route_model` compiles a route model that accepts every route and scores it 0, so the planner needn't work. `lenient_equality` compiles an `api.rs` whose particles and bests always compare equal, which hides one random stream per particle. Both escaped every check of the committed verifier. The hidden tests now judge with their own code. They compare results field by field (positions with the tests' own `PartialEq`, scores bit for bit), score routes with their own copy of the model, and check that the library's model gives the same results as that copy. Both mutants are now caught. This is not a source scan: the agent may lay out its files as it likes, but the judges no longer come from its crate.
2. **The agent's code ran with full rights during grading.** Test binaries ran as root, next to the verifier's scripts and logs, and each test file was compiled just before it ran, so an earlier test could rewrite a later one. Worse, Harbor reads `reward.json` before `reward.txt`, and `test.sh` wrote only `reward.txt`. A Docker demonstration took the reference with nothing running in parallel (a bug only `h_parallel` catches), plus code that runs once in the first search. That code wrote `reward.json` and rewrote `tests/h_parallel.rs`. Under the committed `test.sh` the rewrite went through, the real bug passed, and the reward was **1.0**, even though Miri failed. `test.sh` now compiles everything before running any of the agent's code. It runs every test binary as `nobody`, in a scratch directory, and kills that user's leftover processes after each step. Root then deletes any reward file and writes the reward last. Under the new `test.sh` the same solution got "Permission denied" on the test file and scored **0.0**. Its write to `reward.json` still succeeded: Harbor bind-mounts the log directory, and it's writable by any user. The final deletion is what protects the reward, not the directory's permissions.

What remains exposed, knowingly. Miri runs as root, because it needs root's toolchain and the build directory, and it relies on Miri's isolation, which rejects file and process access by default. The unprivileged user can still overwrite other files in the log directory. Those are human-readable logs; `results.json` and the reward are written by root afterwards.

The audit's other findings were fixed as well:

- Calibration counted only `FAILED` lines, so a crash or a timeout counted as a pass. It now counts every non-zero exit, under a time limit, with the test harness's default threading, as in the verifier.
- This file's v1 sections are now labelled as history, and the calibration table renders again.
- The trial script no longer names a default location for the token file.

The hardened verifier, re-checked in full:

- Harbor: the reference scores 1.0 and the do-nothing agent 0.0.
- 3/3 alternative designs are accepted.
- 24/24 mutants are caught, both hacks included.
- No run fails in 1,000 per test file on the reference, or in 333 on the alternative design, with crashes and timeouts now counted.

### Second audit: mutants, compile checks, links (2026-10-27)

A second fresh audit read every mutant against the instruction, judged each compile check against designs other than the reference's, and checked every link. The links were all fine. Its findings, and what was done:

- **The real-bug mutant wasn't the real bug.** `no_best_reset` removes the reset, but the reference clones a fresh inner search for every particle, so the February form never happened. It was caught only through reruns at the top level. A new mutant, `inner_search_reused`, keeps inner searches and reuses them for later particles without a reset, as the original code did. The independence test catches it the way the original bug showed: particle B gets particle A's timings.
- **Two mutants were caught by the reference's own panic.** If a component is used unprepared, the reference panics. So `region_sees_nothing` and `rules_see_nothing` show only that a crash fails, and their descriptions now say so. The alternative design can't express a missing preparation at all, and wrong preparations that don't crash are covered by `inner_prepared_for_default` and `inner_prepared_for_swarm_best`.
- **Two mutants deadlocked.** `shared_best_arc` and `one_rng_for_all_particles` held a lock across rayon work, so they were caught by timeouts, not by their bugs. They now hold the lock only to copy a value or draw a seed, and they fail on what they're about: different results at different thread counts.
- **Bug classes without a mutant.** `cognitive_uses_swarm_best` and `region_before_rule` were added; both are caught.
- **A false fairness claim.** The README and this file said the alternative design has no constructor bounds, but three of its constructors have them. So no accepted design showed that types settled late, at the point of use, pass the verifier. That's the style that exposed v4's unfair compile checks. The v5 trial solutions by Haiku and Sonnet are written in that style, and they're now accepted alternatives, unedited.
- **Hidden helpers assumed `Run::new` needs only `Search`.** Three helpers bounded their searches by `Search` alone. A valid design whose `Run::new` also requires `Clone` or `Send` wouldn't have compiled them. The helpers now use the instruction's own shape, `Search<...> + Clone + Send`.
- **A thread check that could be flaky on 2 CPUs.** Scoring an outer position was so fast that the starting thread could finish all of it before another thread stole any. The recording objective now sleeps 0.1 ms per call, so other threads always join.
- Wording: the compile checks' twins differ in one line, except `inner_unit_not_a_part`, which differs in two for the same mismatch.

A judgment kept as is: `borrowed_and_owned_compositions_agree` assumes a component's randomness doesn't depend on its closure's type. The instruction ties all randomness to the seed, so that holds for any reasonable design.

Re-checked afterwards:

- 5/5 alternatives are accepted.
- 27/27 mutants are caught, with no timeouts and none failing to build. `mutants.py` now reports a variant whose library doesn't compile as proving nothing, instead of as caught.
- `h_parallel` fails no run in 1,000 on either design.
- Harbor: the reference scores 1.0 and the do-nothing agent 0.0.

### Verifier coverage (v5)

What the instruction asks for, against what the verifier actually checks. The checks sample each requirement; they don't prove it in general.

| The instruction asks for | What the verifier checks |
|---|---|
| **Composition:** searches and rules are values that can be returned, stored, cloned and sent to threads; they nest, and rules can be restricted to one field | Compositions built in functions, stored, cloned (clones interleaved and independent), reused and moved to other threads; three levels of nesting; swapped roles and field orders |
| **Context:** each component of an inner search is prepared exactly once for its outer particle, before the search scores anything | Recording closures log every preparation and call, at every depth |
| **Inference:** user code never names the library's types, and compositions that don't fit don't compile | Every test is written that way, so a design that needs annotations fails to compile them. Six mismatched compositions must fail to compile, each beside a near-identical twin that must compile |
| **Borrowed data:** components capture references to data that is neither `Clone` nor `'static` | Such compositions are returned, nested, cloned, run in pools and moved to scoped threads |
| **Exact reproducibility in parallel:** a seeded run is bit-identical at any thread count | 1 thread against 2, 3, 4 or 8, in several compositions; the threads that do the work are recorded |
| **Per-component randomness:** how much one component draws never changes what another receives | A rule (before or after PSO, beside or inside an inner search) or the seeder draws 1, 3 or 8 extra numbers, and the rest of the run must stay bit-identical |
| **A working planner:** good, feasible routes | Unseen route scenarios, with score thresholds set from measured distributions |

The verifier also checks the PSO update against its formula and ties between particles, and runs a small composition under Miri in a 2-thread pool, so undefined behaviour and data races fail.

## Calibration

### Log

| Date | Task | Model | Harness | Budget | Attempts | Result |
|---|---|---|---|---|---|---|
| 2026-10-26 | nested-search v1 | Claude Sonnet 5.5 (subscription) | Claude Code via Harbor 0.25.0 | 1 h agent time, network limited to the model API | 1 | Pass: 7 turns, 130 s agent time, about $0.40 at API prices. A genuine solution, close to the reference's design, with no hacks. |
| 2026-10-26 | nested-search v2 | Claude Sonnet 5.5 (subscription) | Claude Code via Harbor 0.25.0 | same | 1 | Pass: 6 turns, 153 s agent time, about $0.42. Genuine; locked files untouched. The three field traits, the hidden role swap and the route domain made no visible difference. |
| 2026-10-26 | nested-search v3 | Claude Sonnet 5.5 (subscription) | Claude Code via Harbor 0.25.0 | same | 1 | Pass: 9 turns, 177 s agent time, about $0.46. Genuine: every check passed, `api.rs` untouched. Per-particle random streams from a hash of the seed and the particle index (the same idea as one of the accepted alternatives), parallel scoring, a fresh inner search per particle. Reported speedup 3.6×. Parallelism added 3 turns and 24 seconds. |
| 2026-10-26 | nested-search v4 | Claude Sonnet 5.5 (subscription) | Claude Code via Harbor 0.25.0 | 30 min agent time, network limited to the model API | 1 | Pass: 13 turns, 375 s agent time, about $0.78. Genuine. Its own design: an associated `Outer` type on every component, and `prepare(outer)` building a separate prepared value (between the reference and the alternative design). |
| 2026-10-26 | nested-search v4 | Claude Haiku 5.5 (subscription) | same | same | 1 | Scored 0, then **pass on re-verification** after two unfair checks were fixed (below): 37 turns, 587 s agent time, about $0.35. Genuine: its own traits, no `unsafe`, locks or thread RNG. |
| 2026-10-26 | nested-search v5 | Claude Sonnet 5.5 (subscription) | same | same | 1 | Pass: 11 turns, 383 s agent time, about $0.85. Genuine, and the intended design found directly: one stream per (seed, iteration, rule slot, particle), and a separately tagged stream for the seeder. |
| 2026-10-26 | nested-search v5 | Claude Haiku 5.5 (subscription) | same | same | 1 | Scored 0, then **pass on re-verification** after one unfair test was fixed (below): 22 turns, 609 s agent time, about $0.37. Both new requirements passed on the first try. |
| 2026-10-29 | nested-search v5, hardened verifier | Claude Sonnet 5.5 (subscription) | same | same | 1 | Pass: 15 turns, 418 s agent time, about $0.97. Locked files byte-identical, no `#[path]`, `unsafe`, locks or thread RNG. |
| 2026-10-29 | nested-search v5, hardened verifier | Claude Haiku 5.5 (subscription) | same | same | 1 | Pass: 22 turns, 949 s agent time, about $0.33. Locked files byte-identical, no `#[path]`, `unsafe`, locks or thread RNG. The two trials ran side by side. |

One attempt isn't a pass rate, but it shows v1 is far too easy for current mid-size models: the solve took no exploration and one fix.

The author's prediction before v2's trial: still too easy. The agreed plan: if Sonnet 5.5 passes v2, merge task 2 into this one (reproducible parallelism) and measure again. A failure counts only if it's fair, meaning the agent's solution is really wrong and not rejected by an unfair check.

The author's decision before v3's trial (2026-10-26 18:40): if Sonnet 5.5 still passes easily, the pattern is clear. Every version so far hands the agent the design in `api.rs` and asks it to implement it, which frontier models do well. Adding requirements only makes the task longer. So v4 takes the design away: the agent has to come up with the architecture itself, under constraints that force the properties of the author's own design without describing it. v4 is then measured on both Claude Haiku 5.5 and Claude Sonnet 5.5, each with a 30-minute agent cap (halved from 1 hour, so a model that flails doesn't burn tokens).

**Two unfair checks, found by Haiku's v4 trial (2026-10-26).** Haiku's solution failed only the compile checks and the three-level test. Both failures came from the verifier:

- **The compile checks built compositions without running them.** Haiku's constructors have no bounds, so its types are settled where a composition is used (`Run::new`, `InnerSearch::new` with a typed search). A check that only builds a composition therefore demanded that every type be settled at construction, which the instruction doesn't require. Run as users run them, the same checks pass on Haiku's library, including every rejection. Every compile check now runs its composition (a `run!` helper in the prelude), which still rejects every mismatch.
- **A test helper returned a search that borrowed its argument.** `fn level3(chain, log: &Log<YT>) -> impl Search<...>` captures the borrow's lifetime under Rust 2024's capture rules, even though the search holds only owned clones. Haiku's library requires `'static` for nested searches, a defensible choice the instruction doesn't rule out, so the helper didn't compile against it. The helpers now take what they keep by value.

The agent's work was the same either way, so its saved source was re-verified with the fixed verifier instead of rerunning the agent: every check passes (Miri had already passed in the trial). The reference, the alternative design and Sonnet's solution also pass the fixed verifier.

**What v4 shows.** Taking the design away doubled Sonnet's effort (13 turns and 6 minutes, against 9 turns and 3 minutes on v3) and made Haiku work for 37 turns, but neither came near the 30-minute cap. With a crisp specification, visible tests and a local CI script that runs the same checks, designing a generic Rust architecture of this size is well within reach even for the small model. The effort curve over v1-v4 (Sonnet: 7, 6, 9, 13 turns) is the measurable result of the tuning.

**An unfair PSO test, found by Haiku's v5 trial (2026-10-26).** The test isolated the cognitive pull by giving a particle a personal best before the run and expecting it to survive. Haiku's library resets every particle's personal best when a run starts. The instruction says that nothing found in an earlier run may carry over into a search, so that's a defensible reading, arguably the more literal one. No test sets bests before a run any more: a user rule moves the particle away from its best during the run, and the test checks the combined pull (`cognitive * r1 + social * r2` per coordinate, which must come near its combined weight), next to the existing test that isolates the social pull. Haiku's saved source then passes everything, as do the reference, the alternative and Sonnet's solution.

**What v5 shows.** The interacting requirements didn't measurably raise the effort either: Sonnet took 11 turns (13 on v4), and Haiku met both new requirements on its first attempt. Taken with v1-v4, the evidence is consistent: a small, precisely specified Rust codebase with visible tests and a local copy of the verifier's checks is within reach of both models, however the requirements are combined. The difficulty that remained came from the verifier, in the form of three unfair checks, each found by a trial and fixed. That's the clearest argument for running real trials early.

**Calibration of v5, and thresholds set from distributions (2026-10-27).** The self-test caught all 22 mutants, but calibration found false failures in three hidden tests: 4 in 1,000 runs of the reference in h_borrowed, 1 in h_independence, and 1 in 333 runs of the alternative design in h_routes. Rather than move thresholds until the failures stopped, a probe printed each test's raw metric over the same 1,000 seed ranges, for both designs:

- **h_borrowed:** its convergence test reused the small inner search (6 iterations, 4 particles) that the bit-for-bit comparison tests use. The worst error was 0.050 for the reference and 0.065 for the alternative, against a threshold of 0.05. With h_values' size (20 iterations, 8 particles) the worst is 0.0067 and 0.0043.
- **h_independence:** the timing error is below 1e-7 except in 3 of 2,000 runs, where it's exactly 0.5. That's PSO stagnating on a face of the region in the one coordinate whose optimum is inside it, and it happens in both designs. The bug the test targets puts the error at 4, so the tolerance is now 1.0.
- **h_routes:** the scores never came near the threshold. The failure was a route cutting 0.0026 into a zone, beyond the check's 0.001 slack. The planner is the same code in both designs, and it searches on a penalized score, so it can return a route just inside a zone when it finds nothing better. That happened once in 6,000 plans. The hidden check's slack is now 0.01.

The probe also explained why the planner that skips its timing search (`planner_skips_timing`) is caught only by the visible route test: in the hidden scenarios its scores overlap the reference's (median -30.7 against -19.0, and -10.09 against -9.81). No threshold separates them without false failures, so the hidden thresholds stay where they are and say so in their comments.

The same self-test reported one alternative design rejected, on two runs in a row. It hadn't failed a test: it hadn't built. Under its work dir, a build script's path came to 260 characters, past Windows' limit, so the linker couldn't write it and every test counted as failed. From a shorter work dir all three alternatives pass. Docker runs the verifier on Linux, so only the local tools were affected; `selftest.sh` now warns about it. The lesson: a self-test should tell "rejected" apart from "didn't build".

### Setup

Every trial used Claude Code as the agent, run by Harbor 0.25.0 in Docker on the author's Claude subscription, with network access only to the model API: one attempt per model and version, with a 1-hour agent cap for v1-v3 and 30 minutes for v4-v5. Claude Sonnet 5.5 ran every version and Claude Haiku 5.5 ran v4 and v5. Both re-ran v5 on 2026-10-29 against the hardened verifier; those are the results the README reports. DeepSeek through another agent harness was considered in the plan but not run.

## Format

Harbor / Terminal-Bench style: a Dockerfile, an instruction, tests and a reference solution per task. Checked against Harbor 0.25.0, the version every trial and smoke test used.
