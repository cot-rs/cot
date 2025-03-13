---
title: Introduction
---

[bacon]: https://dystroy.org/bacon/

<div class="alert alert-warning" role="alert"><strong>Disclaimer</strong>: Cot is currently missing a lot of features and is <strong>not ready</strong> for production use. This guide is a work in progress and will be updated as Cot matures. That said, you are more than welcome to try it out and provide feedback!</div>

Cot is a free and open-source web framework for Rust that makes building web applications both fun and reliable. Taking inspiration from [Django](https://www.djangoproject.com/)'s developer-friendly approach, Cot combines Rust's safety guarantees with rapid development features that help you build secure web applications quickly. Whether you're coming from Django or are new to web development entirely, you'll find Cot's intuitive design helps you be productive from day one.

## Who is this guide for?

This guide doesn't assume any advanced knowledge in Rust or web development in general (although this will help, too!). It's aimed at beginners who are looking to get started with Cot, and will guide you through the process of setting up a new project, creating your first views, using the Cot ORM and running your application.

If you are not familiar with Rust, you might want to start by reading the [Rust Book](https://doc.rust-lang.org/book/), which is an excellent resource for learning Rust.

## Installing and running Cot CLI

Let's get your first Cot project up and running! First, you'll need Cargo, Rust's package manager. If you don't have it installed, you can get it through [rustup](https://rustup.rs/). Cot requires Rust version 1.84 or later.

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
async fn index(request: Request) -> cot::Result<Response> {
    let index_template = IndexTemplate {};
    let rendered = index_template.render()?;

    Ok(Response::new_html(StatusCode::OK, Body::fixed(rendered)))
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
async fn hello(request: Request) -> cot::Result<Response> {
    Ok(Response::new_html(StatusCode::OK, Body::fixed("Hello World!")))
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

### Dynamic routes

You can also define dynamic routes by using the `Route::with_handler_and_name` method with a parameter enclosed in curly braces (e.g. `{param_name}`). This parameter will be available in the `Request` object, and you can extract it using the `path_params().parse()` method. It will automatically infer the type of the parameter(s) based on the type of the variable you assign it to. Here's an example:

```rust
async fn hello_name(request: Request) -> cot::Result<Response> {
    let name: String = request.path_params().parse()?;

    Ok(Response::new_html(StatusCode::OK, Body::fixed(format!("Hello, {}!", name))))
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

Now, when you visit [`localhost:8000/hello/John`](http://localhost:8000/hello/John), you should see `Hello, John!` displayed on the page!

## Project structure

### App

Along with an example view, the entire Cot project structure has been created for you. Let's take a look one by one at what you can find in `main.rs`:

```rust
struct CotTutorialApp;

impl App for CotTutorialApp {
    fn name(&self) -> &'static str {
        env!("CARGO_CRATE_NAME")
    }
```

An app is a collection of views and other components that make up a part of your service. Typically, they represent a part of your service, like the main website, an admin panel, or an API. An app usually corresponds to a single Rust crate, hence we're just using the name of the crate as the app name. The app name is used in many places, such as in the database table names, in the admin panel, or when reversing the URLs, so it needs to be unique in your project.

```rust
    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }
```

This defines the database migration list that will be applied when your server starts. You shouldn't normally need to modify this, and the migrations can be generated automatically using the Cot CLI – more on this in the chapter about database models.

```rust
    fn static_files(&self) -> Vec<(String, Bytes)> {
        static_files!("css/main.css")
    }
}
```

This defines a list of static files that will be served by the server. More on that will be covered in the chapter about static files.

### Project

A project is a collection of apps, middlewares, and other components that make up your service. It ties everything together and is the entry point for your application. Here's the default project implementation's structure analyzed step by step:

```rust
struct CotTutorialProject;

impl Project for CotTutorialProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }
```

This defines the project and sets the CLI metadata (like the name, version, and description) that will be displayed when you run `cargo run -- --help` by using the metadata from your Cargo crate.

```rust
    fn register_apps(&self, apps: &mut AppBuilder, _context: &ProjectContext<WithConfig>) {
        apps.register_with_views(CotTutorialApp, "");
    }
```

This registers all the apps that your project is using.

```rust
    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &ProjectContext<WithApps>,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_app_context(context))
            .middleware(LiveReloadMiddleware::from_app_context(context))
            .build()
    }
```

This registers the middlewares that will be applied to all routes in the project. Note that the `LiveReloadMiddleware` may be dynamically disabled in runtime using config!

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

In the next chapter, we'll dive deeper into templates, which will allow us to create more sophisticated HTML pages.

Remember to use `cargo doc --open` to browse the Cot documentation locally, or visit the [online documentation](https://docs.rs/cot) for more details about any of the components we've discussed.
