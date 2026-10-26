// The inner search's components expect outer positions X, but its outer particles are W.
fn main() {
    let inner = Swarm1::new(
        //CASE fit_on!(T, W),
        //FAIL fit_on!(T, X),
        PsoRule::new(cfg()),
    );
    let _ = Swarm2::new(
        fit_on!(W, ()),
        PsoRule::new(cfg()).for_field::<X>(),
        InnerSearch::new(5, inner, init_for!(W, T)),
    );
}
