# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1](https://github.com/cot-rs/cot/compare/cot-v0.3.0...cot-v0.3.1) - 2025-05-16

### <!-- 1 -->New features

- allow more types to be used as a primary key ([#330](https://github.com/cot-rs/cot/pull/330))
- more just commands and clippy fix ([#331](https://github.com/cot-rs/cot/pull/331))

### <!-- 2 -->Fixes

- don't clone string in `<Html as IntoResponse>` ([#324](https://github.com/cot-rs/cot/pull/324))

### <!-- 3 -->Other

- add contributors to the README.md ([#327](https://github.com/cot-rs/cot/pull/327))

## [0.3.0](https://github.com/cot-rs/cot/compare/cot-v0.2.2...cot-v0.3.0) - 2025-05-13

### <!-- 1 -->New features

- implement `AsFormField` for floating point types ([#307](https://github.com/cot-rs/cot/pull/307))
- extractor & IntoResponse for Html and Json ([#321](https://github.com/cot-rs/cot/pull/321))
- *(static-files)* [**breaking**] refactor, add config and content hashing ([#317](https://github.com/cot-rs/cot/pull/317))
- [**breaking**] implement and handle `IntoResponse` ([#256](https://github.com/cot-rs/cot/pull/256))
- add form support for Email field ([#286](https://github.com/cot-rs/cot/pull/286))
- generate OpenAPI specs automatically ([#287](https://github.com/cot-rs/cot/pull/287))
- allow FnOnce for RequestHandlers ([#283](https://github.com/cot-rs/cot/pull/283))
- use SCSS ([#278](https://github.com/cot-rs/cot/pull/278))
- [**breaking**] append app name to table name ([#257](https://github.com/cot-rs/cot/pull/257))

### <!-- 2 -->Fixes

- migration engine only running the first operation in a migration ([#310](https://github.com/cot-rs/cot/pull/310))
- *(docs)* invalid whitespace in a doc ([#303](https://github.com/cot-rs/cot/pull/303))
- allow `#[model]` to be put before `#[derive(AdminModel)]` ([#295](https://github.com/cot-rs/cot/pull/295))
- build when minimal dependency versions are used ([#288](https://github.com/cot-rs/cot/pull/288))
- actually use SessionMiddleware config ([#279](https://github.com/cot-rs/cot/pull/279))

### <!-- 3 -->Other

- don't log backtraces ([#318](https://github.com/cot-rs/cot/pull/318))
- [**breaking**] add `TestServer` utility and add some E2E tests ([#315](https://github.com/cot-rs/cot/pull/315))
- *(docs)* improve `RequestHandler` docs ([#314](https://github.com/cot-rs/cot/pull/314))
- [**breaking**] add `#[non_exhaustive]` to more enums ([#297](https://github.com/cot-rs/cot/pull/297))
- *(deps)* [**breaking**] bump deps (upgrade to askama 0.14) ([#293](https://github.com/cot-rs/cot/pull/293))
- allow trailing comma in static_files macro ([#291](https://github.com/cot-rs/cot/pull/291))
- password comparison ([#285](https://github.com/cot-rs/cot/pull/285))
- [**breaking**] migrate from rinja to askama ([#265](https://github.com/cot-rs/cot/pull/265))

## [0.2.2](https://github.com/cot-rs/cot/compare/cot-v0.2.1...cot-v0.2.2) - 2025-04-03

### <!-- 2 -->Fixes

- don't show 404 when there are 0 objects in admin panel ([#270](https://github.com/cot-rs/cot/pull/270))

### <!-- 3 -->Other

- update `admin` example with a custom `TodoItem` model ([#270](https://github.com/cot-rs/cot/pull/270))

## [0.2.1](https://github.com/cot-rs/cot/compare/cot-v0.2.0...cot-v0.2.1) - 2025-03-30

### <!-- 2 -->Fixes

- *(cli)* migration generator not working in inner project directories ([#267](https://github.com/cot-rs/cot/pull/267))

### <!-- 3 -->Other

- use #[expect] instead of #[allow] where it makes sense ([#259](https://github.com/cot-rs/cot/pull/259))
- add #[diagnostic::on_unimplemented] for key traits ([#260](https://github.com/cot-rs/cot/pull/260))
- *(deps)* use database dependencies only with the "db" feature flag ([#264](https://github.com/cot-rs/cot/pull/264))

## [0.2.0](https://github.com/cot-rs/cot/compare/cot-v0.1.4...cot-v0.2.0) - 2025-03-25

### <!-- 0 -->Security

- cycle session ID on login, flush session on logout ([#246](https://github.com/cot-rs/cot/pull/246))

### <!-- 1 -->New features

- [**breaking**] use extractor pattern for request handlers ([#253](https://github.com/cot-rs/cot/pull/253)),
  introducing `FromRequest` and `FromRequestParts` traits and removing duplicated functionality from `RequestExt`
- add `SessionMiddleware` configuration ([#251](https://github.com/cot-rs/cot/pull/251))
- user-friendly message for `AddrInUse` error ([#233](https://github.com/cot-rs/cot/pull/233))
- support for "Remove Field" in automatic migration generator ([#232](https://github.com/cot-rs/cot/pull/232))
- support for "Remove Model" in Automatic Migration Generator ([#221](https://github.com/cot-rs/cot/pull/221))
- basic pagination support for admin panel ([#217](https://github.com/cot-rs/cot/pull/217))
- display object paths when (de)serialization error happened with serde
- add `RegisterAppsContext`, `AuthBackendContext`, `MiddlewareContext` as type aliases for `ProjectContext` in specific
  bootstrapping phases that are more semantic and whose names won't change when changing the phases

### <!-- 2 -->Fixes

- panic backtrace/location not displayed on the error page ([#237](https://github.com/cot-rs/cot/pull/237))
- include APP_NAME in model ([#228](https://github.com/cot-rs/cot/pull/228))

### <!-- 3 -->Other

- [**breaking**] upgrade edition to 2024 ([#244](https://github.com/cot-rs/cot/pull/244))
- [**breaking**] remove methods from the `RequestExt` that duplicate extractors' functionalities
- [**breaking**] `AuthRequestExt` trait is now replaced by `Auth` struct and `AuthMiddleware` is now required for
- [**breaking**] add `WithDatabase` bootstrapping phase
- `Urls` object can now be used with the `reverse!` macro and not only `Request`
- *(clippy)* add --all-targets to clippy CI and fix all warnings ([#240](https://github.com/cot-rs/cot/pull/240))
- add test for reverse!() reversing in the current app first ([#239](https://github.com/cot-rs/cot/pull/239))
- more docs (up to 100% doc coverage) ([#229](https://github.com/cot-rs/cot/pull/229))

## [0.1.4](https://github.com/cot-rs/cot/compare/cot-v0.1.3...cot-v0.1.4) - 2025-02-28

### Added

- add #[track_caller] to `unwrap`s for better panic messages (#212)

### Fixed

- use cot's version instead of cli's when creating a new project (#213)

## [0.1.3](https://github.com/cot-rs/cot/compare/cot-v0.1.2...cot-v0.1.3) - 2025-02-24

### Other

- add logo to the rustdoc (#198)

## [0.1.2](https://github.com/cot-rs/cot/compare/cot-v0.1.1...cot-v0.1.2) - 2025-02-23

### Added

- *(error)* enhance error logging with tracing integration (#186)

### Fixed

- switch back to using non-prerelease versions of crypto crates (#188)

### Other

- *(deps)* add info about dependencies to CONTRIBUTING.md (#192)

## [0.1.1](https://github.com/cot-rs/cot/compare/cot-v0.1.0...cot-v0.1.1) - 2025-02-21

### Other

- add README.md to the Cargo.toml metadata (#178)
- fix a typo in form.rs docs (#173)

## 0.1.0 - 2025-02-18

- initial version
