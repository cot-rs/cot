use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anstyle::{AnsiColor, Color, Effects, Style};
use anyhow::{bail, Context};
use cargo_toml::Manifest;

pub(crate) fn print_status_msg(status: StatusType, message: &str) {
    let style = status.style();
    let status_str = status.as_str();

    eprintln!("{style}{status_str:>12}{style:#} {message}");
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum StatusType {
    // In-Progress Ops
    Creating,
    Adding,
    Modifying,
    Removing,
    // Completed Ops
    Created,
    Added,
    Modified,
    Removed,

    // Status types
    #[allow(dead_code)]
    Error, // Should be used in Error handling inside remove operations
    #[allow(dead_code)]
    Warning, // Should be used as cautionary messages.
}

impl StatusType {
    fn style(self) -> Style {
        let base_style = Style::new() | Effects::BOLD;

        match self {
            // In-Progress => Brighter colors
            StatusType::Creating => base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightGreen))),
            StatusType::Adding => base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightCyan))),
            StatusType::Removing => {
                base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightMagenta)))
            }
            StatusType::Modifying => base_style.fg_color(Some(Color::Ansi(AnsiColor::BrightBlue))),
            // Completed => Dimmed colors
            StatusType::Created => base_style.fg_color(Some(Color::Ansi(AnsiColor::Green))),
            StatusType::Added => base_style.fg_color(Some(Color::Ansi(AnsiColor::Cyan))),
            StatusType::Removed => base_style.fg_color(Some(Color::Ansi(AnsiColor::Magenta))),
            StatusType::Modified => base_style.fg_color(Some(Color::Ansi(AnsiColor::Blue))),
            // Status types
            StatusType::Warning => base_style.fg_color(Some(Color::Ansi(AnsiColor::Yellow))),
            StatusType::Error => base_style.fg_color(Some(Color::Ansi(AnsiColor::Red))),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            StatusType::Creating => "Creating",
            StatusType::Adding => "Adding",
            StatusType::Modifying => "Modifying",
            StatusType::Removing => "Removing",
            StatusType::Created => "Created",
            StatusType::Added => "Added",
            StatusType::Modified => "Modified",
            StatusType::Removed => "Removed",
            StatusType::Warning => "Warning",
            StatusType::Error => "Error",
        }
    }
}

#[derive(Debug)]
pub(crate) enum CargoTomlManager {
    Workspace(WorkspaceManager),
    Package(PackageManager),
}

#[derive(Debug)]
pub(crate) struct WorkspaceManager {
    workspace_root: PathBuf,
    root_manifest: Manifest,
    package_manifests: HashMap<String, PackageManager>,
}

#[derive(Debug)]
pub(crate) struct PackageManager {
    package_root: PathBuf,
    manifest: Manifest,
}

impl CargoTomlManager {
    pub(crate) fn from_cargo_toml_path(cargo_toml_path: &Path) -> anyhow::Result<Self> {
        let cargo_toml_path = cargo_toml_path
            .canonicalize()
            .context("unable to canonicalize path")?;

        let manifest =
            Manifest::from_path(&cargo_toml_path).context("unable to read Cargo.toml")?;

        let manager = match (&manifest.workspace, &manifest.package) {
            (Some(_), _) => {
                let mut manager = Self::parse_workspace(&cargo_toml_path, manifest);

                if let Some(package) = &manager.root_manifest.package {
                    if manager.get_package_manager(package.name()).is_none() {
                        let workspace = manager
                            .root_manifest
                            .workspace
                            .as_mut()
                            .expect("workspace is known to be present");

                        if !workspace.members.contains(&package.name) {
                            let package_name = package.name().to_string();
                            workspace.members.push(package_name.clone());

                            let entry = PackageManager {
                                package_root: manager.workspace_root.clone(),
                                manifest: manager.root_manifest.clone(),
                            };

                            manager.package_manifests.insert(package_name, entry);
                        }
                    }
                }

                CargoTomlManager::Workspace(manager)
            }

            (None, Some(package)) => {
                let workspace_path = match package.workspace {
                    Some(ref workspace) => Some(PathBuf::from(workspace).join("Cargo.toml")),
                    None => cargo_toml_path
                        .parent() // dir containing Cargo.toml
                        .expect("Cargo.toml should always have a parent")
                        .parent() // dir containing the Cargo crate
                        .map(CargoTomlManager::find_cargo_toml)
                        .unwrap_or_default(), // dir containing the workspace Cargo.toml
                };

                if let Some(workspace_path) = workspace_path {
                    if let Ok(manifest) = Manifest::from_path(&workspace_path) {
                        let manager = Self::parse_workspace(&workspace_path, manifest);
                        return Ok(CargoTomlManager::Workspace(manager));
                    }
                }

                let manager = PackageManager {
                    package_root: cargo_toml_path
                        .parent()
                        .expect("Cargo.toml should always have a parent")
                        .canonicalize()
                        .context("unable to canonicalize path")?,
                    manifest,
                };
                CargoTomlManager::Package(manager)
            }

            (None, None) => {
                bail!("Cargo.toml is not a valid workspace or package manifest");
            }
        };

        Ok(manager)
    }

