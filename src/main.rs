use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::SystemTime,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    checks: Vec<Check>,
    brewfile: Option<String>,
    standalone_casks: Option<std::collections::BTreeMap<String, String>>,
    dotfiles: Option<String>,
    repo_list: Option<String>,
    repo_root: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Check {
    Tool {
        name: String,
        optional: Option<bool>,
        version: Option<bool>,
    },
    XcodeSelection {
        name: String,
        path: String,
    },
    PluginLock {
        name: String,
        lockfile: String,
        directory: String,
    },
    Path {
        name: String,
        path: String,
        optional: Option<bool>,
    },
    Link {
        name: String,
        path: String,
        target: String,
    },
    Repo {
        name: String,
        path: String,
    },
    Backup {
        name: String,
        directory: String,
        prefix: String,
        max_age_days: u32,
        artifact: Option<String>,
    },
    Manual {
        name: String,
        note: String,
    },
}

#[derive(Serialize)]
struct Status {
    section: &'static str,
    name: String,
    state: &'static str,
    detail: String,
}

fn expand(path: &str, home: &Path, base: &Path) -> PathBuf {
    if path == "~" {
        return home.into();
    }
    if let Some(rest) = path.strip_prefix("~/") {
        return home.join(rest);
    }
    base.join(path)
}

