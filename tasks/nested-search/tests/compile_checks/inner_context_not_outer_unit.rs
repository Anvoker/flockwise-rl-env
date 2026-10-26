// The inner search expects context X, but its outer particles are W.
fn main() {
    let inner = GBestSearcherOne::new(
        //CASE FitOn::<T, W>::new(),
        //FAIL FitOn::<T, X>::new(),
        GBestSinglePMover::new(cfg(), rng()),
    );
    let _ = GBestSearcherTwo::new(
        FitOn::<W, ()>::new(),
        GBestSinglePMover::new(cfg(), rng()).on_part::<X>(),
        NestedMover::new(5, inner, InitFor::<W, T>::new(), rng()),
    );
}