    fn parse_workspace(cargo_toml_path: &Path, manifest: Manifest) -> WorkspaceManager {
        assert!(manifest.workspace.is_some());
        let workspace = manifest
            .workspace
            .as_ref()
            .expect("workspace is known to be present");

        let workspace_root = cargo_toml_path
            .parent()
            .expect("Cargo.toml should always have a parent");
        let package_manifests = workspace
            .members
            .iter()
            .map(|member| {
                let member_path = workspace_root
                    .join(member)
                    .canonicalize()
                    .context("unable to canonicalize path")
                    .expect("Cargo.toml should exist");

                let member_manifest = Manifest::from_path(member_path.join("Cargo.toml"))
                    .expect("member manifest should be valid");

                let entry = PackageManager {
                    package_root: member_path,
                    manifest: member_manifest,
                };

                (entry.get_package_name().to_string(), entry)
            })
            .collect();

        WorkspaceManager {
            workspace_root: PathBuf::from(workspace_root),
            root_manifest: manifest,
            package_manifests,
        }
    }

    pub(crate) fn from_path(path: &Path) -> anyhow::Result<Option<Self>> {
        Self::find_cargo_toml(path)
            .map(|p| Self::from_cargo_toml_path(&p))
            .transpose()
    }

    pub(crate) fn find_cargo_toml(starting_dir: &Path) -> Option<PathBuf> {
        let mut current_dir = starting_dir;

        loop {
            let candidate = current_dir.join("Cargo.toml");
            if candidate.exists() {
                return Some(candidate);
            }

            match current_dir.parent() {
                Some(parent) => current_dir = parent,
                None => break,
            }
        }

        None
    }

    pub(crate) fn package_exists(&self, package_name: &str) -> bool {
        match self {
            CargoTomlManager::Workspace(manager) => {
                manager.get_package_manager(package_name).is_some()
            }
            CargoTomlManager::Package(manager) => manager.get_package_name() == package_name,
        }
    }

    pub(crate) fn get_package_manager(&self, package_name: &str) -> Option<&PackageManager> {
        match self {
            CargoTomlManager::Workspace(manager) => manager.get_package_manager(package_name),
            CargoTomlManager::Package(manager) => {
                if manager.get_package_name() == package_name {
                    Some(manager)
                } else {
                    None
                }
            }
        }
    }
}

impl WorkspaceManager {
    pub(crate) fn get_packages(&self) -> Vec<&PackageManager> {
        self.package_manifests.values().collect()
    }

    pub(crate) fn get_package_names(&self) -> Vec<&str> {
        self.package_manifests.keys().map(String::as_str).collect()
    }

    pub(crate) fn get_package_paths(&self) -> Vec<&Path> {
        self.package_manifests
            .values()
            .map(|v| v.package_root.as_path())
            .collect()
    }

    pub(crate) fn get_root_manifest(&self) -> &Manifest {
        &self.root_manifest
    }

    pub(crate) fn get_package_manager(&self, package_name: &str) -> Option<&PackageManager> {
        self.package_manifests.get(package_name)
    }

    pub(crate) fn get_package_manager_by_path(
        &self,
        package_path: &Path,
    ) -> Option<&PackageManager> {
        let mut package_path = package_path
            .canonicalize()
            .context("unable to canonicalize path")
            .ok()?;

        if package_path.is_file() {
            package_path = package_path
                .parent()
                .expect("file path should always have a parent")
                .into();
        }

        self.package_manifests
            .values()
            .find(|m| m.package_root == package_path)
    }

    pub(crate) fn get_workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

impl PackageManager {
    pub(crate) fn get_package_name(&self) -> &str {
        self.manifest
            .package
            .as_ref()
            .expect("package is known to be present")
            .name()
    }

