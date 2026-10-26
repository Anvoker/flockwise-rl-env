// The inner search expects context X, but its outer particles are W.
fn main() {
    let inner = Swarm1::new(
        //CASE FitOn::<T, W>::new(),
        //FAIL FitOn::<T, X>::new(),
        PsoRule::new(cfg(), rng()),
    );
    let _ = Swarm2::new(
        FitOn::<W, ()>::new(),
        PsoRule::new(cfg(), rng()).for_field::<X>(),
        InnerSearch::new(5, inner, InitFor::<W, T>::new(), rng()),
    );
}
