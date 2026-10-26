// A search whose objective needs an outer position can't run at the top level.
fn main() {
    let mut search = Swarm1::new(
        //CASE fit_on!(X, ()),
        //FAIL fit_on!(X, W),
        rule_on!(X, ()),
    );
    let mut particles: Vec<Particle<X>> = Vec::new();
    let _ = Run::new(1, &mut search, &mut particles, 0).count();
}