    pub(crate) fn get_package_path(&self) -> &Path {
        self.package_root.as_path()
    }

    pub(crate) fn get_manifest_path(&self) -> PathBuf {
        let path = &self.get_package_path().join("Cargo.toml");
        path.to_owned()
    }

    pub(crate) fn get_manifest(&self) -> &Manifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use cot_cli::test_utils;

    use super::*;

    fn get_package() -> (tempfile::TempDir, PackageManager) {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        test_utils::make_package(temp_dir.path()).unwrap();

        let CargoTomlManager::Package(manager) = CargoTomlManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap()
        else {
            unreachable!()
        };

        (temp_dir, manager)
    }

    fn get_workspace(packages: u8) -> (tempfile::TempDir, WorkspaceManager) {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        test_utils::make_workspace_package(temp_dir.path(), packages).unwrap();

        let CargoTomlManager::Workspace(manager) = CargoTomlManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap()
        else {
            unreachable!()
        };

        (temp_dir, manager)
    }

    mod cargo_toml_manager {
        use super::*;

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn find_cargo_toml() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_package(temp_dir.path()).unwrap();
            let cargo_toml_path = temp_dir.path().join("Cargo.toml");

            let found_path = CargoTomlManager::find_cargo_toml(temp_dir.path()).unwrap();
            assert_eq!(found_path, cargo_toml_path);
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn find_cargo_toml_recursive() {
            let temp_dir = tempfile::tempdir().unwrap();
            let nested_dir = temp_dir.path().join("nested");
            test_utils::make_package(&nested_dir).unwrap();

            let found_path = CargoTomlManager::find_cargo_toml(&nested_dir).unwrap();
            assert_eq!(found_path, nested_dir.join("Cargo.toml"));
        }

