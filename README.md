<div align="center">
<h1><img src="https://raw.githubusercontent.com/cot-rs/media/6585c518/logo/logo-text.svg" alt="Cot" width="300"></h1>

[![Rust Build Status](https://github.com/cot-rs/cot/workflows/Rust%20CI/badge.svg)](https://github.com/cot-rs/cot/actions/workflows/rust.yml)
[![crates.io](https://img.shields.io/crates/v/cot.svg)](https://crates.io/crates/cot)
[![Guide](https://img.shields.io/website?url=https%3A%2F%2Fcot.rs%2Fguide%2Flatest%2F&label=guide&up_message=online)](https://cot.rs/guide/latest/)
[![Documentation](https://docs.rs/cot/badge.svg)](https://docs.rs/cot)
[![codecov](https://codecov.io/gh/cot-rs/cot/branch/master/graph/badge.svg)](https://codecov.io/gh/cot-rs/cot)
[![Discord chat](https://img.shields.io/discord/1330137289287925781?logo=Discord&logoColor=white)](https://discord.cot.rs)
[![GitHub Sponsors](https://img.shields.io/github/sponsors/cot-rs?label=GitHub%20sponsors)](https://github.com/sponsors/cot-rs)
[![Open Collective backers](https://img.shields.io/opencollective/backers/cot?label=Open%20Collective%20backers)](https://opencollective.com/cot)
</div>

> [!WARNING]
> Cot is currently missing a lot of features and is **not ready** for anything even remotely close to production use.
> That said, you are more than welcome to try it out and provide feedback!

Cot is an easy to use, modern, and fast web framework for Rust. It has been designed to be familiar if you've ever
used [Django](https://www.djangoproject.com/), and easy to learn if you haven't. It's a batteries-included framework
built on top of [axum](https://github.com/tokio-rs/axum).

## Features

* **Easy to use API** — in many ways modeled after Django, Cot's API is designed to be easy to use and intuitive.
  Sensible defaults make it for easy rapid development, while the API is still empowering you when needed. The
  documentation is a first-class citizen in Cot, making it easy to find what you're looking for.
* **ORM integration** — Cot comes with its own ORM, allowing you to interact with your database in a way that feels
  Rusty and intuitive. Rust types are the source of truth, and the ORM takes care of translating them to and from the
  database, as well as creating the migrations automatically.
* **Type safe** — wherever possible, Cot uses Rust's type system to prevent common mistakes and bugs. Not only views
  are taking advantage of the Rust's type system, but also the ORM, the admin panel, and even the templates. All that to
  catch errors as early as possible.
* **Admin panel** — Cot comes with an admin panel out of the box, allowing you to manage your app's data with ease.
  Adding new models to the admin panel is stupidly simple, making it a great tool not only for rapid development and
  debugging, but with its customization options, also for production use.
* **Secure by default** — security should be opt-out, not opt-in. Cot takes care of making your web apps secure by
  default, defending it against common modern web vulnerabilities. You can focus on building your app, not securing it.

## Getting Started

<a href="https://repology.org/project/rust%3Acot-cli/versions">
    <img src="https://repology.org/badge/vertical-allrepos/rust%3Acot-cli.svg" alt="Packaging status" align="right">
</a>

To get started with Cot, you need to have Rust installed. If you don't have it yet, you can install it by following
the instructions on the [official Rust website](https://www.rust-lang.org/tools/install).

Then, you need to install cot-cli by running:

```shell
cargo install cot-cli
```

After that, you can create a new project by running:

```shell
cot new my_project
```

This will create a new project in the `my_project` directory. You can then navigate to the project directory and run
the following command to start the development server:

```shell
cargo run
```

**We recommend you to read the [official guide](https://cot.rs/guide/latest/) to learn more about Cot
and how to use it.**

### cot-cli packages

If you prefer to use your operating system's package manager to manage the `cot-cli` package, you can find it in the
repositories listed in the “Packaging status” badge on the right. Note that most of these packages are maintained by
the community, so you should always check what exactly is included in the package. Moreover, the version in the package
manager might not be the latest one, so we recommend just using the official package which can be installed with
`cargo install cot-cli`.

## Development

### Testing

Tests that require using external databases are ignored by default. In order to run them, execute the following in the
root of the repository:

```shell
docker compose up -d
cargo test --all-features -- --include-ignored
```

You can them execute the following command to stop the database:

```shell
docker compose down
```

## Star History

<a href="https://star-history.com/#cot-rs/cot&Date">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=cot-rs/cot&type=Date&theme=dark" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/svg?repos=cot-rs/cot&type=Date" />
   <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=cot-rs/cot&type=Date" />
 </picture>
</a>

## License

Cot is licensed under either of the following, at your option:

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or https://www.apache.org/licenses/LICENSE-2.0)
* MIT License ([LICENSE-MIT](LICENSE-MIT) or https://opensource.org/licenses/MIT)

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Cot by you shall be
dual licensed under the MIT License and Apache License, Version 2.0, without any additional terms or conditions.
