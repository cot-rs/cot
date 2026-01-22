---
title: Templates
---

Cot does not require you to use any specific templating engine. However, it provides a convenient integration with a powerful engine called [Askama](https://askama.readthedocs.io/). Askama is very similar to Jinja2, which itself was inspired by Django's template engine. It allows you to build complex templates easily while providing type safety to help catch errors at compile time.

## Basic Syntax

An Askama template is simply a text file that includes both static text and dynamic content. The dynamic content is introduced using variables, tags, and filters. Below is a simple Askama template:

```html.j2
<ul>
    {% for item in items %}
    <li>{{ item.title|capitalize }}</li>
    {% endfor %}
</ul>
```

We can identify the following core syntax elements:

- **`{% ... %}` (tags)**: Used to control template logic, such as loops and conditionals. In the example above, `for item in items` iterates over a collection named `items`.
- **`{{ ... }}` (variables)**: Used to output dynamic data into the template.
- **`|capitalize` (filters)**: Modify the output of variables (e.g., `capitalize` converts the first character to uppercase). You can chain multiple filters if needed.

An example of the rendered output (ignoring whitespace) might be:

```html
<ul>
    <li>First item</li>
    <li>Second item</li>
    <li>Third item</li>
</ul>
```

To make variables like `items` available in the template, you need to define them in your Rust code and pass them into the template.

### Example

Here is a simple demonstration of templating with Askama in Cot:

```rust
use cot::request::Request;
use cot::response::{Response, ResponseExt};
use cot::{Body, StatusCode};
use cot::Template;

struct Item {
    title: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    items: Vec<Item>,
}

async fn index() -> cot::Result<Html> {
    let items = vec![
        Item { title: "first item".to_string() },
        Item { title: "second item".to_string() },
        Item { title: "third item".to_string() },
    ];

    let context = IndexTemplate { items };
    let rendered = context.render()?;

    Ok(Html::new(rendered))
}
```

## Template Inheritance

A common approach when using templates is to employ *template inheritance*. This technique lets you define a base template for shared structure and layout, and then create child templates that only override the pieces that need to differ. Askama supports this via two main concepts:

- **`{% extends %}`**: Tells Askama which template the current file extends (the "parent" template). This tag must appear first in the file.
- **`{% block %}`**: Defines a named section in the parent template that child templates can override. By default, the block includes whatever is in the parent, but child templates may completely replace it.

### Example

`base.html`:

```html.j2
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <title>{% block title %}My Site{% endblock %}</title>
</head>
<body>
    <header>
        <h1>{% block header %}My Site{% endblock %}</h1>
    </header>
    <main>
        {% block content %}{% endblock %}
    </main>
</body>
</html>
```

`index.html`:

```html.j2
{% extends "base.html" %}

{% block title %}Home{% endblock %}
{% block header %}Welcome to my site!{% endblock %}

{% block content %}
<p>This is the content of the home page.</p>
{% endblock %}
```

When you render `index.html`, it uses the overall structure from `base.html` but replaces the `title`, `header`, and `content` blocks with its own content.

## Including Templates

Beyond inheritance, Askama also supports *including* other templates. This is useful for reusing small, self-contained pieces of content across multiple pages. You can include a template with the `{% include %}` tag.

### Defining Variables

Any template included via `{% include %}` has access to the parent template's variables. Additionally, you can define new variables with the `{% let %}` tag. Askama's variables behave like Rust variables: they are immutable by default, but you can shadow an existing variable with the same name.

### Example

`hello.html`:

```html.j2
<p>Hello, {{ name }}!</p>
```

`index.html`:

```html.j2
{% let name = "Alice" %}
{% include "hello.html" %}
{% let name = "Bob" %}
{% include "hello.html" %}
```

Rendered output:

```html
<p>Hello, Alice!</p>
<p>Hello, Bob!</p>
```

## URLs

Linking to other pages in your application is a frequent requirement, and hardcoding URLs in templates can become a maintenance hassle. To address this, Cot provides the `cot::reverse!()` macro. This macro generates URLs based on your route definitions, validating that you’ve passed any required parameters and that the route actually exists. If you ever change your URL structure, you'll only need to update the route definitions.

`cot::reverse!()` expects a reference to the `Urls` object (which you can obtain by extracting it from the request), the route name, and any parameters needed by that route.

### Example

`index.html`:

```html.j2
{% let urls = urls %}
<a href="{{ cot::reverse!(urls, "index") }}">Home</a>
<a href="{{ cot::reverse!(urls, "user", id=42) }}">User 42</a>
```

`main.rs`:

```rust
use cot::request::Request;
use cot::response::{Response, ResponseExt};
use cot::router::{Router, Route, Urls};
use cot::{Body, StatusCode};
use cot::Template;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    urls: &'a Urls,
}

async fn index(urls: Urls) -> cot::Result<Html> {
    let template = IndexTemplate { urls: &urls };

    Ok(Html::new(template.render()?))
}

async fn user() -> cot::Result<Response> {
    todo!()
}

struct CotTestApp;

impl cot::App for CotTestApp {
    fn name(&self) -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", index, "index"),
            Route::with_handler_and_name("/user/{id}", user, "user"),
        ])
    }
}
```

## Control Flow and Logic

Askama offers several tags that let you control how the template renders and apply logic. Here are the most commonly used ones:

### If

Use the `{% if %}` tag to conditionally render parts of the template based on a certain condition. For more complex scenarios, you can include `{% elif %}` or an `{% else %}` section.

#### Example

```html.j2
{% if user.is_admin %}
    Welcome, admin!
{% elif user.is_logged_in %}
    Welcome, user!
{% else %}
    Please log in to continue.
{% endif %}
```

### Match

The `{% match %}` tag matches a value against a set of Rust patterns. Use `{% when %}` to specify each pattern and its corresponding content.

#### Example

```html.j2
{% match user.role %}
    {% when Some with ("admin") %}
        Welcome, admin!
    {% when Some %}
        Welcome, user!
    {% when None %}
{% endmatch %}
```

### For

The `{% for %}` tag allows you to iterate over a sequence of items. Inside the loop, Askama provides helpful variables such as:

- `loop.index`: Current iteration (1-indexed).
- `loop.index0`: Current iteration (0-indexed).
- `loop.first`: `true` on the first iteration.
- `loop.last`: `true` on the last iteration.

#### Example

```html.j2
<ul>
    {% for item in items %}
    <li>{{ loop.index }}. {{ item }}</li>
    {% endfor %}
</ul>
```

## Whitespace Control

By default, Askama preserves all whitespace, which can sometimes cause unwanted gaps in your output when using loops or conditionals. To manage this, you can use the `-` modifier before or after a tag to trim surrounding whitespace.

### Example

```html.j2
<ul>
    {%- for item in items -%}
    <li>{{ item }}</li>
    {%- endfor -%}
</ul>
```

This usage of `-` ensures that no extra whitespace or blank lines appear around the `<li>` tags.

## Comments

You can include comments in your Askama templates using `{# ... #}`. These comments are ignored in the rendered output and can be used to document logic or temporarily disable parts of your template. They also support whitespace control via `-`.

### Example

```html.j2
{# This is a comment #}
This will be rendered.
{#-
This is a multi-line comment
which won’t appear in the output.
-#}
```

## Custom Renderable Types

To display custom types in Askama templates, the type must implement `Display`. This makes the type's `to_string()` output available in the template.

### Example

`main.rs`:

```rust
use std::fmt::Display;
use cot::Template;

struct Item {
    title: String,
}

impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    item: Item,
}
```

`index.html`:

```html.j2
{{ item }}
```

When rendered, it will display the `title` from the `Item` struct.

### HTML Escaping

By default, Askama escapes all output to protect against XSS attacks. Special characters are replaced with their HTML entities. If you’re certain your data is safe and want to bypass escaping, you can implement the `HtmlSafe` marker trait.

```rust
use askama::filters::HtmlSafe;

impl HtmlSafe for Item {}
```

Be very cautious when marking output as safe; you are responsible for ensuring that the content doesn’t introduce security risks.

To simplify generating safe HTML in Rust, Cot provides the [`HtmlTag`](https://docs.rs/cot/0.5/cot/html/struct.HtmlTag.html) type. It automatically applies escaping where necessary.

```rust
impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("text");
        tag.attr("value", &self.title); // The title will be safely escaped here

        write!(f, "{}", tag.render().as_str())
    }
}
```

## Read More

This chapter only covers the basics of Askama. For more detailed information, advanced usage, and additional examples, check out the [Askama documentation](https://askama.readthedocs.io/).
