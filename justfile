default:
    @just --choose {{ justfile() }}

alias u := update-lockfiles
alias c := clippy
alias cov := coverage
alias d := docs

update-lockfiles: update-workspace-lockfile update-template-lockfile

update-workspace-lockfile:
    cargo update

update-template-lockfile:
    #!/usr/bin/env bash
    set -euxo pipefail
    tmpdir=$(mktemp -d)

    # special project name to make it appear at the end of the lockfile
    # and to make it unlikely to be used anywhere else
    proj_name="zzzzzzzzzz_tmp_project"
    proj_dir="$tmpdir/$proj_name"
    cargo_lock_path="$proj_dir/Cargo.lock"

    echo $tmpdir
    cargo run --bin cot -- new --cot-path "$(pwd)/cot" $proj_dir
    cargo update --manifest-path "$proj_dir/Cargo.toml"
    sed -i 's/$proj_name/\{\{ project_name \}\}/' $cargo_lock_path
    cp $cargo_lock_path cot-cli/src/project_template/Cargo.lock.template
    rm -rf $tmpdir

clippy:
    cargo +stable clippy --no-deps --all-targets

coverage:
    # generate coverage report as HTML
    # requires cargo-llvm-cov installed and nightly toolchain
    cargo llvm-cov --all-features --workspace --branch --doctests --html --open

docs:
    # generate docs for the `cot` crate with similar settings to docs.rs
    # requires nightly toolchain
    RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --no-deps --all-features --lib
