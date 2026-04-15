use cot::db::query::{Expr, ExprAdd, ExprDiv, ExprEq, ExprMul, ExprOrd, ExprSub, Query};
use cot::db::{model, query};

#[model]
#[derive(Debug, PartialEq)]
struct MyModel {
    #[model(primary_key)]
    id: i32,
    name: String,
    price: i64,
    quantity: i64
}

#[test]
fn test_query_equality() {
    assert_eq!(
        Query::<MyModel>::new().filter(
            ExprEq::eq(
                <MyModel as cot::db::Model>::Fields::id,
                5
            )
        ),
        query!(MyModel, $id == 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            ExprEq::ne(
                <MyModel as cot::db::Model>::Fields::id,
                5
            )
        ),
        query!(MyModel, $id != 5)
    );
}

#[test]
fn test_query_comparison() {
    assert_eq!(
        Query::<MyModel>::new().filter(
            ExprOrd::lt(
                <MyModel as cot::db::Model>::Fields::id,
                5
            )
        ),
        query!(MyModel, $id < 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            ExprOrd::lte(
                <MyModel as cot::db::Model>::Fields::id,
                5
            )
        ),
        query!(MyModel, $id <= 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            ExprOrd::gt(
                <MyModel as cot::db::Model>::Fields::id,
                5
            )
        ),
        query!(MyModel, $id > 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            ExprOrd::gte(
                <MyModel as cot::db::Model>::Fields::id,
                5
            )
        ),
        query!(MyModel, $id >= 5)
    );
    
    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::and(
                ExprEq::eq(
                    <MyModel as cot::db::Model>::Fields::id,
                    5
                ),
                 ExprEq::eq(
                    <MyModel as cot::db::Model>::Fields::name,
                    "test"
                 )
            )
        ),
        query!(MyModel, $id == 5 && $name == "test")
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::or(
                ExprEq::eq(
                    <MyModel as cot::db::Model>::Fields::id,
                    5
                ),
                 ExprEq::eq(
                    <MyModel as cot::db::Model>::Fields::id,
                    10
                 )
            )
        ),
        query!(MyModel, $id == 5 || $id == 10)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::and(
                ExprOrd::gt(
                    <MyModel as cot::db::Model>::Fields::id,
                    0
                ),
                 Expr::or(
                    ExprEq::eq(
                        <MyModel as cot::db::Model>::Fields::name,
                        "a"
                    ),
                     ExprEq::eq(
                        <MyModel as cot::db::Model>::Fields::name,
                        "b"
                     )
                 )
            )
        ),
        query!(MyModel, $id > 0 && ($name == "a" || $name == "b"))
    );
}

#[test]
fn test_query_arithmetic() {
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            Expr::eq(
                <MyModel as ::cot::db::Model>::Fields::id.as_expr(),
                ExprAdd::add(
                    <MyModel as ::cot::db::Model>::Fields::id,
                    5)
            )
        ),
        query!(MyModel, $id == $id + 5)
    );
    

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::eq(
                Expr::field("price"),
                Expr::add(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                )
            )
        ),
        query!(MyModel, $price == $quantity + $id)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::gt(
                Expr::add(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                ),
                Expr::value(11)
            )
        ),
        query!(MyModel, $quantity + $id > 11)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::add(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                <MyModel as cot::db::Model>::Fields::id.as_expr()
            )
        ),
        query!(MyModel, $quantity + $id)
    );
    

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::eq(
                Expr::field("id"),
                <MyModel as cot::db::Model>::Fields::id.sub(5)
            )
        ),
        query!(MyModel, $id == $id - 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::eq(
                Expr::field("price"),
                Expr::sub(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                )
            )
        ),
        query!(MyModel, $price == $quantity - $id)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::gt(
                Expr::sub(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                ),
                Expr::value(11)
            )
        ),
        query!(MyModel, $quantity - $id > 11)
    );
    
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            ExprSub::sub(
                <MyModel as ::cot::db::Model>::Fields::quantity,
                1
            )
        ),
        query!(MyModel, $quantity - 1)
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            Expr::eq(
                <MyModel as ::cot::db::Model>::Fields::id.as_expr(),
                ExprMul::mul(
                    <MyModel as ::cot::db::Model>::Fields::id,
                    5
                )
            )
        ),
        query!(MyModel, $id == $id * 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::eq(
                Expr::field("price"),
                Expr::mul(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                )
            )
        ),
        query!(MyModel, $price == $quantity * $id)
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            Expr::gt(
                Expr::mul(
                    <MyModel as ::cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as ::cot::db::Model>::Fields::id.as_expr()
                ), 
                Expr::value(11)
            )
        ),
        query!(MyModel, $quantity * $id > 11)
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            ::cot::db::query::ExprMul::mul(
                <MyModel as ::cot::db::Model>::Fields::quantity,
                5i64
            )
        ),
        query!(MyModel, $quantity * 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::eq(
                Expr::field("id"), 
                <MyModel as cot::db::Model>::Fields::id.div(5)
            )
        ),
        query!(MyModel, $id == $id / 5)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::eq(
                Expr::field("price"),
                Expr::div(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                )
            )
        ),
        query!(MyModel, $price == $quantity / $id)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::gt(
                Expr::div(
                    <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                    <MyModel as cot::db::Model>::Fields::id.as_expr()
                ),
                Expr::value(11)
            )
        ),
        query!(MyModel, $quantity / $id > 11)
    );

    assert_eq!(
        Query::<MyModel>::new().filter(
            Expr::div(
                <MyModel as cot::db::Model>::Fields::quantity.as_expr(),
                Expr::value(1i64)
            )
        ),
        query!(MyModel, $quantity / 1)
    );

}

#[test]
fn test_query_rust_expressions() {
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            ExprEq::eq(
                <MyModel as ::cot::db::Model>::Fields::id,
                10
            )
        ),
        query!(MyModel, $id == 10)
    );

    struct Outer {
        inner: i32,
    }
    let outer = Outer { inner: 20 };
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            ExprEq::eq(
                <MyModel as ::cot::db::Model>::Fields::id,
                outer.inner)
        ),
        query!(MyModel, $id == outer.inner)
    );

    fn get_id() -> i32 {
        30
    }
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            ExprEq::eq(
                <MyModel as ::cot::db::Model>::Fields::id,
                get_id()
            )
        ),
        query!(MyModel, $id == get_id())
    );

    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            <MyModel as ::cot::db::Model>::Fields::id.as_expr()
        ),
        query!(MyModel, $id)
    );
}

#[test]
fn test_query_path_access() {
    mod constants {
        pub(crate) const ID: i32 = 100;
    }
    assert_eq!(
        <MyModel as ::cot::db::Model>::objects().filter(
            ExprEq::eq(
                <MyModel as ::cot::db::Model>::Fields::id,
                constants::ID
            )
        ),
        query!(MyModel, $id == constants::ID)
    );
}