fn evaluate(check: Check, home: &Path, base: &Path) -> Status {
    let path = |s: &str| expand(s, home, base);
    let presence = |present, optional| {
        if present {
            "present"
        } else if optional == Some(true) {
            "optional; not found"
        } else {
            "missing"
        }
    };
    let section = match &check {
        Check::Tool {
            optional: Some(true),
            ..
        }
        | Check::Path {
            optional: Some(true),
            ..
        } => "Optional installs",
        Check::Tool { .. } => "Command-line tools",
        Check::XcodeSelection { .. } => "Developer setup",
        Check::PluginLock { .. } => "Editor health",
        Check::Path { .. } => "Applications and files",
        Check::Link { .. } => "Dotfiles",
        Check::Repo { .. } => "Repositories",
        Check::Backup { .. } => "Backups",
        Check::Manual { .. } => "Manual checks",
    };
    let (name, state, detail) = match check {
        Check::Tool {
            name,
            optional,
            version,
        } => {
            let found = env::split_paths(&env::var_os("PATH").unwrap_or_default())
                .map(|dir| dir.join(&name))
                .find(|p| {
                    p.metadata()
                        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                });
            let state = presence(found.is_some(), optional);
            if let (Some(bin), Some(true)) = (&found, version) {
                match Command::new(bin).arg("--version").output() {
                    Ok(out) if out.status.success() => {
                        return Status {
                            section,
                            name,
                            state: "present",
                            detail: format!(
                                "{} · {}",
                                String::from_utf8_lossy(&out.stdout)
                                    .lines()
                                    .next()
                                    .unwrap_or("version unavailable"),
                                bin.display()
                            ),
                        };
                    }
                    _ => {
                        return Status {
                            section,
                            name,
                            state: "version check failed",
                            detail: bin.display().to_string(),
                        };
                    }
                }
            }
            (
                name,
                state,
                found.map_or_else(|| "not on PATH".into(), |p| p.display().to_string()),
            )
        }
        Check::XcodeSelection {
            name,
            path: expected,
        } => match Command::new("/usr/bin/xcode-select").arg("-p").output() {
            Ok(out) if out.status.success() => {
                let selected = String::from_utf8_lossy(&out.stdout).trim().to_owned();
                (
                    name,
                    if Path::new(&selected) == path(&expected) {
                        "configured"
                    } else {
                        "not selected"
                    },
                    format!("Selected: {selected}; expected: {expected}"),
                )
            }
            _ => (
                name,
                "unavailable",
                "Cannot read active developer directory".into(),
            ),
        },
        Check::PluginLock {
            name,
            lockfile,
            directory,
        } => {
            #[derive(Deserialize)]
            struct Pin {
                commit: String,
            }
            match fs::read(path(&lockfile)).ok().and_then(|data| {
                serde_json::from_slice::<std::collections::BTreeMap<String, Pin>>(&data).ok()
            }) {
                None => (
                    name,
                    "unavailable",
                    format!("Cannot read plugin lockfile: {lockfile}"),
                ),
                Some(pins) => {
                    let mut issues = Vec::new();
                    for (plugin, pin) in &pins {
                        if plugin.is_empty()
                            || !plugin
                                .chars()
                                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
                            || plugin == "."
                            || plugin == ".."
                        {
                            issues.push(format!("Invalid plugin name: {plugin}"));
                            continue;
                        }
                        let checkout = path(&directory).join(plugin);
                        if !checkout.join(".git").exists() {
                            issues.push(format!("Missing: {plugin}"));
                            continue;
                        }
                        let head = Command::new("git")
                            .arg("-C")
                            .arg(checkout)
                            .args(["rev-parse", "HEAD"])
                            .output();
                        match head {
                            Ok(out)
                                if out.status.success()
                                    && String::from_utf8_lossy(&out.stdout).trim()
                                        == pin.commit =>
                            {
                                ()
                            }
                            Ok(out) if out.status.success() => {
                                issues.push(format!("Different commit: {plugin}"))
                            }
                            _ => issues.push(format!("Cannot check: {plugin}")),
                        }
                    }
                    let state = if issues.is_empty() {
                        "matches lockfile"
                    } else {
                        "plugin drift"
                    };
                    (
                        name,
                        state,
                        format!(
                            "{}/{} match the lockfile (not a runtime health check)\n{}",
                            pins.len() - issues.len(),
                            pins.len(),
                            issues.join("\n")
                        ),
                    )
                }
            }
        }
        Check::Path {
            name,
            path: p,
            optional,
        } => (name, presence(path(&p).exists(), optional), p),
        Check::Link {
            name,
            path: p,
            target,
        } => {
            let local = path(&p);
            let state = match fs::symlink_metadata(&local) {
                Ok(m) if m.is_symlink() => {
                    match (local.canonicalize(), path(&target).canonicalize()) {
                        (Ok(a), Ok(b)) if a == b => "configured",
                        (Ok(_), Ok(_)) => "wrong target",
                        _ => "unresolved link",
                    }
                }
                Ok(_) => "not a symlink",
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => "missing",
                Err(_) => "unavailable",
            };
            (name, state, format!("{p} → {target}"))
        }
        Check::Repo { name, path: p } => {
            let local = path(&p);
            let git = |args: &[&str]| {
                Command::new("git")
                    .env("GIT_OPTIONAL_LOCKS", "0")
                    .arg("-C")
                    .arg(&local)
                    .args(args)
                    .output()
                    .ok()
                    .filter(|out| out.status.success())
            };
            let (state, detail) = if !local.join(".git").exists() {
                ("missing", p)
            } else if let Some(out) = git(&["status", "--porcelain"]) {
                let dirty = !out.stdout.is_empty();
                let counts = git(&["rev-list", "--left-right", "--count", "HEAD...@{upstream}"])
                    .map(|o| {
                        String::from_utf8_lossy(&o.stdout)
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" / ")
                    });
                (
                    if dirty {
                        "local changes"
                    } else {
                        match counts.as_deref() {
                            Some("0 / 0") => "clean",
                            Some(_) => "upstream differs",
                            None => "upstream unknown",
                        }
                    },
                    counts.map_or_else(
                        || "upstream comparison unavailable".into(),
                        |c| format!("{c} ahead / behind (local tracking ref; no fetch)"),
                    ),
                )
            } else {
                ("unavailable", p)
            };
            (name, state, detail)
        }
        Check::Backup {
            name,
            directory,
            prefix,
            max_age_days,
            artifact,
        } => {
            let result = (|| -> std::io::Result<Option<(SystemTime, String)>> {
                let mut latest = None;
                for entry in fs::read_dir(path(&directory))? {
                    let entry = entry?;
                    let filename = entry.file_name().to_string_lossy().into_owned();
                    if !filename.starts_with(&prefix) || !entry.file_type()?.is_dir() {
                        continue;
                    }
                    let marker = entry.path().join(".complete");
                    let metadata = match marker.metadata() {
                        Ok(m) if m.is_file() => m,
                        Ok(_) => continue,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                        Err(e) => return Err(e),
                    };
                    if let Some(item) = &artifact {
                        let relative = Path::new(item);
                        if !relative
                            .components()
                            .all(|part| matches!(part, std::path::Component::Normal(_)))
                            || item.is_empty()
                        {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidInput,
                                "invalid snapshot artifact path",
                            ));
                        }
                        if !entry.path().join(relative).exists() {
                            continue;
                        }
                    }
                    let time = metadata.modified()?;
                    if latest.as_ref().is_none_or(|(old, _)| time > *old) {
                        latest = Some((time, filename));
                    }
                }
                Ok(latest)
            })();
            let (state, detail) = match result {
                Ok(Some((time, file))) => match time.elapsed() {
                    Ok(age) => (
                        if age.as_secs() >= u64::from(max_age_days) * 86400 {
                            "stale"
                        } else {
                            "recent"
                        },
                        format!(
                            "{file} ({} days old; restore not verified)",
                            age.as_secs() / 86400
                        ),
                    ),
                    Err(_) => (
                        "unknown",
                        format!("{file}: completion timestamp is in the future"),
                    ),
                },
                Ok(None) => (
                    "not verified",
                    "no snapshot with a completion marker".into(),
                ),
                Err(e) => ("unavailable", e.to_string()),
            };
            (name, state, detail)
        }
        Check::Manual { name, note } => (name, "manual check", note),
    };
    Status {
        section,
        name,
        state,
        detail,
    }
}

