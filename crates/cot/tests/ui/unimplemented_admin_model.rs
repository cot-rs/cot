use cot::admin::{AdminModelManager, DefaultAdminModelManager};

struct Model {}

fn main() {
    let _: Box<dyn AdminModelManager> = Box::new(DefaultAdminModelManager::<Model>::new());
}
