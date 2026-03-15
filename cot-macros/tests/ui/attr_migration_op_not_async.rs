#[allow(unused)]
use cot::db::Result;
#[allow(unused)]
use cot::db::migrations::{MigrationContext, migration_op};

#[migration_op]
fn my_migration(_ctx: MigrationContext<'_>) -> Result<()> {
    Ok(())
}

fn main() {}
