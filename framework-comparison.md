---
title: Framework comparison
---

Cot is an opinionated, batteries-included web framework for Rust, designed to feel familiar to developers coming from Django (Python) or similar high-level frameworks. While the Rust ecosystem is rich with excellent web frameworks, most of them follow a "modular" philosophy—giving you the building blocks but requiring you to assemble the rest (database, auth, admin interfaces) yourself.

Cot takes a different approach by providing a cohesive, integrated experience out of the box.

## Comparison Table

|                       | Cot                                      | Axum                      | Actix-web                  | Rocket                                   | Loco                            |
|:----------------------|:-----------------------------------------|:--------------------------|:---------------------------|:-----------------------------------------|:--------------------------------|
| **Philosophy**        | Batteries-included (Django-like)         | Modular / Micro-framework | Modular / High-performance | Batteries-included                       | Batteries-included (Rails-like) |
| **Underlying Engine** | [Axum](https://github.com/tokio-rs/axum) | Hyper / Tokio             | Actix                      | Hyper / Tokio                            | Axum                            |
| **ORM**               | Integrated (Cot ORM, based on SeaORM)    | Agnostic                  | Agnostic                   | Agnostic                                 | SeaORM                          |
| **Admin Panel**       | **Built-in**                             | ❌                         | ❌                          | ❌                                        | ❌                               |
| **Migrations**        | Auto-generated from structs              | External (e.g., sqlx-cli) | External                   | External                                 | External / CLI                  |
| **Templating**        | Agnostic, but [Askama] built-in          | Agnostic                  | Agnostic                   | Agnostic (integrates w/ Tera/Handlebars) | Agnostic (integrates w/ Tera)   |

[Askama]: https://askama.readthedocs.io/en/stable/

## Cot vs. Axum

[Axum](https://github.com/tokio-rs/axum) is currently one of the most popular web frameworks in the Rust ecosystem. In fact, **Cot is built on top of Axum**.

* **Choose Axum if:** You want full control over every component of your stack, prefer a minimalist approach, or are building a microservice that doesn't need a UI or database management.
* **Choose Cot if:** You want a full-stack experience with standard conventions. Cot handles the "glue code" for databases, authentication, and sessions so you can focus on business logic. Since Cot uses Axum internally, you often benefit from Axum's performance and compatibility with the Tower ecosystem.

## Cot vs. Actix-web

[Actix-web](https://actix.rs/) is known for its extreme performance and maturity.

* **Choose Actix-web if:** Raw request-per-second performance is your primary metric, or you are deeply invested in the Actor model.
* **Choose Cot if:** You prioritize developer velocity and ease of use over squeezing the last microsecond of performance. Cot is still very fast (thanks to Rust and Axum), but it prioritizes "getting things done" with tools like the Admin panel and auto-migrations.

## Cot vs. Rocket

[Rocket](https://rocket.rs/) is famous for its focus on developer ergonomics and macro-based routing.

* **Choose Rocket if:** You love its specific API style and macro magic, and don't mind picking your own database layer.
* **Choose Cot if:** You want the "Django" experience. While Rocket makes routing easy, it doesn't prescribe how to handle users, permissions, or admin interfaces. Cot provides these standard web application features out of the box.

## Cot vs. Loco

[Loco](https://loco.rs/) is another batteries-included framework, often described as "Rails for Rust".

* **Choose Loco if:** You prefer the Ruby on Rails philosophy, use SeaORM, and like its specific project structure.
* **Choose Cot if:** You prefer the Django philosophy. Cot's defining features—like the auto-generated Admin panel and the specific way it maps Rust structs to database tables—are directly inspired by Django, often making it easier to quickly start a web application with standard features.
