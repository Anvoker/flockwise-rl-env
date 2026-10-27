# Alternative solutions to nested-search

Valid solutions whose designs differ from the reference. Each must pass the full verifier (`python tools/mutants.py <task dir> <work dir> --alternatives`), so they show the verifier accepts more than one design.

| Folder | Origin | Design |
|---|---|---|
| `prepared/` | Written for this repository, without looking at the reference's internals | The outer position is a trait parameter (`Rule<O>`). Each search prepares components into separate values. Each particle has persistent hashed random streams. The best is found by a parallel reduction. |
| `haiku-v5/` | Written by Claude Haiku 5.5 during the v5 trial, unedited | Constructors have no bounds, so types are settled where a composition is used. |
| `sonnet-v5/` | Written by Claude Sonnet 5.5 during the v5 trial, unedited | Its own traits, and constructors without bounds. Each (rule, particle, iteration) gets its own random stream. |

The trial solutions are the agents' saved `src/` from the Harbor runs logged in [DESIGN.md](../../../DESIGN.md#calibration). Their `api.rs` and `routes/model.rs` are the locked files, unchanged.
