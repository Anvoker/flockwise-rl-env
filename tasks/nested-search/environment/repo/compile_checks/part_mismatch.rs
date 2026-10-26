// A mover for `x` (its boundary is over X) placed on the `t` part.
fn main() {
    let _ = GBestSearcherOne::new(
        FitOn::<W, ()>::new(),
        BoundedMover::new(GBestSinglePMover::new(cfg(), rng()), BoundOn::<X, ()>::new())
            //CASE .on_part::<X>(),
            //FAIL .on_part::<T>(),
    );
}
