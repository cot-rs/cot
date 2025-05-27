use cot::db::{Auto, ForeignKey};
use cot::form::{
    AsFormField, Form, FormContext, FormErrorTarget, FormField, FormFieldValidationError,
    FormResult,
};
use cot::test::TestRequestBuilder;
use cot_macros::model;

#[derive(Debug, Form)]
struct MyForm {
    name: String,
    address: Option<String>,
    age: u8,
}

#[cot::test]
async fn context_from_empty_request() {
    let mut request = TestRequestBuilder::get("/").build();

    let context = MyForm::build_context(&mut request).await;
    assert!(context.is_ok());
}

#[cot::test]
async fn context_display_non_empty() {
    let mut request = TestRequestBuilder::get("/").build();

    let context = MyForm::build_context(&mut request).await.unwrap();
    let form_rendered = context.to_string();
    assert!(!form_rendered.is_empty());
}

#[cot::test]
async fn form_from_request() {
    let mut request = TestRequestBuilder::post("/")
        .form_data(&[("name", "Alice"), ("age", "30")])
        .build();

    let form = MyForm::from_request(&mut request).await.unwrap().unwrap();
    assert_eq!(form.name, "Alice");
    assert_eq!(form.address, None);
    assert_eq!(form.age, 30);
}

#[cot::test]
async fn form_errors_required() {
    let mut request = TestRequestBuilder::post("/")
        .form_data::<String>(&[])
        .build();

    let form = MyForm::from_request(&mut request).await;
    match form {
        Ok(FormResult::ValidationError(context)) => {
            assert_eq!(context.errors_for(FormErrorTarget::Form), &[]);
            assert_eq!(
                context.errors_for(FormErrorTarget::Field("name")),
                &[FormFieldValidationError::Required]
            );
            assert_eq!(context.errors_for(FormErrorTarget::Field("address")), &[]);
            assert_eq!(
                context.errors_for(FormErrorTarget::Field("age")),
                &[FormFieldValidationError::Required]
            );
        }
        _ => panic!("Expected a validation error"),
    }
}

#[cot::test]
async fn values_persist_on_form_errors() {
    let mut request = TestRequestBuilder::post("/")
        .form_data(&[("name", "Alice"), ("age", "invalid")])
        .build();

    let form = MyForm::from_request(&mut request).await;
    match form {
        Ok(FormResult::ValidationError(context)) => {
            assert_eq!(context.name.value(), Some("Alice"));
            assert_eq!(context.age.value(), Some("invalid"));

            assert_eq!(context.errors_for(FormErrorTarget::Form), &[]);
            assert_eq!(context.errors_for(FormErrorTarget::Field("name")), &[]);
            assert_eq!(context.errors_for(FormErrorTarget::Field("address")), &[]);
            assert_eq!(
                context.errors_for(FormErrorTarget::Field("age")),
                &[FormFieldValidationError::InvalidValue(
                    "invalid".to_string()
                )]
            );
        }
        _ => panic!("Expected a validation error"),
    }
}

#[cot::test]
async fn foreign_key_field() {
    #[model]
    struct TestModel {
        #[model(primary_key)]
        name: String,
    }

    #[derive(Form)]
    struct TestModelForm {
        test_field: ForeignKey<TestModel>,
    }

    // test field rendering
    let context = TestModelForm::build_context(&mut TestRequestBuilder::get("/").build())
        .await
        .unwrap();
    let form_rendered = context.to_string();
    assert!(form_rendered.contains("test_field"));
    assert!(form_rendered.contains("type=\"text\""));

    // test form data
    let mut request = TestRequestBuilder::post("/")
        .form_data(&[("test_field", "Alice")])
        .build();
    let form = TestModelForm::from_request(&mut request).await;
    match form {
        Ok(FormResult::Ok(instance)) => {
            assert_eq!(instance.test_field.primary_key(), "Alice");
        }
        _ => panic!("Expected a valid form"),
    }

    // test re-raising validation errors
    let mut request = TestRequestBuilder::post("/")
        .form_data(&[("test_field", "")])
        .build();
    let form = TestModelForm::from_request(&mut request).await;
    match form {
        Ok(FormResult::ValidationError(context)) => {
            assert_eq!(
                context.errors_for(FormErrorTarget::Field("test_field")),
                &[FormFieldValidationError::Required]
            );
        }
        _ => panic!("Expected a validation error"),
    }
}

#[cot::test]
async fn foreign_key_field_to_field_value() {
    #[model]
    struct TestModel {
        #[model(primary_key)]
        id: Auto<i32>,
    }

    let field_value = ForeignKey::<TestModel>::Model(Box::new(TestModel {
        id: Auto::fixed(123),
    }))
    .to_field_value();
    assert_eq!(field_value, "123");

    let field_value = ForeignKey::<TestModel>::PrimaryKey(Auto::fixed(456)).to_field_value();
    assert_eq!(field_value, "456");
}
