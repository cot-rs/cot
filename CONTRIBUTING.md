# Contributing to Cot

Cot welcomes contribution from everyone in the form of suggestions, bug
reports, pull requests, and feedback. This document gives some guidance if you
are thinking of helping us.

## Questions, feedback, and feature requests

If you have a question, feedback, or feature request, please open an
[issue](https://github.com/cot-rs/cot/issues/new),
a [discussion](https://github.com/cot-rs/cot/discussions/new/choose), or
join our [Discord server](https://discord.cot.rs/) to talk with us directly.

## Submitting issues

When reporting a bug or asking for help, please include enough details so that
the people helping you can reproduce the behavior you are seeing. For some tips
on how to approach this, read about how to produce a [Minimal, Complete, and
Verifiable example](https://stackoverflow.com/help/mcve).

When making a feature request, please make it clear what problem you intend to
solve with the feature, any ideas for how Cot could support solving that
problem, any possible alternatives, and any disadvantages.

## Pull requests

We're happy to help you get started contributing to Cot. If you're looking for
a place to start, check out the
[good first issue](https://github.com/cot-rs/cot/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22good%20first%20issue%22)
label on the issue tracker. If you're looking for something more challenging,
check out the
[help wanted](https://github.com/cot-rs/cot/issues?q=is%3Aissue%20state%3Aopen%20label%3A%22help%20wanted%22)
label, or just talk to us directly. We're happy to help you get started.

## Test suites, CI, and code style

We encourage you to check that the test suite passes locally before submitting a
pull request with your changes. If anything does not pass, typically it will be
easier to iterate and fix it locally than waiting for the CI servers to run
tests for you.

We are also using [`pre-commit`](https://pre-commit.com/) hooks to handle
formatting and linting. See the `pre-commit` website for installation
instructions. This handles formatting of all the files in the repository.

### Tests that use database, cache, or other external resources

Some tests use a database, cache, or other external resources. All these tests
are marked with `#[ignore]`, so they are not run by default.

If you want to run the full test suite, it's necessary to run these external
dependencies. For convenience, Cot provides a
[Docker compose file](./compose.yml) in the root of the repository that
contains all the dependencies needed to run the tests. You can run it with:

```sh
docker compose up -d
```

Then, the tests can be run with:

```sh
cargo test --all-features --include-ignored
```

#### End-to-end tests

End-to-end tests require a running webdriver server. By default, a Selenium
Grid server is used (included in the `compose.yml` file). You can access the
UI to see the tests running (for example, to debug them) at
`http://localhost:7900/?autoconnect=1&resize=scale&password=secret`.

Alternatively, instead of using Selenium Grid, you can run the tests with
a local webdriver server. To do this, you need to install and run the
Webdriver implementation of your choice, such as
[geckodriver](https://github.com/mozilla/geckodriver/releases) or
[chromedriver](https://developer.chrome.com/docs/chromedriver/downloads).
After running the webdriver server, you will see the tests running in a
local browser window.

### Snapshot tests

Cot uses snapshot testing for the CLI to ensure that the output of commands
remains consistent across changes. We
use [cargo-insta](https://github.com/mitsuhiko/insta)
and [insta-cmd](https://github.com/mitsuhiko/insta-cmd) for snapshot testing,
which automate the whole process. Tool's documentation is
available [here](https://insta.rs/docs/).

When making changes to the CLI, you may need to update the snapshots if your
changes intentionally modify the output. You can do this by running:

```sh
cargo insta test --review
```

### Dependencies

When adding a new dependency to the project, please consider if it's actually
needed and if it's possible to avoid it. If it's not plausible, make sure
that the dependency is well-maintained and has a permissive license.

When adding a new dependency, please add it to the `Cargo.toml` file of the
workspace root, not to the `Cargo.toml` file of the crate, even if it's
not being used in any other crate. This way, all the dependencies are in
one place, and it's easier to manage them. The only exception to this rule
is the `examples` directory – examples should have all their dependencies
(except for the `cot` crate) listed in their own `Cargo.toml` file, so it's
easier to copy them to a new project.

The dependency's version should be pinned to the least specific version
possible. This means you should avoid specifying the patch and minor version,
as long as it works with your patch. The CI will check if the project builds
with the minimum version specified in the `Cargo.toml` file. By doing this,
we ensure that the project is not tied to a specific version of the dependencies,
potentially avoiding duplicate dependencies in the tree, and, more importantly,
avoiding problems when one of the dependencies is yanked.

## Conduct

We follow the [Code of Conduct](CODE_OF_CONDUCT.md).
