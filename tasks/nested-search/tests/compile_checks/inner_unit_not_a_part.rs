// The inner search's unit is not a part of the outer unit.
fn main() {
    let inner = GBestSearcherOne::new(
        //CASE FitOn::<T, W>::new(),
        //FAIL FitOn::<V, W>::new(),
        GBestSinglePMover::new(cfg(), rng()),
    );
    let _ = GBestSearcherTwo::new(
        FitOn::<W, ()>::new(),
        GBestSinglePMover::new(cfg(), rng()).on_part::<X>(),
        //CASE NestedMover::new(5, inner, InitFor::<W, T>::new(), rng()),
        //FAIL NestedMover::new(5, inner, InitFor::<W, V>::new(), rng()),
    );
}
