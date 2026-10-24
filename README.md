A parser for the [HOA](https://adl.github.io/hoaf/) format that is used to describe [ω-automata](https://en.wikipedia.org/wiki/%CE%A9-automaton).
Could be used standalone, but really should be used as part of the [automata](https://github.com/leonbohn/automata) package(s).

Uses [chumsky](https://github.com/zesterer/chumsky/) for parsing and complements it with some [ariadne](https://github.com/zesterer/ariadne) for diagnostics.

### Format
The format itself is thorougly described on [this website](https://adl.github.io/hoaf/) or in the manual found in `doc/hoaf.pdf`.
Obtaining an overview of ω-automata helps before diving into the format itself, see for example [this one](https://spot.lre.epita.fr/concepts.html) given by the makers of [spot](https://spot.lre.epita.fr/).

#### Changelog (most likely incomplete)
**0.2.1**

Version bump to reflect that the code of the crate has been moved.

**0.2.0**

Fix some bugs in the extraction of a label expression for an edge or alias. This no longer defaults to 8 atomic propositions, but builds an abstract expression that is then to be further handled by the consumer downstream.

**0.1.1**

Added lots and lots of documentation, some helper methods and introduced a
method for converting label expressions to DNF, which in turn allows to
use them as symbols for an automaton.

**0.1.0**

First release 0.1 version, probably buggy as hell.
