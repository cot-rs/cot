# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.0](https://github.com/cot-rs/cot/compare/cot-cli-v0.2.2...cot-cli-v0.3.0) - 2025-05-13

### <!-- 1 -->New features

- *(static-files)* [**breaking**] refactor, add config and content hashing ([#317](https://github.com/cot-rs/cot/pull/317))
- [**breaking**] append app name to table name ([#257](https://github.com/cot-rs/cot/pull/257))

### <!-- 3 -->Other

- *(static-files)* use URL rewriting and cache by default ([#320](https://github.com/cot-rs/cot/pull/320))
- update project template to use `Html` instead of `Response` ([#319](https://github.com/cot-rs/cot/pull/319))
- *(clippy)* allow API breaking clippy lints ([#305](https://github.com/cot-rs/cot/pull/305))
- *(deps)* [**breaking**] bump deps (upgrade to askama 0.14) ([#293](https://github.com/cot-rs/cot/pull/293))
- [**breaking**] migrate from rinja to askama ([#265](https://github.com/cot-rs/cot/pull/265))

## [0.2.2](https://github.com/cot-rs/cot/compare/cot-cli-v0.2.1...cot-cli-v0.2.2) - 2025-04-03

### <!-- 3 -->Other

- cli snapshot testing ([#272](https://github.com/cot-rs/cot/pull/272))

## [0.2.1](https://github.com/cot-rs/cot/compare/cot-cli-v0.2.0...cot-cli-v0.2.1) - 2025-03-30

### <!-- 2 -->Fixes

- *(cli)* fix modified models detection ([#266](https://github.com/cot-rs/cot/pull/266))
- *(cli)* tests relying on cwd ([#269](https://github.com/cot-rs/cot/pull/269))
- *(cli)* migration generator not working in inner project directories ([#267](https://github.com/cot-rs/cot/pull/267))

### <!-- 3 -->Other

- use #[expect] instead of #[allow] where it makes sense ([#259](https://github.com/cot-rs/cot/pull/259))

## [0.2.0](https://github.com/cot-rs/cot/compare/cot-cli-v0.1.4...cot-cli-v0.2.0) - 2025-03-25

### <!-- 1 -->New features

- [**breaking**] use extractor pattern for request handlers ([#253](https://github.com/cot-rs/cot/pull/253))
- *(cli)* add generation of manpages and shell completions ([#252](https://github.com/cot-rs/cot/pull/252))
- add `SessionMiddleware` configuration ([#251](https://github.com/cot-rs/cot/pull/251))
- cot-cli commands makeover ([#226](https://github.com/cot-rs/cot/pull/226))
- create Workspace Manager ([#235](https://github.com/cot-rs/cot/pull/235))
- add support for remove field in automatic migration generator ([#232](https://github.com/cot-rs/cot/pull/232))
- support "Remove Model" in Automatic Migration Generator ([#221](https://github.com/cot-rs/cot/pull/221))

### <!-- 2 -->Fixes

- unit test after commit [25785c2](https://github.com/cot-rs/cot/commit/25785c27) ([#218](https://github.com/cot-rs/cot/pull/218))

### <!-- 3 -->Other

- remove duplication in migration generator tests ([#249](https://github.com/cot-rs/cot/pull/249))
- [**breaking**] upgrade edition to 2024 ([#244](https://github.com/cot-rs/cot/pull/244))
- *(clippy)* add --all-targets to clippy CI and fix all warnings ([#240](https://github.com/cot-rs/cot/pull/240))
- more docs (up to 100% doc coverage) ([#229](https://github.com/cot-rs/cot/pull/229))
- change MigrationGenerator for future use ([#224](https://github.com/cot-rs/cot/pull/224))

## [0.1.4](https://github.com/cot-rs/cot/compare/cot-cli-v0.1.3...cot-cli-v0.1.4) - 2025-02-28

### Fixed

- use cot's version instead of cli's when creating a new project (#213)

### Other

- Add status messages to CLI operations for better user feedback ([#204](https://github.com/cot-rs/cot/pull/204))

## [0.1.3](https://github.com/cot-rs/cot/compare/cot-cli-v0.1.2...cot-cli-v0.1.3) - 2025-02-24

### Other

- updated the following local packages: cot

## [0.1.2](https://github.com/cot-rs/cot/compare/cot-cli-v0.1.1...cot-cli-v0.1.2) - 2025-02-23

### Fixed

- add Cargo.lock to project template to avoid broken dependencies (#191)

## [0.1.1](https://github.com/cot-rs/cot/compare/cot-cli-v0.1.0...cot-cli-v0.1.1) - 2025-02-21

### Other

- add README.md to the Cargo.toml metadata (#178)

## 0.1.0 - 2025-02-18

- initial version
