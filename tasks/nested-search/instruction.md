# Make nested, composable searches work in `flockwise`

`/app` is `flockwise`, a small particle swarm optimization (PSO) library in Rust, together with its main user: a drone delivery route planner in `src/routes/`. The maintainer wants searches to be built from interchangeable parts, including searches nested inside other searches. The planner needs exactly that: an outer search over a route's waypoints, and for every candidate route an inner search over its leg timings, whose limits depend on the waypoints.

The public traits for this design are already in `src/api.rs`, and the route model in `src/routes/model.rs` is written against them. The rest of the library is an earlier attempt that doesn't support the design: searchers borrow their parts, and the planner's inner search comes from a function pointer. Nothing compiles at the moment.

Make the library support the design below, and make `routes::plan` work, so that the tests compile and pass.

## What users must be able to do

1. **Treat searches and rules as values.** Build them in a function, return them, store them in structs, clone them, and move them to other threads. A composition is `Clone` and `Send` whenever its components are.
2. **Nest searches.** `InnerSearch::new(iterations, inner_search, seeder, rng)` is a rule that, for each particle it moves, runs a whole inner search and writes the inner search's best position into one field of that particle. Nesting can go any number of levels deep.
3. **Move one field of a position.** `rule.for_field::<Field>()` (from the `ForField` trait) runs a rule on one field of a larger position. The rule sees that field of the particle and that field of the best position so far. Fields are declared with `SplitFrom`, `ReadFrom` and `WriteInto`.
4. **Confine any rule to a region.** `Confined::new(rule, region)` applies the region to the position after the rule runs.
5. **Show inner searches where they are.** Every component of an inner search (its objective, its rules, their regions) is shown the position of the outer particle that search runs for, through `OuterAware::see_outer`, before the inner search begins, and it stays the same until the search ends. In deeper nesting, each level is shown the particle of the level directly above it. Components that don't need it ignore it, and compose the same way.
6. **Start every search fresh.** Each search, including each inner search, optimizes only for its own particles and its own outer particle. Nothing found for one outer particle, in an earlier run, or by another copy of a search may carry over into it.
7. **Plan routes.** `routes::plan(scenario, budget, seed)` returns a feasible route (`Scenario::check`) that scores well (`Scenario::score`), using these components.

Users combine these freely: two-rule swarms whose rules are a field-restricted PSO and an inner search, in either order and on either field; their own objectives, regions, rules and seeders next to the library's; three-level compositions.

## Behaviour

- In each iteration, every particle is scored exactly once, at the position it had when the iteration began, and each of the swarm's rules runs exactly once per particle. So an inner search with `m` particles and `n` iterations scores `m * n` positions.
- The best position so far is the best position scored in this search. Scores are maximized.
- `PsoRule` is standard global-best PSO.
- All randomness comes from the RNGs and seeds passed in. A clone, given the same inputs, behaves exactly like the original, and `plan` gives the same route for the same seed.

## Public API

Users only know these names, all exported from the crate root unless noted:

- From `src/api.rs`: `Point`, `OuterAware`, `Objective`, `Region`, `SplitFrom`, `ReadFrom`, `WriteInto`, `Particle`, `ParticleView`, `BestSoFar`, `Rule`, `Seeder`.
- `PsoParams`, `PsoRule::new(params, rng)`, `Swarm1::new(objective, rule)`, `Swarm2::new(objective, rule1, rule2)`, `Confined::new(rule, region)`, `ForField::for_field::<Field>()`, `InnerSearch::new(iterations, inner_search, seeder, rng)`.
- The `Search` trait (with `type Pos`, and `OuterAware` as a supertrait), so users can write `impl Search<Pos = T> + OuterAware<Outer = W>`, and `Run::new(iterations, &mut search, &mut particles)`, an iterator over the best position so far after each iteration.
- `routes::plan`, `routes::Budget`, and everything in `routes::model`.

User code never names anything else from the library, and never spells out the library's type parameters. The only type users name at a call site is a field, as in `for_field::<X>()`. Everything compiles from the arguments.

## Rules

- Change anything in `src/` except `src/api.rs` and `src/routes/model.rs`, which must stay exactly as they are.
- `tests/`, `compile_checks/`, `Cargo.toml`, `Cargo.lock` and `ci.sh` are read-only. Don't add dependencies. Only `src/` is taken from your work: everything else is restored before verification.
- There's no network access. All dependencies are already fetched. Documentation is available offline: the standard library's at `$(rustc --print sysroot)/share/doc/rust/html/std/index.html` (`rustup doc --std --path` prints the path), and the dependencies' in `/app/target/doc/`.

## How it's checked

`./ci.sh` runs what the verifier runs, on the visible tests:

1. `cargo test --release`.
2. The compile checks in `compile_checks/`: each case must compile in its `//CASE` form and fail to compile in its `//FAIL` form, so mismatched compositions are rejected by the type system.
3. A small composition under Miri (`cargo +$MIRI_TOOLCHAIN miri test --test miri`), which fails on undefined behaviour.

The verifier runs the same three steps with more tests: other compositions of the same components, such as three levels of nesting, swapped roles and field orders, components written outside the library, components that record every outer position they're shown and count their calls, searches cloned and run side by side, searches moved to other threads, other route scenarios, and more compile checks for mismatched compositions.
