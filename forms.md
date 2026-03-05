---
title: Forms
---

Cot has form processing capabilities that allows you to create forms and handle form submissions with ease. Processing forms is as easy as creating a Rust structure, deriving a trait, and then using one function to process the form using the request data. Cot will automatically validate the form data and handle any errors that occur.

## Form trait

The core of the form processing lies in the `Form` trait inside the `cot::form` module. This trait is used to define the form and the fields that are part of the form. Below is an example of how you can define a form:

```rust
use cot::form::Form;

#[derive(Form)]
struct ContactForm {
    name: String,
    email: String,
    #[form(opts(max_length = 1000))]
    message: String,
}
```

And here is how you can process the form inside a request handler:

```rust
use cot::form::{Form, FormResult};
use cot::html::Html;
use cot::request::{Request, RequestExt};
use cot::response::{Response, ResponseExt};

async fn contact(mut request: Request) -> cot::Result<Response> {
    // Handle POST request (form submission)
    if request.method() == Method::POST {
        match ContactForm::from_request(&mut request).await? {
            FormResult::Ok(form) => {
                // Form is valid! Process the data
                println!("Message from {}: {}", form.name, form.message);

                // Redirect after successful submission
                Ok(reverse_redirect!(request, "thank_you")?)
            }
            FormResult::ValidationError(context) => {
                // Form has errors - render the template with error messages
                let template = ContactTemplate {
                    request: &request,
                    form: context,
                };
                Ok(Html::new(template.render()?).into())
            }
        }
    } else {
        // Handle GET request (display empty form)
        let template = ContactTemplate {
            request: &request,
            form: ContactForm::build_context(&mut request).await?,
        };

        Ok(Html::new(template.render()?).into())
    }
}
```

### Forms in templates

Before this is really usable, we need to define the form in the HTML template. Thankfully, Cot provides you with a way to implement this easily, too—it can automatically generate the HTML for the form based on the form definition.

There are several ways how can you use the forms in your templates. The easiest one is to use the form directly in the template:

```html.j2
{% let request = request %}

<form method="post" action="">
    {{ form }}

    <button type="submit">Submit</button>
</form>
```

This is especially useful for prototyping new forms, as it doesn't allow you to customize the rendering of your form. If you need a bit more control, you can use the `form.fields()` method to render the fields individually:

```html.j2
{% let request = request %}

<form method="post" action="">
    {% for field in form.fields() %}
        <div class="form-group">
            <label for="{{ field.dyn_id() }}">{{ field.dyn_options().name }}</label>
            {{ field|safe }}

            <ul class="errors">
            {% for error in form_context.errors_for(FormErrorTarget::Field(field.dyn_id())) %}
                <li>{{ error }}</li>
            {% endfor %}
            </ul>
        </div>
    {% endfor %}

    <button type="submit">Submit</button>
</form>
```

It is recommended to reuse the template code for rendering the form fields using `{% include %}` to make it easy to achieve a consistent look and feel across your application.

## Field validation

Cot provides several ways to validate form data:

### Built-in validation

```rust
#[derive(Form)]
struct ArticleForm {
    // Maximum length validation
    #[form(opts(max_length = 100))]
    title: String,

    // Required checkbox
    #[form(opts(must_be_true = true))]
    confirm_publish: bool,
}
```

### Custom validation

You can implement custom validation by handling the validation result:

```rust
async fn handle_form(mut request: Request) -> cot::Result<Response> {
    match ArticleForm::from_request(&mut request).await? {
        FormResult::Ok(form) => {
            // Add custom validation
            if form.title.to_lowercase().contains("spam") {
                let mut context = ArticleForm::build_context(&mut request).await?;
                context.add_error(
                    FormErrorTarget::Field("title"),
                    FormFieldValidationError::from_static("Title contains spam")
                );

                // Re-render form with error
                return Ok(Html::new(render_template(context)?).into());
            }

            // Process valid form...
            Ok(reverse_redirect!(request, "success")?)
        }
        FormResult::ValidationError(context) => {
            // Handle validation errors...
            Ok(Html::new(render_template(context)?).into())
        }
    }
}
```

## Summary

In this chapter you learned how to handle forms and validate form data in Cot applications. Remember:

* Always validate form data server-side
* Provide clear error messages
* Use appropriate field types
* Consider user experience in form layout
* Handle both GET and POST requests appropriately

In the next chapter, we'll explore database models and how you can use them to persist data in your services.
