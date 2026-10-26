// A mover for `x` (its boundary is over X) placed on the `t` part.
fn main() {
    let _ = Swarm1::new(
        FitOn::<W, ()>::new(),
        Confined::new(PsoRule::new(cfg()), BoundOn::<X, ()>::new())
            //CASE .for_field::<X>(),
            //FAIL .for_field::<T>(),
    );
}
