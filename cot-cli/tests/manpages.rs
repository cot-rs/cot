#[test]
#[cfg_attr(miri, ignore)] // unsupported operation: extern static `pidfd_spawnp` is not supported by Miri
fn generate_manpages() {
    let tempdir = tempfile::TempDir::new().unwrap();
    let args = cot_cli::args::ManpagesArgs {
        output_dir: Some(tempdir.path().to_path_buf()),
        create: false,
    };

    cot_cli::handlers::handle_cli_manpages(args).unwrap();

    let expected_file_names = vec![
        "cot.1",
        "cot-cli.1",
        "cot-cli-completions.1",
        "cot-cli-manpages.1",
        "cot-migration.1",
        "cot-migration-list.1",
        "cot-migration-make.1",
        "cot-new.1",
    ];
    for path in expected_file_names {
        assert!(
            tempdir.path().join(path).exists(),
            "{path} manpage does not exist",
        );
    }
}
