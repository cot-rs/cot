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
pub(crate) struct WorkspaceManager {
    root_manifest: Manifest,
    package_manifests: HashMap<String, ManifestEntry>,
}

#[derive(Debug)]
struct ManifestEntry {
    path: PathBuf,
    manifest: Manifest,
}
impl WorkspaceManager {
    pub fn from_cargo_toml_path(cargo_toml_path: PathBuf) -> anyhow::Result<Self> {
        let cargo_toml_path = cargo_toml_path
            .canonicalize()
            .context("unable to canonicalize path")?;

        let manifest =
            Manifest::from_path(&cargo_toml_path).with_context(|| "unable to read Cargo.toml")?;

        let manager = match (&manifest.workspace, &manifest.package) {
            (Some(_), _) => {
                let mut manager = Self::parse_workspace(&cargo_toml_path, manifest);

                if let Some(package) = &manager.root_manifest.package {
                    if let None = manager.get_package_manifest(package.name()) {
                        let mut workspace = manager
                            .root_manifest
                            .workspace
                            .as_mut()
                            .expect("workspace is known to be present");

                        if !workspace.members.contains(&package.name) {
                            let package_name = package.name().to_string();
                            workspace.members.push(package_name.clone());

                            let entry = ManifestEntry {
                                path: cargo_toml_path,
                                manifest: manager.root_manifest.clone(),
                            };

                            manager
                                .package_manifests
                                .insert(package_name.clone(), entry);
                        }
                    }
                }

                manager
            }

            (None, Some(package)) => {
                let workspace_path = match package.workspace {
                    Some(ref workspace) => Path::new(workspace),
                    None => cargo_toml_path
                        .parent()
                        .expect("Cargo.toml should always have a parent")
                        .parent()
                        .unwrap_or(Path::new(".")),
                }
                .join("Cargo.toml");

                match Manifest::from_path(&workspace_path) {
                    Ok(workspace) if workspace.workspace.is_some() => {
                        Self::parse_workspace(&workspace_path, workspace)
                    }
                    _ => Self {
                        root_manifest: manifest,
                        package_manifests: HashMap::new(),
                    },
                }
            }

            _ => {
                bail!("Cargo.toml is not a valid workspace or package manifest");
            }
        };

        Ok(manager)
    }

    fn parse_workspace(cargo_toml_path: &PathBuf, manifest: Manifest) -> WorkspaceManager {
        assert!(manifest.workspace.is_some());
        let workspace = manifest.workspace.as_ref().unwrap();
        let package_manifests = workspace
            .members
            .iter()
            .map(|member| {
                let member_path = cargo_toml_path
                    .parent()
                    .expect("Cargo.toml should always have a parent")
                    .join(member)
                    .join("Cargo.toml");

                let member_manifest =
                    Manifest::from_path(&member_path).expect("member manifests should be valid");

                let entry = ManifestEntry {
                    path: member_path,
                    manifest: member_manifest,
                };

                (member.clone(), entry)
            })
            .collect();

        Self {
            root_manifest: manifest,
            package_manifests,
        }
    }

    pub fn from_path(path: &Path) -> anyhow::Result<Option<Self>> {
        let path = path.canonicalize().context("unable to canonicalize path")?;
        Self::find_cargo_toml(&path)
            .map(|cargo_toml_path| Self::from_cargo_toml_path(cargo_toml_path))
            .transpose()
    }

    pub fn find_cargo_toml(starting_dir: &Path) -> Option<PathBuf> {
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

    pub fn get_packages(&self) -> Vec<String> {
        self.package_manifests.keys().cloned().collect()
    }

    pub fn get_package_manifest(&self, package_name: &str) -> Option<&Manifest> {
        self.package_manifests
            .get(package_name)
            .map(|m| &m.manifest)
    }

    pub fn get_package_manifest_by_path(&self, package_path: &Path) -> Option<&Manifest> {
        let mut package_path = package_path
            .canonicalize()
            .context("unable to canonicalize path")
            .ok()?;

        if package_path.is_dir() {
            package_path = package_path.join("Cargo.toml");
        }

        self.package_manifests.values().find_map(|m| {
            if m.path == package_path {
                Some(&m.manifest)
            } else {
                None
            }
        })
    }

    pub fn get_manifest_path(&self, package_name: &str) -> Option<&Path> {
        self.package_manifests
            .get(package_name)
            .map(|m| m.path.as_path())
    }
}

#[cfg(test)]
mod tests {

    use std::io::Write;

    use cargo;
    use cargo::ops::NewProjectKind;

    use super::*;

    const WORKSPACE_STUB: &'static str = "[workspace]\nresolver = \"3\"";

