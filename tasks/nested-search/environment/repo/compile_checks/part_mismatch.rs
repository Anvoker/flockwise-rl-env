// A rule for `x` (its region is over X) placed on the `t` field.
fn main() {
    let _ = Swarm1::new(
        fit_on!(W, ()),
        Confined::new(PsoRule::new(cfg()), bound_on!(X, ()))
            //CASE .for_field::<X>(),
            //FAIL .for_field::<T>(),
    );
}