#[cfg(feature = "gpui")]
mod gui;

fn native_gui(rows: &[Status], config: &Path) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    use std::process::Stdio;

    let data = serde_json::to_vec(rows)?;
    let mut child = Command::new("/usr/bin/swift")
        .args(["-e", include_str!("dashboard.swift")])
        .env("MAC_STATUS_REFRESH_EXE", env::current_exe()?)
        .env(
            "MAC_STATUS_REFRESH_CONFIG",
            env::current_dir()?.join(config),
        )
        .stdin(Stdio::piped())
        .spawn()?;
    let sent = child
        .stdin
        .take()
        .ok_or("dashboard stdin unavailable")?
        .write_all(&data);
    let status = child.wait()?;
    sent?;
    if !status.success() {
        return Err(format!("native dashboard exited with {status}").into());
    }
    Ok(())
}

// Read the declarative subset used by the installer without evaluating Ruby.
fn brew_rows(
    file: &Path,
    standalone: &std::collections::BTreeMap<String, String>,
    home: &Path,
    base: &Path,
) -> Result<Vec<Status>, Box<dyn std::error::Error>> {
    let mut rows = Vec::new();
    let mut group = None;
    let installed = |kind| {
        Command::new("brew")
            .args(["list", kind, "-1"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
    };
    let formulas = installed("--formula");
    let casks = installed("--cask");
    for raw in fs::read_to_string(file)?.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("tap ") {
            continue;
        }
        if let Some(key) = line
            .strip_prefix("if ENV[\"")
            .and_then(|s| s.strip_suffix("\"] == \"1\""))
        {
            if group.is_some() {
                return Err("Nested Brewfile groups are unsupported".into());
            }
            group = Some(key.to_owned());
            continue;
        }
        if line == "end" && group.take().is_some() {
            continue;
        }
        let (kind, token, inventory) = if let Some(s) = line.strip_prefix("brew ") {
            ("formula", s, &formulas)
        } else if let Some(s) = line.strip_prefix("cask ") {
            ("cask", s, &casks)
        } else {
            return Err(format!("Unsupported Brewfile entry: {line}").into());
        };
        let token = token
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .filter(|s| !s.is_empty() && !s.contains('"'))
            .ok_or("Invalid Brewfile package")?;
        let short = token.rsplit('/').next().unwrap_or(token);
        let selected = group.as_ref().and_then(|g| {
            let path = env::var_os("MAC_STATUS_SELECTION")?;
            let text = fs::read_to_string(path).ok()?;
            Some(text.lines().any(|line| line == g))
        });
        let state = match inventory {
            Some(list) if list.lines().any(|name| name == token || name == short) => "present",
            _ if kind == "cask"
                && standalone
                    .get(short)
                    .is_some_and(|p| expand(p, home, base).is_dir()) =>
            {
                "installed outside Homebrew"
            }
            _ if selected == Some(false) => "not selected",
            Some(_) if group.is_none() || selected == Some(true) => "missing",
            Some(_) => "optional; not found",
            None => "unavailable",
        };
        rows.push(Status {
            section: if group.is_some() {
                "Optional installs"
            } else if kind == "cask" {
                "Homebrew apps"
            } else {
                "Homebrew tools"
            },
            name: short.into(),
            state,
            detail: format!("{kind}: {token} (Homebrew inventory; standalone copies not counted)"),
        });
    }
    if group.is_some() {
        return Err("Unclosed Brewfile group".into());
    }
    Ok(rows)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let (mut config, mut json, mut gui, mut gpui) = (None, false, false, false);
    let mut install_launcher = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--config") if config.is_none() => {
                config = Some(PathBuf::from(args.next().ok_or("--config needs a file")?))
            }
            Some("--json") => json = true,
            Some("--gui") => gui = true,
            Some("--install-launcher") => install_launcher = true,
            Some("--gpui") => gpui = true,
            Some("--help" | "-h") => {
                println!(
                    "Usage: mac-status --config PLAN.toml [--json | --gui | --install-launcher]"
                );
                return Ok(());
            }
            _ => return Err("Usage: mac-status --config PLAN.toml [--json | --gui]".into()),
        }
    }
    if [gui, gpui, json, install_launcher]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        return Err("Choose only one output or launcher mode".into());
    }
    #[cfg(not(feature = "gpui"))]
    if gpui {
        return Err(
            "Experimental GPUI support was not built; use cargo run --features gpui".into(),
        );
    }
    let config = config.ok_or("Specify --config PLAN.toml")?;
    if install_launcher {
        let exe = env::current_exe()?.canonicalize()?;
        let binary = [
            Path::new("/opt/homebrew/bin/mac-status"),
            Path::new("/usr/local/bin/mac-status"),
        ]
        .into_iter()
        .find(|p| p.canonicalize().ok().as_ref() == Some(&exe))
        .unwrap_or(&exe);
        use std::io::Write;
        use std::process::Stdio;
        let mut child = Command::new("/bin/bash")
            .args(["-c", include_str!("install-launcher.sh"), "mac-status"])
            .arg(env::current_dir()?.join(config))
            .arg(binary)
            .stdin(Stdio::piped())
            .spawn()?;
        let written = child
            .stdin
            .take()
            .unwrap()
            .write_all(include_bytes!("AppIcon.icns"));
        let status = child.wait()?;
        if status.success() {
            written?;
        }
        return if status.success() {
            Ok(())
        } else {
            Err(format!("Launcher installer exited with {status}").into())
        };
    }
    let home = PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?);
    let mut plan: Plan = toml::from_str(&fs::read_to_string(&config)?)?;
    let base = config.parent().unwrap_or(Path::new("."));
    if let Some(list) = plan.repo_list {
        let root = plan
            .repo_root
            .as_deref()
            .ok_or("repo_list requires repo_root")?;
        for name in fs::read_to_string(expand(&list, &home, base))?
            .lines()
            .filter(|line| !line.is_empty())
        {
            if !name.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                || !name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c))
            {
                return Err(format!("invalid repo name: {name}").into());
            }
            plan.checks.push(Check::Repo {
                name: format!("{name} repo"),
                path: format!("{root}/{name}"),
            });
        }
    }
    if let Some(file) = plan.dotfiles {
        #[derive(Deserialize)]
        struct Dotfile {
            name: String,
            root: String,
            destination: String,
            source: String,
        }
        let root = plan
            .repo_root
            .as_deref()
            .ok_or("dotfiles requires repo_root")?;
        let links: Vec<Dotfile> = serde_json::from_slice(&fs::read(expand(&file, &home, base))?)?;
        for link in links {
            let dest = match link.root.as_str() {
                "config" => env::var_os("XDG_CONFIG_HOME")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".config")),
                "zsh" => env::var_os("ZDOTDIR")
                    .filter(|v| !v.is_empty())
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.clone()),
                _ => return Err(format!("Unknown dotfile root: {}", link.root).into()),
            };
            plan.checks.push(Check::Link {
                name: link.name,
                path: dest.join(&link.destination).display().to_string(),
                target: format!("{root}/{}", link.source),
            });
        }
    }
    let mut rows: Vec<_> = plan
        .checks
        .into_iter()
        .map(|c| evaluate(c, &home, base))
        .collect();
    if let Some(file) = plan.brewfile {
        rows.extend(brew_rows(
            &expand(&file, &home, base),
            &plan.standalone_casks.unwrap_or_default(),
            &home,
            base,
        )?);
    }
    if let Some(stage) =
        env::var_os("MAC_STATUS_STAGE").and_then(|path| fs::read_to_string(path).ok())
    {
        let (state, detail) = stage
            .trim()
            .split_once('\t')
            .unwrap_or(("unknown", stage.trim()));
        rows.push(Status {
            section: "Current setup run",
            name: "Guided setup".into(),
            state: match state {
                "running" => "running",
                "done" => "present",
                "failed" => "failed",
                _ => "unknown",
            },
            detail: detail.into(),
        });
    }
    rows.sort_by_key(|row| row.section);
    if gui {
        return native_gui(&rows, &config);
    }
    #[cfg(feature = "gpui")]
    if gpui {
        gui::show(rows);
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else {
        let mut section = None;
        for row in rows {
            if section != Some(row.section) {
                if section.is_some() {
                    println!();
                }
                println!("{}", row.section);
                section = Some(row.section);
            }
            println!("{:<24} {:<21} {}", row.name, row.state, row.detail);
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mac-status: {e}");
            ExitCode::FAILURE
        }
    }
}
