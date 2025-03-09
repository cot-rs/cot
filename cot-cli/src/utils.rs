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
    package_manifests: Vec<ManifestEntry>,
}

#[derive(Debug)]
struct ManifestEntry {
    name: String,
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
            (Some(_), _) => Self::parse_workspace(&cargo_toml_path, manifest),

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
                        package_manifests: Vec::new(),
                    },
                }
            }

            (_, _) => {
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

                ManifestEntry {
                    name: member.clone(),
                    path: member_path,
                    manifest: member_manifest,
                }
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

    pub fn get_package_manifest(&self, package_name: &str) -> Option<&Manifest> {
        self.package_manifests
            .iter()
            .find(|p| p.name == package_name)
            .map(|m| &m.manifest)
    }

    pub fn get_package_manifest_by_path(&self, package_path: &Path) -> Option<&Manifest> {
        let package_path = package_path
            .canonicalize()
            .context("unable to canonicalize path")
            .ok()?;

        self.package_manifests
            .iter()
            .find(|p| p.path == package_path)
            .map(|m| &m.manifest)
    }

    pub fn get_manifest_path(&self, package_name: &str) -> Option<&Path> {
        self.package_manifests
            .iter()
            .find(|p| p.name == package_name)
            .map(|m| m.path.as_path())
    }
}

#[cfg(test)]
mod tests {

    use std::io::Write;

    use cargo;
    use cargo::ops::NewProjectKind;
    use tempfile::TempDir;

    use super::*;

    fn create_safe_tempdir(path: Option<PathBuf>) -> std::io::Result<TempDir> {
        let mut builder = tempfile::Builder::new();
        builder.prefix("cargo-cot");
        match path {
            Some(path) => builder.tempdir_in(path),
            None => builder.tempdir(),
        }
    }
    fn make_workspace_package(path: PathBuf, packages: u8) -> anyhow::Result<()> {
        make_package(path.clone())?;
        let workspace_cargo_toml = path.join("Cargo.toml");
        let mut cargo_toml = std::fs::OpenOptions::new()
            .append(true)
            .open(workspace_cargo_toml.clone())?;
        writeln!(&mut cargo_toml, "[workspace]")?;

        for _ in 0..packages {
            let package_path = create_safe_tempdir(Some(path.clone()))?;
            make_package(package_path.into_path())?;
        }

        Ok(())
    }
    fn make_package(path: PathBuf) -> anyhow::Result<()> {
        let new_options = cargo::ops::NewOptions {
            version_control: None,
            kind: NewProjectKind::Lib,
            auto_detect_kind: false,
            path,
            name: None,
            edition: None,
            registry: None,
        };
        let global_context = cargo::GlobalContext::default()?;
        cargo::ops::init(&new_options, &global_context).map(|_| ())
    }

    use super::*;
    #[test]
    fn find_cargo_toml() {
        let temp_dir = create_safe_tempdir(None).unwrap();
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
    fn load_valid_workspace_manifest() {
        let cot_cli_root = env!("CARGO_MANIFEST_DIR");
        let cot_root = Path::new(cot_cli_root).parent().unwrap();

        let manifest = WorkspaceManager::from_path(&cot_root).unwrap().unwrap();

        assert!(manifest.root_manifest.workspace.is_some());
        assert!(manifest.package_manifests.len() > 0);
    }

    #[test]
    fn load_valid_workspace_from_package_manifest() {
        let temp_dir = create_safe_tempdir(None).unwrap();
        make_workspace_package(temp_dir.path().into(), 2).unwrap();

        let manifest = WorkspaceManager::from_path(&temp_dir.path())
            .unwrap()
            .unwrap();

        assert!(manifest.root_manifest.workspace.is_some());
        assert_eq!(manifest.package_manifests.len(), 2);
    }

    #[test]
    fn test_get_package_manifest() {
        let temp_dir = create_safe_tempdir(None).unwrap();
        make_workspace_package(temp_dir.path().to_path_buf(), 1).unwrap();

        let workspace = WorkspaceManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap();

        let first_package = &workspace.package_manifests[0];
        let manifest = workspace.get_package_manifest(&first_package.name);
        assert!(manifest.is_some());
        assert_eq!(manifest.unwrap(), &first_package.manifest);

        let manifest = workspace.get_package_manifest("non-existent");
        assert!(manifest.is_none());
    }

    #[test]
    fn test_get_package_manifest_by_path() {
        let temp_dir = create_safe_tempdir(None).unwrap();
        make_workspace_package(temp_dir.path().to_path_buf(), 1).unwrap();

        let workspace = WorkspaceManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap();

        let first_package = &workspace.package_manifests[0];
        let manifest = workspace.get_package_manifest_by_path(&first_package.path);
        assert!(manifest.is_some());
        assert_eq!(manifest.unwrap(), &first_package.manifest);

        let non_existent = temp_dir.path().join("non-existent/Cargo.toml");
        let manifest = workspace.get_package_manifest_by_path(&non_existent);
        assert!(manifest.is_none());
    }

    #[test]
    fn test_get_manifest_path() {
        let temp_dir = create_safe_tempdir(None).unwrap();
        make_workspace_package(temp_dir.path().to_path_buf(), 1).unwrap();

        let workspace = WorkspaceManager::from_path(temp_dir.path())
            .unwrap()
            .unwrap();

        let first_package = &workspace.package_manifests[0];
        let path = workspace.get_manifest_path(&first_package.name);
        assert!(path.is_some());
        assert_eq!(path.unwrap(), first_package.path.as_path());

        let path = workspace.get_manifest_path("non-existent");
        assert!(path.is_none());
    }
}
