use std::collections::HashMap;

use flareon::db::migrations::MigrationDependency;
use thiserror::Error;

use crate::db::migrations::{DynMigration, MigrationDependencyInner, OperationInner};

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MigrationSorterError {
    #[error("Cycle detected in migrations")]
    CycleDetected,
    #[error("Dependency not found: {}", format_migration_dependency(.0))]
    InvalidDependency(MigrationDependency),
    #[error("Migration defined twice: {app_name}::{migration_name}")]
    DuplicateMigration {
        app_name: String,
        migration_name: String,
    },
    #[error("Migration creating model defined twice: {app_name}::{model_name}")]
    DuplicateModel {
        app_name: String,
        model_name: String,
    },
}

type Result<T> = core::result::Result<T, MigrationSorterError>;

fn format_migration_dependency(dependency: &MigrationDependency) -> String {
    match dependency.inner {
        MigrationDependencyInner::Migration { app, migration } => {
            format!("migration {app}::{migration}")
        }
        MigrationDependencyInner::Model { app, model_name } => {
            format!("model {app}::{model_name}")
        }
    }
}

/// Sorts migrations topologically based on their dependencies.
#[derive(Debug)]
pub(super) struct MigrationSorter<'a, T> {
    migrations: &'a mut [T],
}

impl<'a, T: DynMigration> MigrationSorter<'a, T> {
    #[must_use]
    pub(super) fn new(migrations: &'a mut [T]) -> Self {
        Self { migrations }
    }

    pub(super) fn sort(&mut self) -> Result<()> {
        // Sort by names to ensure that the order is deterministic
        self.migrations
            .sort_by(|a, b| (b.app_name(), b.name()).cmp(&(a.app_name(), a.name())));

        self.toposort()?;
        Ok(())
    }

    fn toposort(&mut self) -> Result<()> {
        let lookup = Self::create_lookup_table(self.migrations)?;
        let mut graph = Graph::new(self.migrations.len());

        for (index, migration) in self.migrations.iter().enumerate() {
            for dependency in migration.dependencies() {
                let dependency_index = lookup
                    .get(&MigrationLookup::from(dependency))
                    .ok_or(MigrationSorterError::InvalidDependency(*dependency))?;
                graph.add_edge(*dependency_index, index);
            }
        }

        let mut sorted_indices = graph.toposort()?;
        apply_permutation(self.migrations, &mut sorted_indices);

        Ok(())
    }

    fn create_lookup_table(migrations: &[T]) -> Result<HashMap<MigrationLookup, usize>> {
        let mut map = HashMap::with_capacity(migrations.len());

        for (index, migration) in migrations.iter().enumerate() {
            let app_and_name = MigrationLookup::ByAppAndName {
                app: migration.app_name(),
                name: migration.name(),
            };
            if map.insert(app_and_name, index).is_some() {
                return Err(MigrationSorterError::DuplicateMigration {
                    app_name: migration.app_name().to_owned(),
                    migration_name: migration.name().to_owned(),
                });
            };

            for operation in migration.operations() {
                if let OperationInner::CreateModel { table_name, .. } = operation.inner {
                    let app_and_model = MigrationLookup::ByAppAndModel {
                        app: migration.app_name(),
                        model: table_name.0,
                    };
                    if map.insert(app_and_model, index).is_some() {
                        return Err(MigrationSorterError::DuplicateModel {
                            app_name: migration.app_name().to_owned(),
                            model_name: table_name.0.to_owned(),
                        });
                    }
                }
            }
        }

        Ok(map)
    }
}

