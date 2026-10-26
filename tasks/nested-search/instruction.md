# Design and build nested, composable, parallel searches for `flockwise`

`/app` is `flockwise`, a small particle swarm optimization (PSO) library in Rust, together with its main user: a drone delivery route planner in `src/routes/`. The maintainer wants searches to be built from interchangeable parts, including searches nested inside other searches, where any part may depend on the position of the particle its search runs for. Searches must use every core while staying exactly reproducible. The planner needs all of that: an outer search over a route's waypoints, and for every candidate route an inner search over its leg timings, whose limits depend on the waypoints.

The rest of the library is an earlier, sequential attempt that doesn't support this: searches borrow their parts, and the planner's inner search comes from a function pointer. Nothing compiles at the moment.

How users write code against the library is decided, and is described below. How the library works inside is not: design it. `src/api.rs` (positions, particles, and how a position declares its fields) and the route model in `src/routes/model.rs` are fixed. Make the library support the usage below, and make `routes::plan` work, so that the tests compile and pass.

## What users must be able to do

1. **Write components as closures.** A component is built from a closure that is given the *outer position*: the position of the particle its search runs for, or `()` for a search that isn't nested. It returns what the component uses during that search:
   - `objective(f)`: `f(outer)` returns a function `Fn(P) -> f64`, the score to maximize.
   - `region(f)`: `f(outer)` returns a function `Fn(P) -> P` that maps a position back into the feasible region.
   - `rule(f)`: `f(outer)` returns a function `Fn(&BestSoFar<P>, &mut dyn RngCore, ParticleView<'_, P>)` that moves one particle, given the best position its search has found so far and a random number generator.
   - `seeder(f)`: `f(outer, rng)` with `rng: &mut dyn RngCore` returns the `Vec<Particle<I>>` an inner search starts from.

   `P` and `I` are positions (`Point`), and `RngCore` is `rand::RngCore`. The closures, and the functions they return, are `Clone + Send + Sync`.
2. **Prepare once per search.** A component's closure is called exactly once for every search the component takes part in, with that search's outer position, before the search scores anything. What it returns serves every call during that search and no other. A component that doesn't need the outer position ignores it (`objective(|_| ...)`), and works in any search at any depth, as do the library's own components.
3. **Treat searches and rules as values.** Build them in a function, return them (as `impl Search<Pos = P, Outer = O> + Clone + Send`), store them in structs, clone them, and move them to other threads. A composition is `Clone` and `Send` whenever its components are.
4. **Nest searches.** `InnerSearch::new(iterations, inner_search, seeder)` is a rule that, for each particle it moves, runs a whole inner search whose outer position is that particle's position, starting from the particles the seeder makes for it, and writes the inner search's best position into the particle's field of that type. Nesting can go any number of levels deep. Each level's components get the position of the particle at the level directly above.
5. **Move one field of a position.** `rule.for_field::<Field>()` runs a rule on one field of a larger position. The rule sees that field of the particle and that field of the best position so far. Fields are declared with `SplitFrom`, `ReadFrom` and `WriteInto` (`src/api.rs`).
6. **Confine any rule to a region.** `Confined::new(rule, region)` applies the region to the position after the rule runs.
7. **Start every search fresh.** Each search, including each inner search, optimizes only for its own particles and its own outer position. Nothing found for one outer particle, in an earlier run, or by another copy of a search may carry over into it.
8. **Use every core.** Searches do their per-particle work (scoring, and applying rules, including whole inner searches) in parallel on the current rayon thread pool.
9. **Reproduce exactly.** Given the same seed, a run gives bit-for-bit the same results, every particle and every best so far, whatever the number of threads in the pool.
10. **Plan routes.** `routes::plan(scenario, budget, seed)` returns a feasible route (`Scenario::check`) that scores well (`Scenario::score`), using these components.

