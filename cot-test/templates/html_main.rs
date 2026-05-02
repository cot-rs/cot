#![allow(unused_imports, dead_code)]
use std::collections::HashMap;
use std::fmt::Display;

use cot::Template;
use cot::form::{Form, FormContext, FormErrorTarget};
use cot::html::Html;
use cot::request::Request;
use cot::request::extractors::StaticFiles;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ::schemars::JsonSchema)]
struct Item {
    title: String,
}
impl Display for Item {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}
impl askama::filters::HtmlSafe for Item {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, ::schemars::JsonSchema)]
struct User {
    is_admin: bool,
    is_logged_in: bool,
    role: Option<String>,
}

#[derive(cot::form::Form)]
struct DummyForm {
    name: String,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    static_files: &'a StaticFiles,
    request: &'a Request,
    form: &'a <DummyForm as Form>::Context,
    form_context: &'a <DummyForm as Form>::Context,
    name: String,
    items: Vec<Item>,
    user: User,
    item: Item,
    urls: &'a cot::router::Urls,
    error: cot::Error,
}

fn main() {}
