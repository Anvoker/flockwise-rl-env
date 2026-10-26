# Make nested, composable searches work in `nestswarm`

`/app` is `nestswarm`, a small particle swarm optimization (PSO) library in Rust. The maintainer wants searches to be built from interchangeable parts, including searches nested inside other searches. The public traits for this are already in `src/api.rs`. The implementation doesn't support it yet: it holds an earlier attempt (`GBestPSOPMover`, searchers that borrow their parts), and the tests in `tests/` don't compile.

Make the library support the design below, so that the tests compile and pass.

## What users must be able to do

1. **Treat searchers and movers as values.** Build them in a function, return them, store them in structs, clone them, and move them to other threads. A composition is `Clone` and `Send` whenever its components are.
2. **Nest searches.** `NestedMover::new(iterations, inner_searcher, init, rng)` is a mover that, for each particle it moves, runs a whole inner search and writes the inner search's global best into one part of that particle. Nesting can go any number of levels deep.
3. **Move one part of a unit.** `mover.on_part::<Part>()` (from the `OnPart` trait) runs a mover on one part of a larger unit. The mover sees that part of the particle and that part of the global best. Parts are declared with `PartOf`.
4. **Wrap any mover with a boundary.** `BoundedMover::new(mover, boundary)` applies the boundary to the position after the mover runs.
5. **Pass context into inner searches.** Every component of an inner search (its fitness function, its movers, their boundaries) receives the position of the outer particle that search runs for, through `Contextful::set_context`: before the inner search begins, and unchanged until it ends. In deeper nesting, each level receives the particle of the level directly above it. Components that don't need context ignore it, and compose the same way.
6. **Start every search fresh.** Each search, including each inner search, optimizes only for its own particles and its own outer particle. Nothing found for one outer particle, in an earlier run, or by another copy of a searcher may carry over into it.

Users combine these freely: two-mover searchers whose movers are a part-restricted PSO and a nested search, in either order and on either part; their own fitness functions, boundaries, movers and initializers next to the library's; three-level compositions.

## Behaviour

- In each iteration, every particle's fitness is evaluated exactly once, at the position the particle had when the iteration began, and each of the searcher's movers runs exactly once per particle. So an inner search with `m` particles and `n` iterations evaluates its fitness `m * n` times.
- The global best is the best position evaluated in this search. Fitness is maximized.
- `GBestSinglePMover` is standard global-best PSO.
- All randomness comes from the RNGs passed in. A clone, given the same inputs, behaves exactly like the original.

## Public API

Users only know these names, all exported from the crate root:

- From `src/api.rs`: `Unit`, `Contextful`, `FitCalc`, `Boundary`, `PartOf`, `Particle`, `ParticleMut`, `GBest`, `Mover`, `NestedInit`.
- `SearchConfigGBest`, `GBestSinglePMover::new(config, rng)`, `GBestSearcherOne::new(fitness, mover)`, `GBestSearcherTwo::new(fitness, mover1, mover2)`, `BoundedMover::new(mover, boundary)`, `OnPart::on_part::<Part>()`, `NestedMover::new(iterations, inner_searcher, init, rng)`.
- The `Searcher` trait (with `type TUnit`, and `Contextful` as a supertrait), so users can write `impl Searcher<TUnit = T> + Contextful<TContext = W>`, and `SearcherIter`.

User code never names anything else from the library, and never spells out the library's type parameters. The only type users name at a call site is a part, as in `on_part::<X>()`. Everything compiles from the arguments.

## Rules

- Change anything in `src/` except `src/api.rs`, which must stay exactly as it is.
- `tests/`, `compile_checks/`, `Cargo.toml`, `Cargo.lock` and `ci.sh` are read-only. Don't add dependencies. Only `src/` is taken from your work: everything else is restored before verification.
- No network access is needed. All dependencies are already fetched.

## How it's checked

`./ci.sh` runs what the verifier runs, on the visible tests:

1. `cargo test --release`.
2. The compile checks in `compile_checks/`: each case must compile in its `//CASE` form and fail to compile in its `//FAIL` form, so mismatched compositions are rejected by the type system.
3. A small composition under Miri (`cargo +$MIRI_TOOLCHAIN miri test --test miri`), which fails on undefined behaviour.

The verifier runs the same three steps with more tests: other compositions of the same components, such as three levels of nesting, swapped roles and part orders, components written outside the library, components that record every context they receive and count their calls, cloned searchers run side by side, searchers moved to other threads, and more compile checks for mismatched compositions.
