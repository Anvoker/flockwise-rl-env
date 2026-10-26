// Inside an inner search, the boundary's context type differs from the fitness's.
fn main() {
    let inner = Swarm1::new(
        FitOn::<T, W>::new(),
        Confined::new(
            PsoRule::new(cfg()),
            //CASE BoundOn::<T, W>::new(),
            //FAIL BoundOn::<T, ()>::new(),
        ),
    );
    let _ = Swarm2::new(
        FitOn::<W, ()>::new(),
        PsoRule::new(cfg()).for_field::<X>(),
        InnerSearch::new(5, inner, InitFor::<W, T>::new()),
    );
}
