use cot::db::query::Query;
use cot::db::query::expr::{Expr, ExprAdd, ExprDiv, ExprEq, ExprLike, ExprMul, ExprOrd, ExprSub};
use cot::db::{model, query};

#[model]
#[derive(Debug, PartialEq)]
struct MyModel {
    #[model(primary_key)]
    id: i32,
    name: String,
    title: String,
    price: i64,
    quantity: i64,
    valid: bool,
}

#[test]
fn test_query_equality() {
    assert_eq!(
        Query::<MyModel>::new().filter(ExprEq::eq(<MyModel as cot::db::Model>::Fields::id, 5)),
        query!(MyModel, $id == 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprEq::ne(<MyModel as cot::db::Model>::Fields::id, 5)),
        query!(MyModel, $id != 5)
    );
}

#[test]
fn test_query_comparison() {
    assert_eq!(
        Query::<MyModel>::new().filter(ExprOrd::lt(<MyModel as cot::db::Model>::Fields::id, 5)),
        query!(MyModel, $id < 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprOrd::lte(<MyModel as cot::db::Model>::Fields::id, 5)),
        query!(MyModel, $id <= 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprOrd::gt(<MyModel as cot::db::Model>::Fields::id, 5)),
        query!(MyModel, $id > 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprOrd::gte(<MyModel as cot::db::Model>::Fields::id, 5)),
        query!(MyModel, $id >= 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::and(
            ExprEq::eq(<MyModel as cot::db::Model>::Fields::id, 5),
            ExprEq::eq(<MyModel as cot::db::Model>::Fields::name, "test")
        )),
        query!(MyModel, $id == 5 && $name == "test")
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::or(
            ExprEq::eq(<MyModel as cot::db::Model>::Fields::id, 5),
            ExprEq::eq(<MyModel as cot::db::Model>::Fields::id, 10)
        )),
        query!(MyModel, $id == 5 || $id == 10)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::and(
            ExprOrd::gt(<MyModel as cot::db::Model>::Fields::id, 0),
            Expr::or(
                ExprEq::eq(<MyModel as cot::db::Model>::Fields::name, "a"),
                ExprEq::eq(<MyModel as cot::db::Model>::Fields::name, "b")
            )
        )),
        query!(MyModel, $id > 0 && ($name == "a" || $name == "b"))
    );
}

#[test]
fn test_query_add_fields() {
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(Expr::eq(
            <MyModel as ::cot::db::Model>::Fields::id.as_expr(),
            ExprAdd::add(<MyModel as ::cot::db::Model>::Fields::id, 5)
        )),
        query!(MyModel, $id == $id + 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            Expr::field("price"),
            Expr::add(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            )
        )),
        query!(MyModel, $price == $quantity + $id)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::gt(
            Expr::add(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            ),
            Expr::value(11)
        )),
        query!(MyModel, $quantity + $id > 11)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::add(
            <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
            <MyModel as cot::db::Model>::Fields::id.as_expr()
        )),
        query!(MyModel, $quantity + $id)
    );
}

#[test]
fn test_query_sub_fields() {
    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            Expr::field("id"),
            <MyModel as cot::db::Model>::Fields::id.sub(5)
        )),
        query!(MyModel, $id == $id - 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            Expr::field("price"),
            Expr::sub(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            )
        )),
        query!(MyModel, $price == $quantity - $id)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::gt(
            Expr::sub(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            ),
            Expr::value(11)
        )),
        query!(MyModel, $quantity - $id > 11)
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(ExprSub::sub(
            <MyModel as ::cot::db::Model>::Fields::quantity,
            1
        )),
        query!(MyModel, $quantity - 1)
    );
}

#[test]
fn test_query_mul_fields() {
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(Expr::eq(
            <MyModel as ::cot::db::Model>::Fields::id.as_expr(),
            ExprMul::mul(<MyModel as ::cot::db::Model>::Fields::id, 5)
        )),
        query!(MyModel, $id == $id * 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            Expr::field("price"),
            Expr::mul(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            )
        )),
        query!(MyModel, $price == $quantity * $id)
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(Expr::gt(
            Expr::mul(
                <MyModel as ::cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as ::cot::db::Model>::Fields::id.as_expr()
            ),
            Expr::value(11)
        )),
        query!(MyModel, $quantity * $id > 11)
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(::cot::db::query::expr::ExprMul::mul(
            <MyModel as ::cot::db::Model>::Fields::quantity,
            5i64
        )),
        query!(MyModel, $quantity * 5)
    );
}

