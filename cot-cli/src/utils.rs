use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anstyle::{AnsiColor, Color, Effects, Style};
use anyhow::{bail, Context};
use cargo_toml::{Error, Manifest, Package, Workspace};

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
    package_manifests: HashMap<String, Manifest>,
}
impl WorkspaceManager {
    pub fn from_cargo_toml_path(cargo_toml_path: PathBuf) -> anyhow::Result<Self> {
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
                        package_manifests: HashMap::new(),
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
                (member.clone(), member_manifest)
            })
            .collect();

        Self {
            root_manifest: manifest,
            package_manifests,
        }
    }

    pub fn from_path(path: &Path) -> anyhow::Result<Self> {
        let cargo_toml_path = Self::find_cargo_toml(path)
            .ok_or_else(|| anyhow::anyhow!("Cargo.toml not found in the package"))?;
        Self::from_cargo_toml_path(cargo_toml_path)
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
}

#[cfg(test)]
mod tests {

    use super::*;
    #[test]
    fn find_cargo_toml() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        std::fs::write(&cargo_toml_path, "").unwrap();

        let found_path = WorkspaceManager::find_cargo_toml(&temp_dir.path()).unwrap();
        assert_eq!(found_path, cargo_toml_path);
    }

    #[test]
    fn find_cargo_toml_recursive() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_dir = temp_dir.path().join("nested");
        std::fs::create_dir(&nested_dir).unwrap();
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        std::fs::write(&cargo_toml_path, "").unwrap();

        let found_path = WorkspaceManager::find_cargo_toml(&temp_dir.path()).unwrap();
        assert_eq!(found_path, cargo_toml_path);
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

        let manifest = WorkspaceManager::from_path(&cot_root).unwrap();

        assert!(manifest.root_manifest.workspace.is_some());
        assert!(manifest.package_manifests.len() > 0);
    }

    #[test]
    fn load_valid_workspace_from_package_manifest() {
        let cot_cli_root = env!("CARGO_MANIFEST_DIR");

        let manifest = WorkspaceManager::from_path(Path::new(cot_cli_root)).unwrap();

        assert!(manifest.root_manifest.workspace.is_some());
        assert!(manifest.package_manifests.len() > 0);
    }

    // TODO: test Cargo.toml with package and manifest in one file
}
