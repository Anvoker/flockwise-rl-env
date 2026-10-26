// Inside an inner search, the boundary's context type differs from the fitness's.
fn main() {
    let inner = GBestSearcherOne::new(
        FitOn::<T, W>::new(),
        BoundedMover::new(
            GBestSinglePMover::new(cfg(), rng()),
            //CASE BoundOn::<T, W>::new(),
            //FAIL BoundOn::<T, ()>::new(),
        ),
    );
    let _ = GBestSearcherTwo::new(
        FitOn::<W, ()>::new(),
        GBestSinglePMover::new(cfg(), rng()).on_part::<X>(),
        NestedMover::new(5, inner, InitFor::<W, T>::new(), rng()),
    );
}
