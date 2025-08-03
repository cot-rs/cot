#[rustversion::attr(
    not(nightly),
    ignore = "only test on nightly for consistent error messages"
)]
#[test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: extern static `pidfd_spawnp` is not supported by Miri"
)]
fn diagnostic_on_unimplemented() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/unimplemented_db_model.rs");
    t.compile_fail("tests/ui/unimplemented_request_handler.rs");
    t.compile_fail("tests/ui/unimplemented_form.rs");
    t.compile_fail("tests/ui/unimplemented_admin_model.rs");
}
