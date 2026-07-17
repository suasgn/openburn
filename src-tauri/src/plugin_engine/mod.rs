pub mod host_api;
pub mod manifest;
pub mod runtime;

use manifest::LoadedPlugin;
use std::path::{Path, PathBuf};

#[cfg(mobile)]
use include_dir::{Dir, include_dir};

#[cfg(mobile)]
static EMBEDDED_BUNDLED_PLUGINS: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/resources/bundled_plugins");

pub fn initialize_plugins(
    app_data_dir: &Path,
    resource_dir: &Path,
) -> (PathBuf, Vec<LoadedPlugin>) {
    if let Some(dev_dir) = find_dev_plugins_dir() {
        if !is_dir_empty(&dev_dir) {
            let plugins = manifest::load_plugins_from_dir(&dev_dir);
            return (dev_dir, plugins);
        }
    }

    let install_dir = app_data_dir.join("plugins");
    if let Err(err) = std::fs::create_dir_all(&install_dir) {
        log::warn!(
            "failed to create install dir {}: {}",
            install_dir.display(),
            err
        );
    }

    #[cfg(mobile)]
    let _ = resource_dir;

    #[cfg(mobile)]
    if !has_installed_plugins(&install_dir) {
        copy_embedded_dir(&EMBEDDED_BUNDLED_PLUGINS, &install_dir);
    }

    #[cfg(not(mobile))]
    {
        let bundled_dir = resolve_bundled_dir(resource_dir);
        if bundled_dir.exists() {
            copy_dir_recursive(&bundled_dir, &install_dir);
        }
    }

    let plugins = manifest::load_plugins_from_dir(&install_dir);
    (install_dir, plugins)
}

fn find_dev_plugins_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let direct = cwd.join("plugins");
    if direct.exists() {
        return Some(direct);
    }
    let parent = cwd.join("..").join("plugins");
    if parent.exists() {
        return Some(parent);
    }
    None
}

#[cfg(not(mobile))]
fn resolve_bundled_dir(resource_dir: &Path) -> PathBuf {
    let nested = resource_dir.join("resources/bundled_plugins");
    if nested.exists() {
        nested
    } else {
        resource_dir.join("bundled_plugins")
    }
}

fn is_dir_empty(path: &Path) -> bool {
    match std::fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_none(),
        Err(err) => {
            log::warn!("failed to read dir {}: {}", path.display(), err);
            true
        }
    }
}

#[cfg(mobile)]
fn has_installed_plugins(path: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(path) else {
        return false;
    };

    entries.flatten().any(|entry| {
        entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false)
            && entry.path().join("plugin.json").is_file()
    })
}

#[cfg(not(mobile))]
fn copy_dir_recursive(src: &Path, dst: &Path) {
    match std::fs::read_dir(src) {
        Ok(entries) => {
            for entry in entries {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(err) => {
                        log::warn!("failed to read entry in {}: {}", src.display(), err);
                        continue;
                    }
                };
                let src_path = entry.path();
                let dst_path = dst.join(entry.file_name());
                let file_type = match entry.file_type() {
                    Ok(file_type) => file_type,
                    Err(err) => {
                        log::warn!(
                            "failed to read file type for {}: {}",
                            src_path.display(),
                            err
                        );
                        continue;
                    }
                };
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    if let Err(err) = std::fs::create_dir_all(&dst_path) {
                        log::warn!("failed to create dir {}: {}", dst_path.display(), err);
                        continue;
                    }
                    copy_dir_recursive(&src_path, &dst_path);
                } else if file_type.is_file() {
                    if let Err(err) = std::fs::copy(&src_path, &dst_path) {
                        log::warn!(
                            "failed to copy {} to {}: {}",
                            src_path.display(),
                            dst_path.display(),
                            err
                        );
                    }
                }
            }
        }
        Err(err) => {
            log::warn!("failed to read dir {}: {}", src.display(), err);
        }
    }
}

#[cfg(mobile)]
fn copy_embedded_dir(src: &Dir<'_>, dst: &Path) {
    if let Err(err) = src.extract(dst) {
        log::warn!(
            "failed to extract embedded plugins to {}: {}",
            dst.display(),
            err
        );
    }
}
