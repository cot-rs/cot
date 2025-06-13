#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn derive_form() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/derive_form.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn attr_model() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/attr_model.rs");
    t.compile_fail("tests/ui/attr_model_migration_invalid_name.rs");
    t.compile_fail("tests/ui/attr_model_tuple.rs");
    t.compile_fail("tests/ui/attr_model_enum.rs");
    t.compile_fail("tests/ui/attr_model_generic.rs");
    t.compile_fail("tests/ui/attr_model_no_pk.rs");
    t.compile_fail("tests/ui/attr_model_multiple_pks.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn derive_admin_model() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/derive_admin_model.rs");
    t.pass("tests/ui/derive_admin_model_derive_first.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn func_query() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/func_query.rs");
    t.compile_fail("tests/ui/func_query_double_op.rs");
    t.compile_fail("tests/ui/func_query_starting_op.rs");
    t.compile_fail("tests/ui/func_query_double_field.rs");
    t.compile_fail("tests/ui/func_query_invalid_field.rs");
    t.compile_fail("tests/ui/func_query_method_call_on_db_field.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn attr_main() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/attr_main.rs");
    t.compile_fail("tests/ui/attr_main_args.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn derive_from_struct() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/derive_from_request_parts.rs");
    t.compile_fail("tests/ui/derive_from_request_parts_enum.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn derive_select_choice() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/derive_select_choice.rs");
}
