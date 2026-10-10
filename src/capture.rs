use anyhow::{Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
pub fn capture(directory: &Path) -> Result<PathBuf> {
    fs::create_dir_all(directory)?;
    let path = directory.join(format!(
        "frame-{}.png",
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let output = Command::new("gamescopectl")
        .arg("screenshot")
        .arg(&path)
        .env(
            "XDG_RUNTIME_DIR",
            format!(
                "/run/user/{}",
                std::env::var("UID").unwrap_or_else(|_| "1000".into())
            ),
        )
        .env("WAYLAND_DISPLAY", "gamescope-0")
        .output()?;
    ensure!(
        output.status.success(),
        "Gamescope capture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut previous = 0;
    while Instant::now() < deadline {
        let size = fs::metadata(&path).map(|v| v.len()).unwrap_or_default();
        if size > 100 && size == previous {
            return Ok(path);
        }
        previous = size;
        thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("Capture did not finish in time")
}
