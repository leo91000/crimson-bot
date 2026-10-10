use anyhow::{Context, Result, ensure};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn default_save_root() -> Result<PathBuf> {
    let home = std::env::var_os("HOME").context("HOME missing")?;
    Ok(PathBuf::from(home).join(".local/share/Steam/steamapps/compatdata/3321460/pfx/drive_c/users/steamuser/AppData/Local/Pearl Abyss/CD/save"))
}

fn list_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = vec![];
    fn walk(root: &Path, at: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(at)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            ensure!(!kind.is_symlink(), "Save tree contains a symlink");
            if kind.is_dir() {
                walk(root, &entry.path(), files)?;
                continue;
            }
            ensure!(kind.is_file(), "Save tree contains a special file");
            files.push(entry.path().strip_prefix(root)?.to_path_buf());
        }
        Ok(())
    }
    walk(root, root, &mut files)?;
    files.sort();
    ensure!(
        files
            .iter()
            .any(|p| p.file_name().is_some_and(|n| n == "save.save")),
        "No game save files found"
    );
    Ok(files)
}

pub fn snapshot(source: &Path, state: &Path) -> Result<PathBuf> {
    fs::create_dir_all(state)?;
    let source_root = source.canonicalize()?;
    let state_root = state.canonicalize()?;
    ensure!(
        !source_root.starts_with(&state_root) && !state_root.starts_with(&source_root),
        "Backup source overlaps destination"
    );
    fs::create_dir_all(state.join("backups"))?;
    fs::set_permissions(state, fs::Permissions::from_mode(0o700))?;
    let id = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let destination = state.join("backups").join(id.to_string());
    fs::create_dir(&destination)?;
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o700))?;
    let result = (|| -> Result<()> {
        let files = list_files(source)?;
        let mut manifest = vec![];
        for relative in &files {
            let from = source.join(relative);
            let bytes = fs::read(&from)?;
            ensure!(
                bytes.len() <= 64 * 1024 * 1024,
                "Unexpectedly large save file"
            );
            let to = destination.join(relative);
            fs::create_dir_all(to.parent().unwrap())?;
            fs::write(&to, &bytes)?;
            fs::set_permissions(&to, fs::Permissions::from_mode(0o600))?;
            ensure!(fs::read(&to)? == bytes, "Backup verification failed");
            manifest.push(json!({"path":relative,"bytes":bytes.len(),"sha256":format!("{:x}",Sha256::digest(&bytes))}));
        }
        ensure!(
            list_files(source)? == files,
            "Save tree changed while copying; retry later"
        );
        for entry in &manifest {
            let relative = entry["path"].as_str().context("Invalid backup manifest")?;
            let current = fs::read(source.join(relative))?;
            ensure!(
                format!("{:x}", Sha256::digest(&current)) == entry["sha256"].as_str().unwrap(),
                "Save changed during backup; retry later"
            );
        }
        fs::write(
            destination.join("manifest.json"),
            serde_json::to_vec_pretty(&json!({"files":manifest,"verified":true}))?,
        )?;
        Ok(())
    })();
    if let Err(error) = result {
        fs::remove_dir_all(&destination)?;
        return Err(error);
    }
    Ok(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_preserves_source_and_rejects_symlinks() {
        let root = std::env::temp_dir().join(format!(
            "crimson-backup-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source = root.join("save");
        let state = root.join("state");
        fs::create_dir_all(source.join("slot0")).unwrap();
        fs::write(source.join("slot0/save.save"), b"synthetic-game-state").unwrap();
        let destination = snapshot(&source, &state).unwrap();
        assert_eq!(
            fs::read(destination.join("slot0/save.save")).unwrap(),
            b"synthetic-game-state"
        );
        assert_eq!(
            fs::read(source.join("slot0/save.save")).unwrap(),
            b"synthetic-game-state"
        );
        std::os::unix::fs::symlink("/etc/passwd", source.join("link")).unwrap();
        assert!(snapshot(&source, &state).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