Users combine all of this freely: two-rule swarms whose rules are a field-restricted PSO and an inner search, in either order and on either field; their own closures next to the library's components, inside wrappers and at any depth; three-level compositions. Compositions that don't fit together (a rule for one field placed on another, an inner search whose components expect a different outer position, a search that needs an outer position run at the top level) must not compile.

## Behaviour

- In each iteration, every particle is scored exactly once, at the position it had when the iteration began. Then the personal bests and the best so far are updated; when two particles score the same, the earlier one in the slice wins. Then each of the swarm's rules runs exactly once per particle. So an inner search with `m` particles and `n` iterations scores `m * n` positions.
- The best position so far is the best position scored in this search. Scores are maximized.
- `PsoRule` is standard global-best PSO: `v = inertia * v + cognitive * r1 * (personal_best - x) + social * r2 * (best_so_far - x)`, then `x = x + v`, with `r1` and `r2` drawn from `[0, 1)` independently for every coordinate (see `Point::map_scalars`).
- All randomness comes from the seed given to `Run::new` (and to `plan`). A clone, given the same inputs, behaves exactly like the original.

## Public API

Users only know these names, all exported from the crate root unless noted, and they import them with `use flockwise::*;`:

- From `src/api.rs`: `Point`, `SplitFrom`, `ReadFrom`, `WriteInto`, `Particle`, `ParticleView`, `BestSoFar`.
- `objective`, `region`, `rule` and `seeder`, as above.
- `PsoParams` (with public fields `inertia`, `cognitive`, `social`), `PsoRule::new(params)`, `Swarm1::new(objective, rule)`, `Swarm2::new(objective, rule1, rule2)` (rules applied in that order), `Confined::new(rule, region)`, `rule.for_field::<Field>()`, `InnerSearch::new(iterations, inner_search, seeder)`.
- The `Search` trait, with associated types `Pos` and `Outer`, which every search implements.
- `Run::new(iterations, &mut search, &mut particles, seed)`, where `particles` is a `Vec<Particle<Pos>>`: runs a search that isn't nested (`Outer = ()`), as an iterator over the best so far (`BestSoFar`) after each iteration. Afterwards, `particles` holds the particles' final state.
- `routes::plan`, `routes::Budget`, and everything in `routes::model`.

User code never names anything else from the library, and never spells out the library's type parameters. The only types users write at a call site are their own and the ones listed above: a field, as in `for_field::<X>()`, and the types of closure parameters. Everything else is inferred from the arguments.

## Rules

- Change anything in `src/` except `src/api.rs` and `src/routes/model.rs`, which must stay exactly as they are.
- `tests/`, `compile_checks/`, `Cargo.toml`, `Cargo.lock` and `ci.sh` are read-only. Don't add dependencies. Only `src/` is taken from your work: everything else is restored before verification.
- There's no network access. All dependencies are already fetched. Documentation is available offline: the standard library's at `$(rustc --print sysroot)/share/doc/rust/html/std/index.html` (`rustup doc --std --path` prints the path), and the dependencies' in `/app/target/doc/`.

## How it's checked

`./ci.sh` runs what the verifier runs, on the visible tests:

1. `cargo test --release`.
2. The compile checks in `compile_checks/`: each case must compile in its `//CASE` form and fail to compile in its `//FAIL` form.
3. A small composition under Miri, with the flags `ci.sh` sets (`-Zmiri-tree-borrows -Zmiri-ignore-leaks`, because of rayon's internals), which fails on undefined behaviour, data races included.

Each test file must finish within 3 minutes (the reference solution takes a few seconds).

The verifier runs the same three steps with more tests: the PSO update checked against its formula, other compositions of the same components, such as three levels of nesting, swapped roles and field orders, components that use or ignore the outer position in every combination, user-written rules, closures that record every outer position they're given and count their calls and preparations, searches cloned, reused and run side by side, searches moved to other threads, results compared across thread counts, the threads that do the work recorded, ties between particles, other route scenarios, and more compile checks for compositions that don't fit. It also reports how much faster 8 threads are than 1, without scoring it.