#[test]
fn test_query_div_fields() {
    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            Expr::field("id"),
            <MyModel as cot::db::Model>::Fields::id.div(5)
        )),
        query!(MyModel, $id == $id / 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            Expr::field("price"),
            Expr::div(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            )
        )),
        query!(MyModel, $price == $quantity / $id)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::gt(
            Expr::div(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            ),
            Expr::value(11)
        )),
        query!(MyModel, $quantity / $id > 11)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::div(
            <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
            Expr::value(1i64)
        )),
        query!(MyModel, $quantity / 1)
    );
}

struct Outer {
    inner: i32,
}

#[test]
fn test_query_rust_expressions() {
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects()
            .filter(ExprEq::eq(<MyModel as ::cot::db::Model>::Fields::id, 10)),
        query!(MyModel, $id == 10)
    );

    let outer = Outer { inner: 20 };
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(ExprEq::eq(
            <MyModel as ::cot::db::Model>::Fields::id,
            outer.inner
        )),
        query!(MyModel, $id == outer.inner)
    );

    let get_id = || 30;

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(ExprEq::eq(
            <MyModel as ::cot::db::Model>::Fields::id,
            get_id()
        )),
        query!(MyModel, $id == get_id())
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects()
            .filter(<MyModel as ::cot::db::Model>::Fields::id.as_expr()),
        query!(MyModel, $id)
    );
}

#[test]
fn test_query_path_access() {
    mod constants {
        pub(crate) const ID: i32 = 100;
    }
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(ExprEq::eq(
            <MyModel as ::cot::db::Model>::Fields::id,
            constants::ID
        )),
        query!(MyModel, $id == constants::ID)
    );
}

#[test]
fn test_query_string_methods_on_bare_field() {
    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::contains(
            <MyModel as cot::db::Model>::Fields::name,
            "foo"
        )),
        query!(MyModel, $name.contains("foo"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::icontains(
            <MyModel as cot::db::Model>::Fields::name,
            "FOO"
        )),
        query!(MyModel, $name.icontains("FOO"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::starts_with(
            <MyModel as cot::db::Model>::Fields::name,
            "foo"
        )),
        query!(MyModel, $name.starts_with("foo"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::istarts_with(
            <MyModel as cot::db::Model>::Fields::name,
            "FOO"
        )),
        query!(MyModel, $name.istarts_with("FOO"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::ends_with(
            <MyModel as cot::db::Model>::Fields::name,
            "bar"
        )),
        query!(MyModel, $name.ends_with("bar"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::iends_with(
            <MyModel as cot::db::Model>::Fields::name,
            "BAR"
        )),
        query!(MyModel, $name.iends_with("BAR"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::raw_like(
            <MyModel as cot::db::Model>::Fields::name,
            "f??o"
        )),
        query!(MyModel, $name.raw_like("f??o"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(ExprLike::iraw_like(
            <MyModel as cot::db::Model>::Fields::name,
            "F??O"
        )),
        query!(MyModel, $name.iraw_like("F??O"))
    );
}

#[test]
fn test_query_string_method_composite_receiver_falls_back_to_expr() {
    assert_eq!(
        Query::<MyModel>::new().filter(Expr::contains(
            Expr::add(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            ),
            Expr::value("50")
        )),
        query!(MyModel, ($quantity + $id).contains("50"))
    );

    assert_eq!(
        Query::<MyModel>::new().filter(Expr::ends_with(
            Expr::add(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            ),
            Expr::value("0")
        )),
        query!(MyModel, ($quantity + $id).ends_with("0"))
    );
}

#[test]
fn test_query_string_method_combined_with_boolean_ops() {
    assert_eq!(
        Query::<MyModel>::new().filter(Expr::and(
            ExprLike::contains(<MyModel as cot::db::Model>::Fields::name, "foo"),
            Expr::gt(
                <MyModel as cot::db::Model>::Fields::id.as_expr(),
                Expr::value(0)
            )
        )),
        query!(MyModel, $name.contains("foo") && $id > 0)
    );
}

#[test]
fn test_query_string_method_string_concat_field_refs() {
    assert_eq!(
        Query::<MyModel>::new().filter(Expr::contains(
            Expr::add(
                <MyModel as cot::db::Model>::Fields::name.as_expr(),
                <MyModel as cot::db::Model>::Fields::title.as_expr()
            ),
            Expr::value("foo")
        )),
        query!(MyModel, ($name + $title).contains("foo"))
    );
}

#[test]
fn test_query_string_method_non_field_receiver_call() {
    let allowed_names = &["foo", "bar"];
    assert_eq!(
        Query::<MyModel>::new().filter(Expr::eq(
            <MyModel as cot::db::Model>::Fields::valid.as_expr(),
            Expr::value(true)
        )),
        query!(MyModel, $valid == allowed_names.contains(&"foo"))
    );
}
