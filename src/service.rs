use std::path::{Path, PathBuf};

const LABEL: &str = "com.diffodil.server";

pub fn install(root: &Path, port: u16, print: bool) {
    #[cfg(target_os = "macos")]
    {
        install_macos(root, port, print);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (root, port, print);
        unsupported();
    }
}

pub fn uninstall() {
    #[cfg(target_os = "macos")]
    {
        uninstall_macos();
    }
    #[cfg(not(target_os = "macos"))]
    {
        unsupported();
    }
}

pub fn restart() {
    #[cfg(target_os = "macos")]
    {
        restart_macos();
    }
    #[cfg(not(target_os = "macos"))]
    {
        unsupported();
    }
}

#[cfg(not(target_os = "macos"))]
fn unsupported() -> ! {
    fail(&format!(
        "installing diffodil as a service is not supported on {} yet (only macOS). \
         Run diffodil directly, or under your platform's service manager.",
        std::env::consts::OS,
    ));
}

#[cfg(target_os = "macos")]
fn install_macos(root: &Path, port: u16, print: bool) {
    let exe = match current_exe() {
        Ok(p) => p,
        Err(e) => fail(&format!("cannot resolve the diffodil binary path: {e}")),
    };

    let root = std::fs::canonicalize(root).unwrap_or_else(|e| {
        fail(&format!(
            "cannot resolve root path '{}': {e}",
            root.display()
        ));
    });

    let plist = render_plist(&exe, &root, port);

    if print {
        print!("{plist}");
        return;
    }

    if exe.components().any(|c| c.as_os_str() == "target") {
        eprintln!(
            "warning: installing from a build-artifact path:\n  {}\n\
             A `cargo clean` or rebuild will break the service. Consider running\n\
             `just install` first to put the binary in ~/.cargo/bin.\n",
            exe.display()
        );
    }

    let plist_path = match plist_path() {
        Some(p) => p,
        None => fail("cannot resolve ~/Library/LaunchAgents (is $HOME set?)"),
    };

    if let Some(dir) = plist_path.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            fail(&format!("cannot create {}: {e}", dir.display()));
        }
    }
    if let Err(e) = std::fs::write(&plist_path, &plist) {
        fail(&format!("cannot write {}: {e}", plist_path.display()));
    }
    println!("wrote {}", plist_path.display());

    let domain = gui_domain();
    let service_target = format!("{domain}/{LABEL}");

    run_launchctl(&["bootout", &service_target], true);
    run_launchctl(&["enable", &service_target], true);
    if !run_launchctl(
        &["bootstrap", &domain, plist_path.to_string_lossy().as_ref()],
        false,
    ) {
        fail("launchctl bootstrap failed (see output above)");
    }

    println!(
        "diffodil service installed and started.\n\n\
         It is now running at http://0.0.0.0:{port} and will start at login.\n\
         Watching: {root}\n\n\
           Status:  launchctl print {service_target}\n\
           Logs:    tail -f {log}\n\
           Stop:    diffodil uninstall\n",
        root = root.display(),
        log = log_path()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );
}

#[cfg(target_os = "macos")]
fn restart_macos() {
    let service_target = format!("{}/{LABEL}", gui_domain());
    if !run_launchctl(&["kickstart", "-k", &service_target], false) {
        fail("failed to restart the diffodil service — is it installed?");
    }
    println!("diffodil service restarted.");
}

#[cfg(target_os = "macos")]
fn uninstall_macos() {
    let plist_path = match plist_path() {
        Some(p) => p,
        None => fail("cannot resolve ~/Library/LaunchAgents (is $HOME set?)"),
    };

    let service_target = format!("{}/{LABEL}", gui_domain());
    run_launchctl(&["bootout", &service_target], true);

    match std::fs::remove_file(&plist_path) {
        Ok(()) => println!("removed {}", plist_path.display()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!("no plist at {} — nothing to remove", plist_path.display());
        }
        Err(e) => fail(&format!("cannot remove {}: {e}", plist_path.display())),
    }
    println!("diffodil service uninstalled.");
}

#[cfg(target_os = "macos")]
fn render_plist(exe: &Path, root: &Path, port: u16) -> String {
    let log = log_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/tmp/diffodil.log".to_string());

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>{root}</string>
        <string>--port</string>
        <string>{port}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
</dict>
</plist>
"#,
        label = LABEL,
        exe = xml_escape(&exe.to_string_lossy()),
        root = xml_escape(&root.to_string_lossy()),
        port = port,
        log = xml_escape(&log),
    )
}

#[cfg(target_os = "macos")]
fn current_exe() -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(std::fs::canonicalize(&exe).unwrap_or(exe))
}

#[cfg(target_os = "macos")]
fn plist_path() -> Option<PathBuf> {
    home().map(|h| {
        h.join("Library")
            .join("LaunchAgents")
            .join(format!("{LABEL}.plist"))
    })
}

#[cfg(target_os = "macos")]
fn log_path() -> Option<PathBuf> {
    home().map(|h| h.join("Library").join("Logs").join("diffodil.log"))
}

#[cfg(target_os = "macos")]
fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

#[cfg(target_os = "macos")]
fn gui_domain() -> String {
    let uid = unsafe { libc::getuid() };
    format!("gui/{uid}")
}

#[cfg(target_os = "macos")]
fn run_launchctl(args: &[&str], ignore_failure: bool) -> bool {
    match std::process::Command::new("launchctl").args(args).output() {
        Ok(out) => {
            if out.status.success() {
                return true;
            }
            if !ignore_failure {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if !stderr.trim().is_empty() {
                    eprintln!("launchctl {}: {}", args.join(" "), stderr.trim());
                }
            }
            false
        }
        Err(e) => {
            if !ignore_failure {
                eprintln!("failed to run launchctl: {e}");
            }
            false
        }
    }
}

#[cfg(target_os = "macos")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn fail(msg: &str) -> ! {
    eprintln!("diffodil: {msg}");
    std::process::exit(1);
}
