// A user rule over T inside a search over X.
fn main() {
    let _ = Swarm1::new(
        fit_on!(X, ()),
        //CASE rule_on!(X, ()),
        //FAIL rule_on!(T, ()),
    );
}