fn apply_permutation<T>(migrations: &mut [T], order: &mut [usize]) {
    for i in 0..order.len() {
        let mut current = i;
        let mut next = order[current];

        while next != i {
            // process the cycle
            migrations.swap(current, next);
            order[current] = current;

            current = next;
            next = order[current];
        }

        order[current] = current;
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
enum MigrationLookup<'a> {
    ByAppAndName { app: &'a str, name: &'a str },
    ByAppAndModel { app: &'a str, model: &'a str },
}

impl From<&MigrationDependency> for MigrationLookup<'_> {
    fn from(dependency: &MigrationDependency) -> Self {
        match dependency.inner {
            MigrationDependencyInner::Migration { app, migration } => {
                MigrationLookup::ByAppAndName {
                    app,
                    name: migration,
                }
            }
            MigrationDependencyInner::Model { app, model_name } => MigrationLookup::ByAppAndModel {
                app,
                model: model_name,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Graph {
    vertex_edges: Vec<Vec<usize>>,
}

impl Graph {
    #[must_use]
    fn new(vertex_num: usize) -> Self {
        Self {
            vertex_edges: vec![Vec::new(); vertex_num],
        }
    }

    fn add_edge(&mut self, from: usize, to: usize) {
        self.vertex_edges[from].push(to);
    }

    #[must_use]
    fn vertex_num(&self) -> usize {
        self.vertex_edges.len()
    }

    fn toposort(&mut self) -> Result<Vec<usize>> {
        let mut visited = vec![VisitedStatus::NotVisited; self.vertex_num()];
        let mut sorted_indices_stack = Vec::with_capacity(self.vertex_num());

        for index in 0..self.vertex_num() {
            self.visit(index, &mut visited, &mut sorted_indices_stack)?;
        }

        assert_eq!(sorted_indices_stack.len(), self.vertex_num());

        sorted_indices_stack.reverse();
        Ok(sorted_indices_stack)
    }

    fn visit(
        &self,
        index: usize,
        visited: &mut Vec<VisitedStatus>,
        sorted_indices_stack: &mut Vec<usize>,
    ) -> Result<()> {
        match visited[index] {
            VisitedStatus::Visited => return Ok(()),
            VisitedStatus::Visiting => {
                return Err(MigrationSorterError::CycleDetected);
            }
            VisitedStatus::NotVisited => {}
        }

        visited[index] = VisitedStatus::Visiting;

        for &neighbor in &self.vertex_edges[index] {
            self.visit(neighbor, visited, sorted_indices_stack)?;
        }

        visited[index] = VisitedStatus::Visited;
        sorted_indices_stack.push(index);

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum VisitedStatus {
    NotVisited,
    Visiting,
    Visited,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::migrations::{MigrationDependency, Operation};
    use crate::db::Identifier;
    use crate::test::TestMigration;

    #[test]
    fn graph_toposort() {
        let mut graph = Graph::new(8);
        graph.add_edge(0, 3);
        graph.add_edge(1, 3);
        graph.add_edge(1, 4);
        graph.add_edge(2, 4);
        graph.add_edge(2, 7);
        graph.add_edge(3, 5);
        graph.add_edge(3, 6);
        graph.add_edge(3, 7);
        graph.add_edge(4, 6);

        let sorted_indices = graph.toposort().unwrap();

        assert_eq!(sorted_indices, vec![2, 1, 4, 0, 3, 7, 6, 5]);
    }

    #[test]
    fn create_lookup_table() {
        let migrations = vec![
            TestMigration::new(
                "app1",
                "migration1",
                [],
                [Operation::create_model()
                    .table_name(Identifier::new("model1"))
                    .fields(&[])
                    .build()],
            ),
            TestMigration::new(
                "app1",
                "migration2",
                [],
                [Operation::create_model()
                    .table_name(Identifier::new("model2"))
                    .fields(&[])
                    .build()],
            ),
        ];

        let lookup = MigrationSorter::create_lookup_table(&migrations).unwrap();

        assert_eq!(lookup.len(), 4);
        assert!(lookup.contains_key(&MigrationLookup::ByAppAndName {
            app: "app1",
            name: "migration1"
        }));
        assert!(lookup.contains_key(&MigrationLookup::ByAppAndName {
            app: "app1",
            name: "migration2"
        }));
        assert!(lookup.contains_key(&MigrationLookup::ByAppAndModel {
            app: "app1",
            model: "model1"
        }));
        assert!(lookup.contains_key(&MigrationLookup::ByAppAndModel {
            app: "app1",
            model: "model2"
        }));
    }

    #[test]
    fn sort() {
        let mut migrations = vec![
            TestMigration::new("app1", "migration2", [], []),
            TestMigration::new("app1", "migration1", [], []),
        ];

        let mut sorter = MigrationSorter::new(&mut migrations);
        sorter.sort().unwrap();

        assert_eq!(sorter.migrations[0].name(), "migration1");
        assert_eq!(sorter.migrations[1].name(), "migration2");
    }

    #[test]
    fn toposort() {
        let mut migrations = vec![
            TestMigration::new("app2", "migration_before", [], []),
            TestMigration::new(
                "app2",
                "migration_after",
                [MigrationDependency::migration("app2", "migration_before")],
                [],
            ),
            TestMigration::new(
                "app1",
                "migration_before",
                [MigrationDependency::migration("app2", "migration_before")],
                [],
            ),
            TestMigration::new(
                "app1",
                "migration_after",
                [
                    MigrationDependency::migration("app1", "migration_before"),
                    MigrationDependency::migration("app2", "migration_after"),
                ],
                [],
            ),
        ];

        let mut sorter = MigrationSorter::new(&mut migrations);
        sorter.sort().unwrap();

        assert_eq!(
            (migrations[0].app_name(), migrations[0].name()),
            ("app2", "migration_before")
        );
        assert_eq!(
            (migrations[1].app_name(), migrations[1].name()),
            ("app1", "migration_before")
        );
        assert_eq!(
            (migrations[2].app_name(), migrations[2].name()),
            ("app2", "migration_after")
        );
        assert_eq!(
            (migrations[3].app_name(), migrations[3].name()),
            ("app1", "migration_after")
        );
    }

    // migration names must be &'static str
    const MIGRATION_NAMES: [&str; 100] = [
        "m0", "m1", "m2", "m3", "m4", "m5", "m6", "m7", "m8", "m9", "m10", "m11", "m12", "m13",
        "m14", "m15", "m16", "m17", "m18", "m19", "m20", "m21", "m22", "m23", "m24", "m25", "m26",
        "m27", "m28", "m29", "m30", "m31", "m32", "m33", "m34", "m35", "m36", "m37", "m38", "m39",
        "m40", "m41", "m42", "m43", "m44", "m45", "m46", "m47", "m48", "m49", "m50", "m51", "m52",
        "m53", "m54", "m55", "m56", "m57", "m58", "m59", "m60", "m61", "m62", "m63", "m64", "m65",
        "m66", "m67", "m68", "m69", "m70", "m71", "m72", "m73", "m74", "m75", "m76", "m77", "m78",
        "m79", "m80", "m81", "m82", "m83", "m84", "m85", "m86", "m87", "m88", "m89", "m90", "m91",
        "m92", "m93", "m94", "m95", "m96", "m97", "m98", "m99",
    ];

    #[test]
    fn toposort_big() {
        const MIGRATION_NUM: usize = 100;

        let mut migrations = Vec::new();
        for i in 0..MIGRATION_NUM {
            let deps = (0..i)
                .map(|i| MigrationDependency::migration("app1", MIGRATION_NAMES[i]))
                .collect::<Vec<_>>();

            migrations.push(TestMigration::new("app1", MIGRATION_NAMES[i], deps, []));
        }

        let mut sorter = MigrationSorter::new(&mut migrations);
        sorter.sort().unwrap();

        for (i, migration) in migrations.iter().enumerate() {
            assert_eq!(migration.name(), MIGRATION_NAMES[i]);
        }
    }

    #[test]
    fn cycle_detection() {
        let mut migrations = vec![
            TestMigration::new(
                "app1",
                "migration1",
                [MigrationDependency::migration("app1", "migration2")],
                [Operation::create_model()
                    .table_name(Identifier::new("model1"))
                    .fields(&[])
                    .build()],
            ),
            TestMigration::new(
                "app1",
                "migration2",
                [MigrationDependency::migration("app1", "migration1")],
                [Operation::create_model()
                    .table_name(Identifier::new("model2"))
                    .fields(&[])
                    .build()],
            ),
        ];

        let mut sorter = MigrationSorter::new(&mut migrations);
        assert_eq!(
            sorter.toposort().unwrap_err(),
            MigrationSorterError::CycleDetected
        );
    }

    #[test]
    fn duplicate_migration() {
        let mut migrations = vec![
            TestMigration::new("app1", "migration1", [], []),
            TestMigration::new("app1", "migration1", [], []),
        ];

        let mut sorter = MigrationSorter::new(&mut migrations);
        assert_eq!(
            sorter.toposort().unwrap_err(),
            MigrationSorterError::DuplicateMigration {
                app_name: "app1".to_owned(),
                migration_name: "migration1".to_owned()
            }
        );
    }

    #[test]
    fn duplicate_model() {
        let mut migrations = vec![
            TestMigration::new(
                "app1",
                "migration1",
                [],
                [Operation::create_model()
                    .table_name(Identifier::new("model1"))
                    .fields(&[])
                    .build()],
            ),
            TestMigration::new(
                "app1",
                "migration2",
                [],
                [Operation::create_model()
                    .table_name(Identifier::new("model1"))
                    .fields(&[])
                    .build()],
            ),
        ];

        let mut sorter = MigrationSorter::new(&mut migrations);
        assert_eq!(
            sorter.toposort().unwrap_err(),
            MigrationSorterError::DuplicateModel {
                app_name: "app1".to_owned(),
                model_name: "model1".to_owned()
            }
        );
    }
}