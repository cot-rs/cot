use cot::db::{model, Auto, ForeignKey};

pub mod models {
    use super::*;

    #[model]
    pub struct Parent {
        #[model(primary_key)]
        pub id: Auto<i32>,
    }

    #[model]
    pub struct Child {
        #[model(primary_key)]
        pub id: Auto<i32>,
        pub parent: ForeignKey<crate::Parent>,
    }
}

pub use models::*;

fn main() {}
