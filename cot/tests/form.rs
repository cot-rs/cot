use cot::db::{Auto, ForeignKey};
use cot::form::fields::{SelectChoice, SelectField};
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

#[derive(Debug, Clone, PartialEq)]
enum Priority {
    Low,
    Medium,
    High,
}

impl SelectChoice for Priority {
    fn default_choices() -> Vec<Self> {
        vec![Self::Low, Self::Medium, Self::High]
    }

    fn from_str(s: &str) -> Result<Self, FormFieldValidationError> {
        match s {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            _ => Err(FormFieldValidationError::invalid_value(s.to_owned())),
        }
    }

    fn id(&self) -> String {
        match self {
            Self::Low => "low".to_string(),
            Self::Medium => "medium".to_string(),
            Self::High => "high".to_string(),
        }
    }

    fn to_string(&self) -> String {
        match self {
            Self::Low => "Low Priority".to_string(),
            Self::Medium => "Medium Priority".to_string(),
            Self::High => "High Priority".to_string(),
        }
    }
}

impl AsFormField for Priority {
    type Type = SelectField<Self>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        if let Some(value) = field.value() {
            if value.is_empty() {
                return Err(FormFieldValidationError::Required);
            }
            Self::from_str(value)
        } else {
            Err(FormFieldValidationError::Required)
        }
    }

    fn to_field_value(&self) -> String {
        self.id()
    }
}

#[derive(Debug, Form)]
struct SimpleTaskForm {
    title: String,
    priority: Priority,
}

#[cot::test]
async fn select_field_form_integration() {
    let mut request = TestRequestBuilder::post("/")
        .form_data(&[("title", "Complete project"), ("priority", "high")])
        .build();

    let form = SimpleTaskForm::from_request(&mut request)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(form.title, "Complete project");
    assert_eq!(form.priority, Priority::High);
}

#[cot::test]
async fn select_field_validation_error() {
    let mut request = TestRequestBuilder::post("/")
        .form_data(&[("title", "Test task"), ("priority", "invalid_priority")])
        .build();

    let form = SimpleTaskForm::from_request(&mut request).await;
    match form {
        Ok(FormResult::ValidationError(context)) => {
            assert_eq!(context.errors_for(FormErrorTarget::Form), &[]);
            assert_eq!(context.errors_for(FormErrorTarget::Field("title")), &[]);
            assert_eq!(
                context.errors_for(FormErrorTarget::Field("priority")),
                &[FormFieldValidationError::InvalidValue(
                    "invalid_priority".to_string()
                )]
            );
        }
        _ => panic!("Expected a validation error"),
    }
}

#[cot::test]
async fn select_field_context_display() {
    let mut request = TestRequestBuilder::get("/").build();

    let context = SimpleTaskForm::build_context(&mut request).await.unwrap();
    let form_rendered = context.to_string();

    assert!(form_rendered.contains("<select"));
    assert!(form_rendered.contains("name=\"priority\""));
    assert!(form_rendered.contains("Low Priority"));
    assert!(form_rendered.contains("Medium Priority"));
    assert!(form_rendered.contains("High Priority"));
    assert!(form_rendered.contains("value=\"low\""));
    assert!(form_rendered.contains("value=\"medium\""));
    assert!(form_rendered.contains("value=\"high\""));
}
