#[allow(unused)]
use cot::db::migrations::{MigrationContext, migration_op};

#[migration_op]
async fn my_migration(_ctx: MigrationContext<'_>) {}

fn main() {}
