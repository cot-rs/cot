use tempfile::TempDir;

use super::*;
#[test]
fn no_args() {
    insta::with_settings!(
        { filters => GENERIC_FILTERS.to_owned() },
        { assert_cmd_snapshot!(cot_cli!("cli")) }
    );
}

#[test]
fn manpages() {
    let tempdir = TempDir::new().unwrap();
    insta::with_settings!(
        { filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS].concat() },
        { assert_cmd_snapshot!(cot_cli!("cli", "manpages", "-o", tempdir.path())) }
    );
}

#[test]
fn completions_missing_shell() {
    insta::with_settings!(
        { filters => [GENERIC_FILTERS, TEMP_PATH_FILTERS].concat() },
        { assert_cmd_snapshot!(cot_cli!("cli", "completions")) }
    );
}

#[test]
fn completions_bash() {
    assert_cmd_snapshot!(cot_cli!("cli", "completions", "bash"));
}

#[test]
fn completions_elvish() {
    assert_cmd_snapshot!(cot_cli!("cli", "completions", "elvish"));
}

#[test]
fn completions_fish() {
    assert_cmd_snapshot!(cot_cli!("cli", "completions", "fish"));
}

#[test]
fn completions_powershell() {
    assert_cmd_snapshot!(cot_cli!("cli", "completions", "powershell"));
}

#[test]
fn completions_zsh() {
    assert_cmd_snapshot!(cot_cli!("cli", "completions", "zsh"));
}