        #[test]
        fn find_cargo_toml_not_found() {
            let temp_dir = tempfile::tempdir().unwrap();
            let found_path = CargoTomlManager::find_cargo_toml(temp_dir.path());
            assert!(found_path.is_none());
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn load_valid_virtual_workspace_manifest() {
            let cot_cli_root = env!("CARGO_MANIFEST_DIR");
            let cot_root = Path::new(cot_cli_root).parent().unwrap();

            let manager = CargoTomlManager::from_path(cot_root).unwrap().unwrap();
            match manager {
                CargoTomlManager::Workspace(manager) => {
                    assert!(manager.get_root_manifest().workspace.is_some());
                    assert!(!manager.package_manifests.is_empty());
                }
                _ => panic!("Expected workspace manifest"),
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn load_valid_workspace_from_package_manifest() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_package(temp_dir.path()).unwrap();
            let cargo_toml_path = temp_dir.path().join("Cargo.toml");
            let mut handle = std::fs::OpenOptions::new()
                .append(true)
                .open(&cargo_toml_path)
                .unwrap();
            writeln!(handle, "{}", test_utils::WORKSPACE_STUB).unwrap();

            let manager = CargoTomlManager::from_path(temp_dir.path())
                .unwrap()
                .unwrap();
            match manager {
                CargoTomlManager::Workspace(manager) => {
                    assert!(manager.get_root_manifest().workspace.is_some());
                    assert_eq!(manager.get_packages().len(), 1);
                    assert_eq!(
                        manager.get_packages()[0].get_manifest_path(),
                        cargo_toml_path
                    );
                }
                _ => panic!("Expected workspace manifest"),
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn load_valid_package_manifest() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
            test_utils::make_package(temp_dir.path()).unwrap();

            let manager = CargoTomlManager::from_path(temp_dir.path())
                .unwrap()
                .unwrap();

            match manager {
                CargoTomlManager::Package(manager) => {
                    assert_eq!(manager.get_package_name(), package_name);
                    assert_eq!(manager.get_package_path(), temp_dir.path());
                }
                _ => panic!("Expected package manifest"),
            }
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn package_exists() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_workspace_package(temp_dir.path(), 1).unwrap();
            let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();

            let manager = CargoTomlManager::from_path(temp_dir.path())
                .unwrap()
                .unwrap();

            assert!(manager.package_exists(&test_utils::get_nth_crate_name(1)));
            assert!(!manager.package_exists("non-existent"));
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_package_manager() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_workspace_package(temp_dir.path(), 1).unwrap();

            let manager = CargoTomlManager::from_path(temp_dir.path())
                .unwrap()
                .unwrap();

            let first_package = test_utils::get_nth_crate_name(1);
            let package = manager.get_package_manager(first_package.as_str());
            assert!(package.is_some());
            assert_eq!(
                package
                    .unwrap()
                    .get_manifest()
                    .package
                    .as_ref()
                    .unwrap()
                    .name,
                first_package.as_str()
            );

            let package = manager.get_package_manager("non-existent");
            assert!(package.is_none());
        }
    }

    mod workspace_manager {
        use super::*;

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_packages() {
            let (_, manager) = get_workspace(2);

            assert_eq!(manager.get_packages().len(), 2);
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_package_names() {
            let (_, manager) = get_workspace(2);

            let mut packages = manager.get_package_names();
            packages.sort();

            assert_eq!(packages.len(), 2);
            assert_eq!(packages[0], test_utils::get_nth_crate_name(1));
            assert_eq!(packages[1], test_utils::get_nth_crate_name(2));
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn test_get_package_manager_by_path() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_workspace_package(temp_dir.path(), 1).unwrap();
            #[test]
            #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                      // `linux`
            fn get_package_paths() {
                let (temp_dir, manager) = get_workspace(2);

                let mut packages = manager.get_package_paths();
                packages.sort();

                assert_eq!(packages.len(), 2);
                assert_eq!(
                    packages[0],
                    temp_dir.path().join(test_utils::get_nth_crate_name(1))
                );
                assert_eq!(
                    packages[1],
                    temp_dir.path().join(test_utils::get_nth_crate_name(2))
                );
            }

            #[test]
            #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                      // `linux`
            fn get_root_manifest() {
                let (temp_dir, manager) = get_workspace(1);
                let manifest_path = temp_dir.path().join("Cargo.toml");
                let orig_manifest = Manifest::from_path(&manifest_path).unwrap();

                let manifest = manager.get_root_manifest();

                assert_eq!(*manifest, orig_manifest);
            }

            #[test]
            #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                      // `linux`
            fn get_package_manager() {
                let (_, manager) = get_workspace(2);
                let package_name = test_utils::get_nth_crate_name(1);

                let package = manager.get_package_manager(package_name.as_str());

                assert!(package.is_some());
                assert_eq!(
                    package
                        .unwrap()
                        .get_manifest()
                        .package
                        .as_ref()
                        .unwrap()
                        .name,
                    package_name
                );

                let package = manager.get_package_manager("non-existent");
                assert!(package.is_none());
            }

            #[test]
            #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                      // `linux`
            fn get_package_manager_by_path() {
                let (temp_dir, manager) = get_workspace(1);
                let package_name = test_utils::get_nth_crate_name(1);
                let package_path = temp_dir.path().join(&package_name);

                let package = manager.get_package_manager_by_path(&package_path);
                assert!(package.is_some());
                assert_eq!(
                    package
                        .unwrap()
                        .get_manifest()
                        .package
                        .as_ref()
                        .unwrap()
                        .name,
                    package_name
                );

                let package_path = package_path.join("Cargo.toml");
                let package = manager.get_package_manager_by_path(&package_path);
                assert!(package.is_some());
                assert_eq!(
                    package
                        .unwrap()
                        .get_manifest()
                        .package
                        .as_ref()
                        .unwrap()
                        .name,
                    package_name
                );

                let non_existent = temp_dir.path().join("non-existent/Cargo.toml");
                let package = manager.get_package_manager_by_path(&non_existent);
                assert!(package.is_none());
            }

            #[test]
            #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                      // `linux`
            fn get_workspace_root() {
                let (temp_dir, manager) = get_workspace(1);

                assert_eq!(manager.get_workspace_root(), temp_dir.path());
            }
        }
    }
    mod package_manager {
        use super::*;

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_package_name() {
            let (temp_dir, manager) = get_package();
            let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();

            assert_eq!(manager.get_package_name(), package_name);
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_package_path() {
            let (temp_dir, manager) = get_package();

            assert_eq!(manager.get_package_path(), temp_dir.path());
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_manifest_path() {
            let (temp_dir, manager) = get_package();

            assert_eq!(
                manager.get_manifest_path(),
                temp_dir.path().join("Cargo.toml")
            );
        }

        #[test]
        #[cfg_attr(miri, ignore)] // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS
                                  // `linux`
        fn get_manifest() {
            let (temp_dir, manager) = get_package();
            let manifest_path = temp_dir.path().join("Cargo.toml");
            let orig_manifest = Manifest::from_path(&manifest_path).unwrap();

            assert_eq!(*manager.get_manifest(), orig_manifest);
        }
    }
}
