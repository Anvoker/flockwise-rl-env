// The inner search's unit is not a part of the outer unit.
fn main() {
    let inner = Swarm1::new(
        //CASE FitOn::<T, W>::new(),
        //FAIL FitOn::<V, W>::new(),
        PsoRule::new(cfg(), rng()),
    );
    let _ = Swarm2::new(
        FitOn::<W, ()>::new(),
        PsoRule::new(cfg(), rng()).for_field::<X>(),
        //CASE InnerSearch::new(5, inner, InitFor::<W, T>::new(), rng()),
        //FAIL InnerSearch::new(5, inner, InitFor::<W, V>::new(), rng()),
    );
}
