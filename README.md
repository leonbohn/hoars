A parser for the [HOA](https://adl.github.io/hoaf/) format that is used to describe [ω-automata](https://en.wikipedia.org/wiki/%CE%A9-automaton).
Could be used standalone, but really should be used as part of the [automata](https://github.com/leonbohn/automata) package(s).

Uses [chumsky](https://github.com/zesterer/chumsky/) for parsing and complements it with some [ariadne](https://github.com/zesterer/ariadne) for diagnostics.

### Format
The format itself is thorougly described on [this website](https://adl.github.io/hoaf/) or in the manual found in `doc/hoaf.pdf`.
Obtaining an overview of ω-automata helps before diving into the format itself, see for example [this one](https://spot.lre.epita.fr/concepts.html) given by the makers of [spot](https://spot.lre.epita.fr/).

#### Changelog (most likely incomplete)

**0.5.0** (2026-11-10)
We no longer depend on `biodivine-lib-bdd` and instead move the parsing downstream to the `automata` crate, which in turn will use another BDD library `oxiDD`. So there is no BDD type anymore.

**0.4.0** (2026-11-06)

See `CHANGELOG.md` for the full list. Breaking: `HoaRepresentation::aps` returns `&[String]`, `num_aps` returns `u16`, `acceptance` returns `Option<HoaAcceptance>`, and `Header::acceptance_name` returns `Option<AcceptanceName>`; accessors no longer panic on missing optional header items. `AbstractLabelExpression::try_into_bdd` now takes only a `&BddVariableSet` (the AP `i` is variable `BddVariable::from_index(i)`), and takes `&self`; `try_into_prop`/`try_into_hoa_expression` take the AP count as `u16`. The unused `build_vars` helper was removed. Overflowing integers are now parse errors instead of panics, and labels with nested conjunctions/disjunctions are converted correctly.

**0.3.0**

Breaking: updated `biodivine-lib-bdd` to 0.6 (its `Bdd` type is part of the public API via `HoaExpression`). Also updated `itertools` to 0.15, moved to the 2024 edition, and the code now lives in its own repository again.

**0.2.1**

Version bump to reflect that the code of the crate has been moved (not published).

**0.2.0**

Fix some bugs in the extraction of a label expression for an edge or alias. This no longer defaults to 8 atomic propositions, but builds an abstract expression that is then to be further handled by the consumer downstream.

**0.1.1**

Added lots and lots of documentation, some helper methods and introduced a
method for converting label expressions to DNF, which in turn allows to
use them as symbols for an automaton.

**0.1.0**

First release 0.1 version, probably buggy as hell.
