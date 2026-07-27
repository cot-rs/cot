use async_trait::async_trait;
use clap::{ArgMatches, Command};
use cot::cli::{CliTask, CliTaskGroup};
use cot::config::ProjectConfig;
use cot::project::WithConfig;
use cot::{Bootstrapper, Project};

struct TestProject;
impl Project for TestProject {}

#[cot::test]
async fn cli_task_group_dispatches_nested_task() {
    use std::sync::atomic::{AtomicBool, Ordering};

    struct NestedTask;
    #[async_trait(?Send)]
    impl CliTask for NestedTask {
        fn subcommand(&self) -> Command {
            Command::new("nested")
        }

        async fn execute(
            &mut self,
            _matches: &ArgMatches,
            _bootstrapper: Bootstrapper<WithConfig>,
        ) -> cot_core::Result<()> {
            TASK_CALLED.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    static TASK_CALLED: AtomicBool = AtomicBool::new(false);
    TASK_CALLED.store(false, Ordering::SeqCst);

    let mut group = CliTaskGroup::new("group");
    group.add_task(NestedTask);
    let matches = group.subcommand().get_matches_from(["group", "nested"]);
    let bootstrapper = Bootstrapper::new(TestProject).with_config(ProjectConfig::default());

    group.execute(&matches, bootstrapper).await.unwrap();

    assert!(TASK_CALLED.load(Ordering::SeqCst));
}