    fn make_workspace_package(path: PathBuf, packages: u8) -> anyhow::Result<()> {
        let workspace_cargo_toml = path.join("Cargo.toml");
        std::fs::write(workspace_cargo_toml, WORKSPACE_STUB)?;

        for i in 0..packages {
            let package_path = path.join(format!("cargo-test-crate-{i}"));
            make_package(package_path)?;
        }

        Ok(())
    }
    fn make_package(path: PathBuf) -> anyhow::Result<()> {
        let new_options = cargo::ops::NewOptions {
            version_control: None,
            kind: NewProjectKind::Lib,
            auto_detect_kind: false,
            path: path.clone(),
            name: None,
            edition: None,
            registry: None,
        };
        let global_context = cargo::GlobalContext::default()?;
        match path.exists() {
            true => cargo::ops::init(&new_options, &global_context).map(|_| ()),
            false => cargo::ops::new(&new_options, &global_context),
        }
    }

    use super::*;
    #[test]
    fn find_cargo_toml() {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        make_package(temp_dir.path().into()).unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");

        let found_path = WorkspaceManager::find_cargo_toml(&temp_dir.path()).unwrap();
        assert_eq!(found_path, cargo_toml_path);
    }

    #[test]
    fn find_cargo_toml_recursive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_dir = temp_dir.path().join("nested");
        make_package(nested_dir.clone()).unwrap();

        let found_path = WorkspaceManager::find_cargo_toml(&nested_dir).unwrap();
        assert_eq!(found_path, nested_dir.join("Cargo.toml"));
    }

    #[test]
    fn find_cargo_toml_not_found() {
        let temp_dir = tempfile::tempdir().unwrap();
        let found_path = WorkspaceManager::find_cargo_toml(&temp_dir.path());
        assert!(found_path.is_none());
    }

    #[test]
    fn load_valid_virtual_workspace_manifest() {
        let cot_cli_root = env!("CARGO_MANIFEST_DIR");
        let cot_root = Path::new(cot_cli_root).parent().unwrap();

        let manifest = WorkspaceManager::from_path(&cot_root).unwrap().unwrap();

        assert!(manifest.root_manifest.workspace.is_some());
        assert!(manifest.package_manifests.len() > 0);
    }

    #[test]
    fn load_valid_workspace_from_package_manifest() {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        make_package(temp_dir.path().into()).unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        let mut handle = std::fs::OpenOptions::new()
            .append(true)
            .open(&cargo_toml_path)
            .unwrap();
        writeln!(handle, "{WORKSPACE_STUB}").unwrap();

        let manifest = WorkspaceManager::from_path(&temp_dir.path())
            .unwrap()
            .unwrap();

        assert!(manifest.root_manifest.workspace.is_some());
        assert_eq!(manifest.package_manifests.len(), 1);
        assert_eq!(
            manifest
                .package_manifests
                .get(temp_dir.path().file_name().unwrap().to_str().unwrap())
                .unwrap()
                .path,
            cargo_toml_path
        );
    }

    #[test]
    fn test_get_package_manifest() {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        make_workspace_package(temp_dir.path().to_path_buf(), 1).unwrap();

        let workspace = WorkspaceManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap();

        let first_package = &workspace.get_packages()[0];
        let manifest = workspace.get_package_manifest(first_package);
        assert!(manifest.is_some());
        assert_eq!(
            manifest.unwrap().package.as_ref().unwrap().name,
            *first_package
        );

        let manifest = workspace.get_package_manifest("non-existent");
        assert!(manifest.is_none());
    }

    #[test]
    fn test_get_package_manifest_by_path() {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        make_workspace_package(temp_dir.path().to_path_buf(), 1).unwrap();

        let workspace = WorkspaceManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap();

        let first_package = &workspace.get_packages()[0];
        let first_package_path = temp_dir.path().join(format!("{first_package}/"));
        let manifest = workspace.get_package_manifest_by_path(&first_package_path);
        assert!(manifest.is_some());
        assert_eq!(
            manifest.unwrap().package.as_ref().unwrap().name,
            *first_package
        );

        let first_package_path = temp_dir.path().join(first_package).join("Cargo.toml");
        let manifest = workspace.get_package_manifest_by_path(&first_package_path);
        assert!(manifest.is_some());
        assert_eq!(
            manifest.unwrap().package.as_ref().unwrap().name,
            *first_package
        );

        let non_existent = temp_dir.path().join("non-existent/Cargo.toml");
        let manifest = workspace.get_package_manifest_by_path(&non_existent);
        assert!(manifest.is_none());
    }

    #[test]
    fn test_get_manifest_path() {
        let temp_dir = tempfile::TempDir::with_prefix("cot-test-").unwrap();
        make_workspace_package(temp_dir.path().to_path_buf(), 1).unwrap();

        let workspace = WorkspaceManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap();

        let first_package = &workspace.get_packages()[0];
        let first_package_path = temp_dir.path().join(first_package);
        let path = workspace.get_manifest_path(&first_package);
        assert!(path.is_some());
        assert_eq!(path.unwrap(), first_package_path.join("Cargo.toml"));

        let path = workspace.get_manifest_path("non-existent");
        assert!(path.is_none());
    }
}
