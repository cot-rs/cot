use cot::db::Model;

struct TodoItem {}

fn main() {
    let _ = <TodoItem as Model>::objects();
}
