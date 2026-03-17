---
title: Introduction
---

[bacon]: https://dystroy.org/bacon/

Cot is a free and open-source web framework for Rust that makes building web applications both fun and reliable. Taking inspiration from [Django](https://www.djangoproject.com/)'s developer-friendly approach, Cot combines Rust's safety guarantees with rapid development features that help you build secure web applications quickly. Whether you're coming from Django or are new to web development entirely, you'll find Cot's intuitive design helps you be productive from day one.

## Who is this guide for?

This guide doesn't assume any advanced knowledge in Rust or web development in general (although this will help, too!). It's aimed at beginners who are looking to get started with Cot, and will guide you through the process of setting up a new project, creating your first views, using the Cot ORM and running your application.

If you are not familiar with Rust, you might want to start by reading the [Rust Book](https://doc.rust-lang.org/book/), which is an excellent resource for learning Rust.

## Installing and running Cot CLI

Let's get your first Cot project up and running! First, you'll need Cargo, Rust's package manager. If you don't have it installed, you can get it through [rustup](https://rustup.rs/). Cot requires Rust version 1.88 or later.

Install the Cot CLI with:

```bash
cargo install --locked cot-cli
```

Now create your first project:

```bash
cot new cot_tutorial
```

This creates a new directory called `cot_tutorial` with a new Cot project inside. Let's explore what Cot has created for us:

```bash
cot_tutorial
├── config      # Configuration files for different environments
│   ├── dev.toml
│   └── prod.toml.example
├── src         # Your application code lives here
│   ├── main.rs
│   └── migrations.rs
├── static      # CSS, JavaScript, Images, and other static files
│   └── css
│       └── main.css
├── templates   # HTML templates for your pages
│   └── index.html
├── .gitignore
├── bacon.toml  # Configuration for live-reloading during development
└── Cargo.toml
```

If you don't have [bacon] installed already, we strongly recommend you to do so. It will make your development process much more pleasant by providing you with the live-reloading functionality. You can install it by running:

```bash
cargo install --locked bacon
```

After you do that, you can run your Cot application by running:

```bash
bacon serve
```

Or, if you don't have [bacon] installed, you can run your application with the typical:

```bash
cargo run
```

Now, if you open your browser and navigate to [`localhost:8000`](http://localhost:8000), you should see a welcome page that Cot has generated for you. Congratulations, you've just created your first Cot application!

## Command Line Interface

Cot provides you with a CLI (Command Line Interface) for running your service. You can see all available commands by running:

```bash
cargo run -- --help
```

This will show you a list of available commands and options. This will be useful later, but for now you might want to know probably the most useful options `-c/--config`, which allows you to specify the configuration file to use. By default, Cot uses the `dev.toml` file from the `config` directory.

## Views and routing

At the heart of any web application is the ability to handle requests and return responses—this is exactly what views do in Cot. Let's look at the view that Cot generated for us and then create our own!

When you open the `src/main.rs` file, you'll see the following example view that has been generated for you:

```rust
async fn index() -> cot::Result<Html> {
    let index_template = IndexTemplate {};
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
}
```

Further in the file you can see that this view is registered in the `App` implementation:

```rust
struct CotTutorialApp;

impl App for CotTutorialApp {
    // ...

    fn router(&self) -> Router {
        Router::with_urls([Route::with_handler_and_name("/", index, "index")])
    }
}
```

This is how you specify the URL the view will be available at – in this case, the view is available at the root URL of your application. The `"index"` string is the name of the view, which you can use to reverse the URL in your templates – more on that in the next chapter.

You can add more views by adding more routes to the `Router` by simply defining more functions and registering them in the `router` method:

```rust
async fn hello() -> Html {
    Html::new("Hello World!")
}

// inside `impl App`:

fn router(&self) -> Router {
    Router::with_urls([
        Route::with_handler_and_name("/", index, "index"),
        Route::with_handler_and_name("/hello", hello, "hello"),
    ])
}
```

Now, when you visit [`localhost:8000/hello`](http://localhost:8000/hello) you should see `Hello World!` displayed on the page!

### Extractors and dynamic routes

You can also define dynamic routes by using the `Route::with_handler_and_name` method with a parameter enclosed in curly braces (e.g. `{param_name}`). How do we get the parameter value in the request handler's body, though?

At the core of Cot's request handling are _extractors_, which allow you to extract data from the request and pass it to the handler as arguments. One of such extractors is the `Path` extractor, which allows you to extract path parameters from the URL. In order to use it, you need to define a parameter in the handler function, passing the parameter type as the generic parameter, like so:

```rust
use cot::request::extractors::Path;

async fn hello_name(Path(name): Path<String>) -> cot::Result<Html> {
    Ok(Html::new(format!("Hello, {}!", name)))
}

// inside `impl App`:

fn router(&self) -> Router {
    Router::with_urls([
        Route::with_handler_and_name("/", index, "index"),
        Route::with_handler_and_name("/hello", hello, "hello"),
        Route::with_handler_and_name("/hello/{name}", hello_name, "hello_name"),
    ])
}
```

This works for multiple parameters, too—you just need to define a tuple of parameters in the handler function:

```rust
async fn hello_name(Path((first_name, last_name)): Path<(String, String)>) -> cot::Result<Html> {
    Ok(Html::new(format!("Hello, {first_name} {last_name}!")))
}

// inside `impl App`:

fn router(&self) -> Router {
    Router::with_urls([
        // ...
        Route::with_handler_and_name("/hello/{first_name}/{last_name}/", hello_name, "hello_name"),
    ])
}
```

Now, when you visit [`localhost:8000/hello/John/Smith/`](http://localhost:8000/hello/John), you should see `Hello, John Smith!` displayed on the page!

## Project structure

### App

An **app** is a self-contained collection of views, models, and static files that represent a specific functional unit of your service (e.g., an admin panel, a blog, or a user authentication system). Apps are designed to be modular and reusable.

When you define an app, you implement the `App` trait:

```rust
struct MyBlogApp;

#[async_trait::async_trait]
impl App for MyBlogApp {
    fn name(&self) -> &'static str {
        "blog"
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", index, "index"),
        ])
    }

    fn static_files(&self) -> Vec<StaticFile> {
        static_files!("css/blog.css")
    }
}
```

The `App` trait provides several hooks for initializing your application, defining routes, migrations, and administrative interfaces.

### Project

A **project** is the top-level container that ties multiple apps together and defines global configurations, middlewares, and the overall structure of your service. It's the entry point for your application.

When you define a project, you implement the `Project` trait:

```rust
struct MyProject;

impl Project for MyProject {
    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        // Registering an app with a URL prefix
        apps.register_with_views(MyBlogApp, "/blog");

        // Registering a built-in app
        apps.register(DatabaseUserApp::new());
    }

    fn middlewares(&self, handler: RootHandlerBuilder, context: &MiddlewareContext) -> RootHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(SessionMiddleware::from_context(context))
            .build()
    }
}
```

The `Project` trait is responsible for:
- **Registering apps**: Using `AppBuilder` to include apps and their routers in the project.
- **Configuring middlewares**: Defining the stack of middlewares that apply to all requests.
- **Providing CLI metadata**: Setting information for the generated CLI.
- **Handling global configuration**: Defining how the project is configured for different environments.

```rust
#[cot::main]
fn main() -> impl Project {
    CotTutorialProject
}
```

Finally, the `main` function just returns the Project implementation, which is the entry point for your application. Cot takes care of running it by providing a command line interface!

## Final words

In this chapter, you learned about:

* creating a new Cot project and how the Cot project structure looks like,
* running your first Cot project,
* create views, registering them in the router and passing parameters to them.

In the next chapter, we'll dive deeper into routing, which will allow us to define more complex URL structures for our application.

Remember to use `cargo doc --open` to browse the Cot documentation locally, or visit the [online documentation](https://docs.rs/cot) for more details about any of the components we've discussed.
