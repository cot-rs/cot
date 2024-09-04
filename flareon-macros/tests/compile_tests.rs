#[rustversion::attr(not(nightly), ignore)]
#[test]
fn test_derive_form() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/derive_form.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
fn test_attr_model() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/attr_model.rs");
    t.compile_fail("tests/ui/attr_model_migration_invalid_name.rs");
}

#[rustversion::attr(not(nightly), ignore)]
#[test]
fn test_func_query() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/func_query.rs");
    t.compile_fail("tests/ui/func_query_double_op.rs");
    t.compile_fail("tests/ui/func_query_starting_op.rs");
    t.compile_fail("tests/ui/func_query_double_field.rs");
    t.compile_fail("tests/ui/func_query_invalid_field.rs");
}
