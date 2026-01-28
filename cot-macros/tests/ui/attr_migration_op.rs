use cot::db::Result;
use cot::db::migrations::{MigrationContext, migration_op};

#[migration_op]
async fn my_migration(_ctx: MigrationContext<'_>) -> Result<()> {
    Ok(())
}

fn main() {}
