// The inner search's position is not a field of the outer position.
fn main() {
    let inner = Swarm1::new(
        //CASE fit_on!(T, W),
        //FAIL fit_on!(V, W),
        PsoRule::new(cfg()),
    );
    run!(Swarm2::new(
        fit_on!(W, ()),
        PsoRule::new(cfg()).for_field::<X>(),
        //CASE InnerSearch::new(5, inner, init_for!(W, T)),
        //FAIL InnerSearch::new(5, inner, init_for!(W, V)),
    ), W);
}
