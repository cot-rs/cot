# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.5.0](https://github.com/cot-rs/cot/compare/cot-v0.4.0...cot-v0.5.0) - 2026-01-21

[View diff on diff.rs](https://diff.rs/cot/0.4.0/cot/0.5.0/Cargo.toml)

### New features

- [**breaking**] Cache support with pluggable backends ([#399](https://github.com/cot-rs/cot/pull/399)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- [**breaking**] Add Redis Cache store ([#410](https://github.com/cot-rs/cot/pull/410)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- [**breaking**] *(db)* Add `bulk_insert` ([#414](https://github.com/cot-rs/cot/pull/414)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Email support using lettre ([#419](https://github.com/cot-rs/cot/pull/419)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- Add derive macro `SelectAsFormField` ([#397](https://github.com/cot-rs/cot/pull/397)) (by [@kumarUjjawal](https://github.com/kumarUjjawal))
- Expose cache in request API ([#448](https://github.com/cot-rs/cot/pull/448)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- Add Redirect and deprecate new_redirect ([#451](https://github.com/cot-rs/cot/pull/451)) (by [@seqre](https://github.com/seqre))

### Fixes

- Clippy errors ([#402](https://github.com/cot-rs/cot/pull/402)) (by [@m4tx](https://github.com/m4tx))

### Other

- [**breaking**] Rename opt to opts ([#398](https://github.com/cot-rs/cot/pull/398)) (by [@seqre](https://github.com/seqre))
- [**breaking**] Use `trait_upcasting`, bump MSRV to 1.86 ([#412](https://github.com/cot-rs/cot/pull/412)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Bump deps, bump MSRV to 1.88 ([#431](https://github.com/cot-rs/cot/pull/431)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Use `Arc` internally in `Database` ([#432](https://github.com/cot-rs/cot/pull/432)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Fix includes for `#[derive(Template)]` ([#446](https://github.com/cot-rs/cot/pull/446)) (by [@Kijewski](https://github.com/Kijewski))
- [**breaking**] Cleanup `RequestExt`; use extractors instead ([#449](https://github.com/cot-rs/cot/pull/449)) (by [@m4tx](https://github.com/m4tx))
- *(deps)* Bump the dependencies group with 22 updates ([#401](https://github.com/cot-rs/cot/pull/401)) (by [@dependabot[bot]](https://github.com/dependabot[bot]))
- Remove `#[cfg(doc_auto_cfg)]` ([#406](https://github.com/cot-rs/cot/pull/406)) (by [@m4tx](https://github.com/m4tx))
- Remove rust-lang/rust#145288 workaround ([#411](https://github.com/cot-rs/cot/pull/411)) (by [@m4tx](https://github.com/m4tx))
- `Cache` instead of `Arc<Cache>` in public APIs ([#433](https://github.com/cot-rs/cot/pull/433)) (by [@m4tx](https://github.com/m4tx))

## [0.4.0](https://github.com/cot-rs/cot/compare/cot-v0.3.1...cot-v0.4.0) - 2025-09-11

[View diff on diff.rs](https://diff.rs/cot/0.3.1/cot/0.4.0/Cargo.toml)

### New features

- [**breaking**] Add support for file fields in forms ([#334](https://github.com/cot-rs/cot/pull/334)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Add `SelectField`; support more chrono form fields ([#345](https://github.com/cot-rs/cot/pull/345)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Error handling overhaul, more powerful custom error handlers ([#373](https://github.com/cot-rs/cot/pull/373)) (by [@m4tx](https://github.com/m4tx))
- Support multiple session stores ([#277](https://github.com/cot-rs/cot/pull/277)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- Support more chrono datatypes in the framework ([#332](https://github.com/cot-rs/cot/pull/332)) (by [@seqre](https://github.com/seqre))
- Add simple `AsFormField` impl for `ForeignKey`s ([#335](https://github.com/cot-rs/cot/pull/335)) (by [@m4tx](https://github.com/m4tx))
- FromRequestParts derive macro ([#336](https://github.com/cot-rs/cot/pull/336)) (by [@kumarUjjawal](https://github.com/kumarUjjawal))
- More Session config knobs ([#337](https://github.com/cot-rs/cot/pull/337)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- AsFormField for Date, Time, DateTime ([#342](https://github.com/cot-rs/cot/pull/342)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- Add validated `Url` type ([#339](https://github.com/cot-rs/cot/pull/339)) (by [@kingzcheung](https://github.com/kingzcheung))
- *(macros)* Derive macro for `SelectChoice` trait ([#351](https://github.com/cot-rs/cot/pull/351)) (by [@kumarUjjawal](https://github.com/kumarUjjawal))
- `ToDbValue` for `Url` and bug fixes for `Url` and `Email` ([#353](https://github.com/cot-rs/cot/pull/353)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- DB session store implementation ([#360](https://github.com/cot-rs/cot/pull/360)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- Basic benchmarking ([#378](https://github.com/cot-rs/cot/pull/378)) (by [@seqre](https://github.com/seqre))
- IntoResponse and ApiOperationResponse derive macro for enum ([#388](https://github.com/cot-rs/cot/pull/388)) (by [@kumarUjjawal](https://github.com/kumarUjjawal))

### Fixes

- [**breaking**] OpenAPI specs with item references ([#333](https://github.com/cot-rs/cot/pull/333)) (by [@m4tx](https://github.com/m4tx))
- Clippy warning in the latest nightly ([#346](https://github.com/cot-rs/cot/pull/346)) (by [@m4tx](https://github.com/m4tx))
- Ignored doctest ([#372](https://github.com/cot-rs/cot/pull/372)) (by [@m4tx](https://github.com/m4tx))
- Swagger not working when static files weren't served at `/static/` ([#383](https://github.com/cot-rs/cot/pull/383)) (by [@m4tx](https://github.com/m4tx))
- Trybuild tests on the latest nightly ([#386](https://github.com/cot-rs/cot/pull/386)) (by [@m4tx](https://github.com/m4tx))

### Other

- [**breaking**] Remove deprecated items ([#349](https://github.com/cot-rs/cot/pull/349)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] Add `#[non_exhaustive]` to config structs ([#354](https://github.com/cot-rs/cot/pull/354)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] *(deps)* Bump all dependencies ([#361](https://github.com/cot-rs/cot/pull/361)) (by [@dependabot[bot]](https://github.com/dependabot[bot]))
- [**breaking**] Add `#[non_exhaustive]` to `FormError` variants ([#374](https://github.com/cot-rs/cot/pull/374)) (by [@m4tx](https://github.com/m4tx))
- [**breaking**] `FromRequest(Parts)` only gets immutable request parts ([#377](https://github.com/cot-rs/cot/pull/377)) (by [@m4tx](https://github.com/m4tx))
- Tiny doc and code consistency fixes ([#348](https://github.com/cot-rs/cot/pull/348)) (by [@m4tx](https://github.com/m4tx))
- Fix clippy warnings on Rust 1.88 ([#355](https://github.com/cot-rs/cot/pull/355)) (by [@m4tx](https://github.com/m4tx))
- Custom `Default` impl for `SessionMiddlewareConfig` ([#359](https://github.com/cot-rs/cot/pull/359)) (by [@ElijahAhianyo](https://github.com/ElijahAhianyo))
- Warn on `clippy::allow_attributes`; fix clippy warnings ([#363](https://github.com/cot-rs/cot/pull/363)) (by [@m4tx](https://github.com/m4tx))
- Fix warning on stable clippy ([#364](https://github.com/cot-rs/cot/pull/364)) (by [@m4tx](https://github.com/m4tx))
- *(pre-commit)* Add HTML/Jinja2 linter & formatter ([#365](https://github.com/cot-rs/cot/pull/365)) (by [@melroy12](https://github.com/melroy12))
- Style fixes; replace todo!() with unimplemented!() ([#370](https://github.com/cot-rs/cot/pull/370)) (by [@m4tx](https://github.com/m4tx))
- Extract `LiveReloadMiddleware` to a separate file ([#375](https://github.com/cot-rs/cot/pull/375)) (by [@m4tx](https://github.com/m4tx))
- *(deps)* Bump the dependencies group with 11 updates ([#384](https://github.com/cot-rs/cot/pull/384)) (by [@dependabot[bot]](https://github.com/dependabot[bot]))
- *(deps)* Bump all deps ([#396](https://github.com/cot-rs/cot/pull/396)) (by [@m4tx](https://github.com/m4tx))

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
