use anyhow::{ensure, Result};
pub fn install() -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let exe = exe
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Non-UTF8 executable path"))?;
    ensure!(
        !exe.contains(['\n', '\r', '"', '%']),
        "Unsupported characters in executable path"
    );
    #[cfg(target_os = "linux")]
    {
        let dir = std::path::PathBuf::from(std::env::var("HOME")?).join(".config/systemd/user");
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join("den-host.service"), format!("[Unit]\nDescription=Den terminal host\nAfter=network-online.target\n[Service]\nExecStart=\"{exe}\" run\nRestart=always\nRestartSec=3\n[Install]\nWantedBy=default.target\n"))?;
        ensure!(
            std::process::Command::new("systemctl")
                .args(["--user", "daemon-reload"])
                .status()?
                .success(),
            "systemd reload failed"
        );
        ensure!(
            std::process::Command::new("systemctl")
                .args(["--user", "enable", "--now", "den-host.service"])
                .status()?
                .success(),
            "systemd start failed"
        );
    }
    #[cfg(target_os = "macos")]
    {
        let dir = std::path::PathBuf::from(std::env::var("HOME")?).join("Library/LaunchAgents");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("com.den.host.plist");
        let escaped = exe
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");
        std::fs::write(&path, format!("<?xml version=\"1.0\"?><plist version=\"1.0\"><dict><key>Label</key><string>com.den.host</string><key>ProgramArguments</key><array><string>{escaped}</string><string>run</string></array><key>RunAtLoad</key><true/><key>KeepAlive</key><true/></dict></plist>"))?;
        let uid = String::from_utf8(std::process::Command::new("id").arg("-u").output()?.stdout)?;
        ensure!(
            std::process::Command::new("launchctl")
                .args(["bootstrap", &format!("gui/{}", uid.trim())])
                .arg(path)
                .status()?
                .success(),
            "launchd bootstrap failed"
        );
    }
    #[cfg(target_os = "windows")]
    {
        ensure!(
            std::process::Command::new("schtasks")
                .args([
                    "/Create",
                    "/TN",
                    "Den Host",
                    "/TR",
                    &format!("\"{exe}\" run"),
                    "/SC",
                    "ONLOGON",
                    "/RL",
                    "LIMITED",
                    "/IT",
                    "/F"
                ])
                .status()?
                .success(),
            "Scheduled Task registration failed"
        );
        ensure!(
            std::process::Command::new("schtasks")
                .args(["/Run", "/TN", "Den Host"])
                .status()?
                .success(),
            "Scheduled Task start failed"
        );
    }
    println!("Installed den-host for the current user");
    Ok(())
}
