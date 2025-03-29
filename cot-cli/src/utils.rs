use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anstyle::{AnsiColor, Color, Effects, Style};
use anyhow::{Context, bail};
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
    #[expect(dead_code)]
    Error, // Should be used in Error handling inside remove operations
    #[expect(dead_code)]
    Warning, // Should be used as cautionary messages.
    Notice,
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
            StatusType::Notice => base_style.fg_color(Some(Color::Ansi(AnsiColor::White))),
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
            StatusType::Notice => "Notice",
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
    /// If we are inside a specific package when creating a Workspace manager,
    /// this will be the name of the package.
    current_package: Option<String>,
}

impl WorkspaceManager {
    pub(crate) fn get_current_package_manager(&self) -> Option<&PackageManager> {
        self.current_package.as_ref().map(|name| {
            self.package_manifests
                .get(name)
                .expect("current package should exist")
        })
    }
}

#[derive(Debug)]
pub(crate) struct PackageManager {
    package_root: PathBuf,
    manifest: Manifest,
}

impl CargoTomlManager {
    pub(crate) fn from_cargo_toml_path(cargo_toml_path: &Path) -> anyhow::Result<Self> {
        let manifest = Manifest::from_path(cargo_toml_path).context("unable to read Cargo.toml")?;

        let manager = match (&manifest.workspace, &manifest.package) {
            (Some(_), _) => {
                let mut manager = Self::parse_workspace(cargo_toml_path, manifest);

                if let Some(package) = &manager.root_manifest.package {
                    manager.current_package = Some(package.name.clone());

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
                        let mut manager = Self::parse_workspace(&workspace_path, manifest);
                        manager.current_package = Some(package.name.clone());
                        return Ok(CargoTomlManager::Workspace(manager));
                    }
                }

                let manager = PackageManager {
                    package_root: PathBuf::from(
                        cargo_toml_path
                            .parent()
                            .expect("Cargo.toml should always have a parent"),
                    ),
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
                let member_path = workspace_root.join(member);

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
            current_package: None,
        }
    }

    pub(crate) fn from_path(path: &Path) -> anyhow::Result<Option<Self>> {
        let path = std::path::absolute(path).context("could not make the path absolute")?;
        if !path.exists() {
            bail!("path does not exist: `{}`", path.display());
        }

        Self::find_cargo_toml(&path)
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
}

impl WorkspaceManager {
    pub(crate) fn get_packages(&self) -> Vec<&PackageManager> {
        self.package_manifests.values().collect()
    }

    #[cfg(test)]
    pub(crate) fn get_root_manifest(&self) -> &Manifest {
        &self.root_manifest
    }

    pub(crate) fn get_package_manager(&self, package_name: &str) -> Option<&PackageManager> {
        self.package_manifests.get(package_name)
    }

    #[cfg(test)]
    pub(crate) fn get_package_manager_by_path(
        &self,
        package_path: &Path,
    ) -> Option<&PackageManager> {
        let mut package_path = package_path;

        if package_path.is_file() {
            package_path = package_path
                .parent()
                .expect("file path should always have a parent");
        }

        self.package_manifests
            .values()
            .find(|m| m.package_root == package_path)
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

    #[cfg(test)]
    pub(crate) fn get_manifest(&self) -> &Manifest {
        &self.manifest
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use cot::test::serial_guard;
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
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn find_cargo_toml() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_package(temp_dir.path()).unwrap();
            let cargo_toml_path = temp_dir.path().join("Cargo.toml");

            let found_path = CargoTomlManager::find_cargo_toml(temp_dir.path()).unwrap();
            assert_eq!(found_path, cargo_toml_path);
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
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
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn load_valid_virtual_workspace_manifest() {
            let cot_cli_root = env!("CARGO_MANIFEST_DIR");
            let cot_root = Path::new(cot_cli_root).parent().unwrap();

            let manager = CargoTomlManager::from_path(cot_root).unwrap().unwrap();
            match manager {
                CargoTomlManager::Workspace(manager) => {
                    assert!(manager.get_root_manifest().workspace.is_some());
                    assert!(!manager.package_manifests.is_empty());
                }
                CargoTomlManager::Package(_) => panic!("Expected workspace manifest"),
            }
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn load_valid_workspace_from_package_manifest() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
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
                    assert_eq!(manager.current_package.as_deref(), Some(package_name));
                }
                CargoTomlManager::Package(_) => panic!("Expected workspace manifest"),
            }
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn load_valid_workspace_from_workspace_manifest() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            test_utils::make_workspace_package(temp_dir.path(), 3).unwrap();

            let cargo_toml_path = temp_dir.path().join("cargo-test-crate-1");
            let manager = CargoTomlManager::from_path(&cargo_toml_path)
                .unwrap()
                .unwrap();

            match manager {
                CargoTomlManager::Workspace(manager) => {
                    assert!(manager.get_root_manifest().workspace.is_some());
                    assert_eq!(manager.get_packages().len(), 3);
                    assert_eq!(
                        manager.current_package.as_deref(),
                        Some("cargo-test-crate-1")
                    );
                }
                CargoTomlManager::Package(_) => panic!("Expected workspace manifest"),
            }
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
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
                CargoTomlManager::Workspace(_) => panic!("Expected package manifest"),
            }
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn load_valid_package_manifest_current_dir() {
            let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
            let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();
            test_utils::make_package(temp_dir.path()).unwrap();

            // ensure the tests run sequentially when setting the current directory
            let _guard = serial_guard();
            std::env::set_current_dir(temp_dir.path().join("src")).unwrap();

            let manager = CargoTomlManager::from_path(Path::new("."))
                .unwrap()
                .unwrap();

            match manager {
                CargoTomlManager::Package(manager) => {
                    assert_eq!(manager.get_package_name(), package_name);
                    assert_eq!(
                        manager.get_package_path().canonicalize().unwrap(),
                        temp_dir.path().canonicalize().unwrap(),
                    );
                }
                CargoTomlManager::Workspace(_) => panic!("Expected package manifest"),
            }
        }
    }

    mod workspace_manager {
        use super::*;

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn get_root_manifest() {
            let (temp_dir, manager) = get_workspace(1);
            let manifest_path = temp_dir.path().join("Cargo.toml");
            let orig_manifest = Manifest::from_path(&manifest_path).unwrap();

            let manifest = manager.get_root_manifest();

            assert_eq!(*manifest, orig_manifest);
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
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
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
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
    }
    mod package_manager {
        use super::*;

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn get_package_name() {
            let (temp_dir, manager) = get_package();
            let package_name = temp_dir.path().file_name().unwrap().to_str().unwrap();

            assert_eq!(manager.get_package_name(), package_name);
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn get_package_path() {
            let (temp_dir, manager) = get_package();

            assert_eq!(manager.get_package_path(), temp_dir.path());
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn get_manifest_path() {
            let (temp_dir, manager) = get_package();

            assert_eq!(
                manager.get_manifest_path(),
                temp_dir.path().join("Cargo.toml")
            );
        }

        #[test]
        // unsupported operation: can't call foreign function `OPENSSL_init_ssl` on OS `linux`
        #[cfg_attr(miri, ignore)]
        fn get_manifest() {
            let (temp_dir, manager) = get_package();
            let manifest_path = temp_dir.path().join("Cargo.toml");
            let orig_manifest = Manifest::from_path(&manifest_path).unwrap();

            assert_eq!(*manager.get_manifest(), orig_manifest);
        }
    }
}
