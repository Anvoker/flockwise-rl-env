// Inside an inner search, the region's outer position type differs from the objective's.
fn main() {
    let inner = Swarm1::new(
        fit_on!(T, W),
        Confined::new(
            PsoRule::new(cfg()),
            //CASE bound_on!(T, W),
            //FAIL bound_on!(T, ()),
        ),
    );
    run!(Swarm2::new(
        fit_on!(W, ()),
        PsoRule::new(cfg()).for_field::<X>(),
        InnerSearch::new(5, inner, init_for!(W, T)),
    ), W);
}
