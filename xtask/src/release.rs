//! `cargo xtask release <channel>` (ADR-006): version, changelog cut, build, sign,
//! `latest.json`, upload. One command from the dev PC; CI later with the same code.
//!
//! Everything secret or machine-specific comes from `.env` in the repo root (gitignored,
//! documented in `.env.example`): the Tauri updater's minisign key, the upload target and
//! the public base URL. `cargo xtask release keygen` makes the key pair.
//!
//! Targets (`YAPPA_RELEASE_TARGET`, default `github` — user decision 2026-10-25, see the
//! amendment in ADR-006: the home server is IPv6-only, the repo is public):
//!   github[:owner/repo] GitHub Releases: `v<version>` holds the installers, the rolling
//!                      release `<channel>` holds `latest.json` (`gh` must be logged in)
//!   <dir>              a folder Caddy serves as `/updates/` (the box, a WSL path, a mount)
//!   user@host:/path    the same folder on another machine, over ssh/scp
//!
//! Git stays the user's: the version bump and the changelog cut are left in the working
//! tree to review and commit; the summary says what to tag.

use crate::{step, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHANNELS: &[&str] = &["stable", "beta", "nightly"];
const MANIFEST: &str = "latest.json";
const USAGE: &str = "usage: cargo xtask release <stable|beta|nightly> [--version <x.y.z> | --bump <major|minor|patch>] \
     [--target <dir|user@host:/path|github[:owner/repo]>] [--no-upload] [--skip-build]\n\
     \x20      cargo xtask release keygen [--force]";

pub fn run(root: &Path, args: &[String]) -> Result {
    let flag = |f: &str| args.iter().any(|a| a == f);
    let value = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    match args.first().map(String::as_str) {
        Some("keygen") => keygen(root, flag("--force")),
        Some(channel) if CHANNELS.contains(&channel) => {
            let bump = match (value("--version"), value("--bump")) {
                (Some(_), Some(_)) => return Err("--version or --bump, not both".into()),
                (Some(v), None) => Some(Bump::To(v)),
                (None, Some(part)) => Some(Bump::part(&part)?),
                (None, None) => None,
            };
            let options = Options {
                channel: channel.to_string(),
                bump,
                target: value("--target"),
                upload: !flag("--no-upload"),
                build: !flag("--skip-build"),
            };
            release(root, &options)
        }
        _ => Err(USAGE.into()),
    }
}

struct Options {
    channel: String,
    bump: Option<Bump>,
    target: Option<String>,
    upload: bool,
    build: bool,
}

fn release(root: &Path, options: &Options) -> Result {
    let env = Env::load(root);
    let key = env.signing_key()?;
    let current = current_version(root)?;
    let version = match &options.bump {
        Some(bump) => bump.apply(&current)?,
        None => current.clone(),
    };
    let target = options
        .target
        .clone()
        .or_else(|| env.get("YAPPA_RELEASE_TARGET"))
        .map_or(Target::Github(None), |t| Target::parse(&t));
    println!(
        "release {} → {} ({})",
        version,
        options.channel,
        target.describe()
    );

    if version != current {
        step(&format!("version {current} → {version}"), || {
            set_version(root, &current, &version)
        })?;
    }
    let notes = step("changelog", || {
        let path = root.join("CHANGELOG.md");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("CHANGELOG.md: {e}"))?;
        let cut = cut_changelog(&text, &version, &today())?;
        if cut.text != text {
            std::fs::write(&path, &cut.text).map_err(|e| format!("CHANGELOG.md: {e}"))?;
            crate::docs::generate(root)?;
        }
        Ok(cut.notes)
    })?;

    if options.build {
        // Tauri reads the key from the environment while bundling; `tauri build` then
        // writes a `.sig` next to every installer.
        std::env::set_var("TAURI_SIGNING_PRIVATE_KEY", &key);
        if let Some(password) = env.get("TAURI_SIGNING_PRIVATE_KEY_PASSWORD") {
            std::env::set_var("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", password);
        }
        crate::build(root, false)?;
    }

    let bundle_dir = root.join("target").join("release").join("bundle");
    let artifacts = step("collect installers + signatures", || artifacts(&bundle_dir))?;
    if artifacts.is_empty() {
        return Err(format!(
            "no signed installer under {} — did the build run with createUpdaterArtifacts?",
            bundle_dir.display()
        ));
    }
    for a in &artifacts {
        println!("  {}  {}", a.platform, a.file.display());
    }

    let download_base = match target.download_base(&env, &options.channel, &version) {
        Ok(base) => base,
        // A dry run may lack the host's address; the manifest then says so visibly.
        Err(e) if !options.upload => {
            println!("  ({e} — placeholder URLs in latest.json)");
            format!("https://update-host.invalid/updates/{}", options.channel)
        }
        Err(e) => return Err(e),
    };
    let existing = target.existing_manifest(&options.channel);
    let manifest = build_manifest(
        existing.as_ref(),
        &version,
        &notes,
        &now_rfc3339(),
        artifacts.iter().map(|a| {
            let file = a.file.file_name().unwrap_or_default().to_string_lossy();
            (
                a.platform.clone(),
                PlatformEntry {
                    signature: a.signature.clone(),
                    url: format!("{download_base}/{file}"),
                },
            )
        }),
    );
    let manifest_path = bundle_dir.join(MANIFEST);
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())? + "\n";
    std::fs::write(&manifest_path, &json)
        .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    println!("  {MANIFEST} → {}", manifest_path.display());

    if options.upload {
        let files: Vec<PathBuf> = artifacts
            .iter()
            .flat_map(|a| [a.file.clone(), a.sig_file.clone()])
            .collect();
        step("upload", || {
            target.upload(&options.channel, &version, &notes, &files, &manifest_path)
        })?;
    }

    println!(
        "\n✔ {} {} released{}\n  now: review + commit Cargo.toml, Cargo.lock, apps/desktop/package*.json, \
         CHANGELOG.md and the generated docs; tag it: git tag v{}",
        options.channel,
        version,
        if options.upload { "" } else { " (not uploaded)" },
        version
    );
    Ok(())
}

// ── version ───────────────────────────────────────────────────────────────────────────

enum Bump {
    To(String),
    Major,
    Minor,
    Patch,
}

impl Bump {
    fn part(part: &str) -> Result<Self> {
        match part {
            "major" => Ok(Self::Major),
            "minor" => Ok(Self::Minor),
            "patch" => Ok(Self::Patch),
            other => Err(format!("--bump {other}: expected major, minor or patch")),
        }
    }

    fn apply(&self, current: &str) -> Result<String> {
        let [major, minor, patch] = parse_version(current)?;
        Ok(match self {
            Self::To(v) => {
                parse_version(v)?;
                v.clone()
            }
            Self::Major => format!("{}.0.0", major + 1),
            Self::Minor => format!("{major}.{}.0", minor + 1),
            Self::Patch => format!("{major}.{minor}.{}", patch + 1),
        })
    }
}

fn parse_version(v: &str) -> Result<[u64; 3]> {
    let parts: Vec<&str> = v.trim().split('.').collect();
    if parts.len() != 3 {
        return Err(format!("{v}: not a major.minor.patch version"));
    }
    let mut out = [0u64; 3];
    for (slot, part) in out.iter_mut().zip(parts) {
        *slot = part
            .parse()
            .map_err(|_| format!("{v}: not a major.minor.patch version"))?;
    }
    Ok(out)
}

/// `[workspace.package] version` in the root Cargo.toml — the one source of the version.
fn current_version(root: &Path) -> Result<String> {
    let text =
        std::fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    workspace_version(&text).ok_or_else(|| "Cargo.toml: no [workspace.package] version".into())
}

fn workspace_version(cargo_toml: &str) -> Option<String> {
    let section = cargo_toml.split("[workspace.package]").nth(1)?;
    section
        .lines()
        .take_while(|l| !l.trim_start().starts_with('['))
        .find_map(|l| {
            let (key, value) = l.split_once('=')?;
            (key.trim() == "version").then(|| value.trim().trim_matches('"').to_string())
        })
}

/// The version lives in Cargo.toml and, for npm's sake, in package.json and
/// package-lock.json (its first two entries: the root and `packages[""]`). Cargo.lock
/// follows with `cargo update --workspace`.
fn set_version(root: &Path, from: &str, to: &str) -> Result {
    let edit = |rel: &str, f: &dyn Fn(&str) -> Result<String>| -> Result {
        let path = root.join(rel);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{rel}: {e}"))?;
        std::fs::write(&path, f(&text)?).map_err(|e| format!("{rel}: {e}"))
    };
    edit("Cargo.toml", &|t| bump_cargo_toml(t, from, to))?;
    edit("apps/desktop/package.json", &|t| bump_json(t, from, to, 1))?;
    edit("apps/desktop/package-lock.json", &|t| {
        bump_json(t, from, to, 2)
    })?;
    crate::cargo(root, &["update", "--workspace", "--offline"])
}

fn bump_cargo_toml(text: &str, from: &str, to: &str) -> Result<String> {
    let (head, tail) = text
        .split_once("[workspace.package]")
        .ok_or("Cargo.toml: no [workspace.package]")?;
    let needle = format!("version = \"{from}\"");
    let replaced = tail.replacen(&needle, &format!("version = \"{to}\""), 1);
    if replaced == tail {
        return Err(format!(
            "Cargo.toml: {needle} not found under [workspace.package]"
        ));
    }
    Ok(format!("{head}[workspace.package]{replaced}"))
}

fn bump_json(text: &str, from: &str, to: &str, occurrences: usize) -> Result<String> {
    let needle = format!("\"version\": \"{from}\"");
    let found = text.matches(&needle).count();
    if found < occurrences {
        return Err(format!("expected {occurrences} × {needle}, found {found}"));
    }
    Ok(text.replacen(&needle, &format!("\"version\": \"{to}\""), occurrences))
}

// ── changelog ─────────────────────────────────────────────────────────────────────────

struct Cut {
    text: String,
    /// The released version's entries, markdown, for `latest.json` and the release page.
    notes: String,
}

/// Moves the entries under `## Unreleased` into a new `## <version> (<date>)` section and
/// leaves an empty Unreleased behind. If the version's section exists already (a second
/// channel of the same build), nothing moves and its entries are the notes.
fn cut_changelog(text: &str, version: &str, date: &str) -> Result<Cut> {
    let header = format!("## {version} ");
    if let Some(section) = section_body(text, |h| {
        h.starts_with(&header) || h == format!("## {version}")
    }) {
        return Ok(Cut {
            text: text.to_string(),
            notes: section.trim().to_string(),
        });
    }
    let unreleased = section_body(text, |h| h == "## Unreleased")
        .ok_or("CHANGELOG.md: no `## Unreleased` section")?;
    if unreleased.trim().is_empty() {
        return Err(format!(
            "CHANGELOG.md: nothing under `## Unreleased` and no section for {version}"
        ));
    }
    let notes = unreleased.trim().to_string();
    let replaced = text.replacen(
        &format!("## Unreleased\n{unreleased}"),
        &format!("## Unreleased\n\n## {version} ({date})\n\n{notes}\n\n"),
        1,
    );
    Ok(Cut {
        text: replaced,
        notes,
    })
}

/// The lines between a `## ` heading that matches and the next `## ` heading (or the end).
fn section_body(text: &str, matches: impl Fn(&str) -> bool) -> Option<String> {
    let mut lines = text.lines();
    lines.find(|l| l.starts_with("## ") && matches(l.trim_end()))?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).collect();
    Some(body.join("\n") + "\n")
}

// ── artifacts + manifest ──────────────────────────────────────────────────────────────

struct Artifact {
    /// `windows-x86_64`, `darwin-aarch64`, … — the updater's platform keys.
    platform: String,
    file: PathBuf,
    sig_file: PathBuf,
    signature: String,
}

/// The updater's own bundle per OS (NSIS installer on Windows, the `.app.tar.gz` on
/// macOS) with its `.sig`, as `tauri build` leaves them with `createUpdaterArtifacts`.
/// A product name with a space (the installer was „yAPPA B_…“ until 0.1.0) breaks URLs and
/// GitHub asset names, so the pair is copied to `bundle/upload/` under a name without one.
fn artifacts(bundle_dir: &Path) -> Result<Vec<Artifact>> {
    let (kind, suffix, os) = if cfg!(windows) {
        ("nsis", "-setup.exe", "windows")
    } else if cfg!(target_os = "macos") {
        ("macos", ".app.tar.gz", "darwin")
    } else {
        ("appimage", ".AppImage", "linux")
    };
    let dir = bundle_dir.join(kind);
    let mut out = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for file in entries.flatten().map(|e| e.path()) {
        let name = file
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if !name.ends_with(suffix) {
            continue;
        }
        let sig_file = PathBuf::from(format!("{}.sig", file.display()));
        let signature = std::fs::read_to_string(&sig_file).map_err(|e| {
            format!(
                "{}: {e} — was TAURI_SIGNING_PRIVATE_KEY set?",
                sig_file.display()
            )
        })?;
        let upload = bundle_dir.join("upload");
        std::fs::create_dir_all(&upload).map_err(|e| format!("{}: {e}", upload.display()))?;
        let clean = upload.join(upload_name(&name));
        let clean_sig = PathBuf::from(format!("{}.sig", clean.display()));
        std::fs::copy(&file, &clean).map_err(|e| format!("{}: {e}", clean.display()))?;
        std::fs::copy(&sig_file, &clean_sig)
            .map_err(|e| format!("{}: {e}", clean_sig.display()))?;
        out.push(Artifact {
            platform: format!("{os}-{}", std::env::consts::ARCH),
            file: clean,
            sig_file: clean_sig,
            signature: signature.trim().to_string(),
        });
    }
    Ok(out)
}

/// `yAPPA B_0.2.0_x64-setup.exe` → `yAPPA-B_0.2.0_x64-setup.exe`: no spaces, nothing a URL
/// or GitHub would rewrite.
fn upload_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_whitespace() { '-' } else { c })
        .collect()
}

/// `latest.json` as `tauri-plugin-updater` reads it (static manifest).
#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Manifest {
    version: String,
    notes: String,
    pub_date: String,
    platforms: BTreeMap<String, PlatformEntry>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct PlatformEntry {
    signature: String,
    url: String,
}

/// One manifest per channel, with every platform in it. A release runs on one OS at a
/// time, so the other platforms' entries are kept from the manifest on the host — but only
/// if it is the same version: an older macOS build must not be offered as the new one.
fn build_manifest(
    existing: Option<&Manifest>,
    version: &str,
    notes: &str,
    pub_date: &str,
    fresh: impl IntoIterator<Item = (String, PlatformEntry)>,
) -> Manifest {
    let mut platforms = match existing {
        Some(m) if m.version == version => m.platforms.clone(),
        _ => BTreeMap::new(),
    };
    platforms.extend(fresh);
    Manifest {
        version: version.to_string(),
        notes: notes.to_string(),
        pub_date: pub_date.to_string(),
        platforms,
    }
}

// ── targets ───────────────────────────────────────────────────────────────────────────

enum Target {
    Dir(PathBuf),
    /// `user@host` and the remote folder.
    Scp {
        host: String,
        path: String,
    },
    /// `owner/repo`; `None` = the repo of the current checkout (`gh` decides).
    Github(Option<String>),
}

impl Target {
    fn parse(text: &str) -> Self {
        let text = text.trim();
        if text == "github" {
            return Self::Github(None);
        }
        if let Some(repo) = text.strip_prefix("github:") {
            return Self::Github(Some(repo.to_string()));
        }
        // `user@host:/path` — the colon after the host, not a drive letter.
        if let Some((host, path)) = text.split_once(':') {
            if host.contains('@') && !path.starts_with('\\') {
                return Self::Scp {
                    host: host.to_string(),
                    path: path.to_string(),
                };
            }
        }
        Self::Dir(PathBuf::from(text))
    }

    fn describe(&self) -> String {
        match self {
            Self::Dir(dir) => format!("folder {}", dir.display()),
            Self::Scp { host, path } => format!("scp {host}:{path}"),
            Self::Github(None) => "GitHub Releases".into(),
            Self::Github(Some(repo)) => format!("GitHub Releases {repo}"),
        }
    }

    fn gh_repo_args(&self) -> Vec<String> {
        match self {
            Self::Github(Some(repo)) => vec!["--repo".into(), repo.clone()],
            _ => Vec::new(),
        }
    }

    /// Where the installers of this release can be downloaded from, without the file name.
    fn download_base(&self, env: &Env, channel: &str, version: &str) -> Result<String> {
        match self {
            Self::Dir(_) | Self::Scp { .. } => {
                let base = env.get("YAPPA_UPDATE_BASE_URL").ok_or(
                    "YAPPA_UPDATE_BASE_URL missing in .env (what Caddy serves as /updates, \
                     e.g. https://<YAPPA_DOMAIN>/updates)",
                )?;
                Ok(format!("{}/{channel}", base.trim_end_matches('/')))
            }
            Self::Github(_) => {
                let repo = match self {
                    Self::Github(Some(repo)) => repo.clone(),
                    _ => gh_output(&[
                        "repo",
                        "view",
                        "--json",
                        "nameWithOwner",
                        "-q",
                        ".nameWithOwner",
                    ])?,
                };
                Ok(format!(
                    "https://github.com/{}/releases/download/v{version}",
                    repo.trim()
                ))
            }
        }
    }

    /// The channel's manifest as it is on the host, if any (errors are "none": a fresh
    /// channel has none, and the merge only keeps entries of the same version anyway).
    fn existing_manifest(&self, channel: &str) -> Option<Manifest> {
        let text = match self {
            Self::Dir(dir) => std::fs::read_to_string(dir.join(channel).join(MANIFEST)).ok()?,
            Self::Scp { host, path } => {
                let out = Command::new("ssh")
                    .args([host, &format!("cat {path}/{channel}/{MANIFEST}")])
                    .output()
                    .ok()?;
                out.status
                    .success()
                    .then(|| String::from_utf8_lossy(&out.stdout).into_owned())?
            }
            Self::Github(_) => {
                let mut args = vec![
                    "release".to_string(),
                    "view".into(),
                    channel.into(),
                    "--json".into(),
                    "assets".into(),
                ];
                args.extend(self.gh_repo_args());
                let out = Command::new("gh").args(&args).output().ok()?;
                if !out.status.success() {
                    return None;
                }
                let assets: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
                let url = assets["assets"]
                    .as_array()?
                    .iter()
                    .find(|a| a["name"] == MANIFEST)?["url"]
                    .as_str()?
                    .to_string();
                let out = Command::new("curl").args(["-sSL", &url]).output().ok()?;
                out.status
                    .success()
                    .then(|| String::from_utf8_lossy(&out.stdout).into_owned())?
            }
        };
        serde_json::from_str(&text).ok()
    }

    fn upload(
        &self,
        channel: &str,
        version: &str,
        notes: &str,
        files: &[PathBuf],
        manifest: &Path,
    ) -> Result {
        match self {
            Self::Dir(dir) => {
                let dest = dir.join(channel);
                std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
                // Installers first, the manifest last: a client that reads in between
                // still sees a consistent pair.
                for file in files.iter().chain(std::iter::once(&manifest.to_path_buf())) {
                    let to = dest.join(file.file_name().unwrap_or_default());
                    std::fs::copy(file, &to).map_err(|e| format!("{}: {e}", to.display()))?;
                    println!("  → {}", to.display());
                }
                Ok(())
            }
            Self::Scp { host, path } => {
                let dest = format!("{path}/{channel}");
                crate::run(Command::new("ssh").args([host, &format!("mkdir -p {dest}")]))?;
                let mut scp = Command::new("scp");
                scp.args(files).arg(format!("{host}:{dest}/"));
                crate::run(&mut scp)?;
                crate::run(
                    Command::new("scp")
                        .arg(manifest)
                        .arg(format!("{host}:{dest}/")),
                )
            }
            Self::Github(_) => {
                let repo = self.gh_repo_args();
                let tag = format!("v{version}");
                let notes_file = manifest.with_file_name("release-notes.md");
                std::fs::write(&notes_file, notes).map_err(|e| e.to_string())?;
                let prerelease = channel != "stable";
                // The versioned release: created once, installers added per OS.
                if !gh_release_exists(&tag, &repo) {
                    let mut args = vec![
                        "release".to_string(),
                        "create".into(),
                        tag.clone(),
                        "--title".into(),
                        format!("yAPPA {version}"),
                        "--notes-file".into(),
                        notes_file.display().to_string(),
                    ];
                    if prerelease {
                        args.push("--prerelease".into());
                    }
                    args.extend(repo.clone());
                    crate::run(Command::new("gh").args(&args))?;
                }
                let mut args = vec!["release".to_string(), "upload".into(), tag.clone()];
                args.extend(files.iter().map(|f| f.display().to_string()));
                args.push("--clobber".into());
                args.extend(repo.clone());
                crate::run(Command::new("gh").args(&args))?;
                // The rolling channel release: only the manifest, never "latest".
                if !gh_release_exists(channel, &repo) {
                    let mut args = vec![
                        "release".to_string(),
                        "create".into(),
                        channel.to_string(),
                        "--title".into(),
                        format!("Update-Kanal {channel}"),
                        "--notes".into(),
                        format!("`latest.json` des Kanals {channel}; die Installer liegen bei den Versionen."),
                        "--latest=false".into(),
                        "--prerelease".into(),
                    ];
                    args.extend(repo.clone());
                    crate::run(Command::new("gh").args(&args))?;
                }
                let mut args = vec![
                    "release".to_string(),
                    "upload".into(),
                    channel.to_string(),
                    manifest.display().to_string(),
                    "--clobber".into(),
                ];
                args.extend(repo);
                crate::run(Command::new("gh").args(&args))
            }
        }
    }
}

fn gh_release_exists(tag: &str, repo: &[String]) -> bool {
    Command::new("gh")
        .args(["release", "view", tag])
        .args(repo)
        .output()
        .is_ok_and(|o| o.status.success())
}

fn gh_output(args: &[&str]) -> Result<String> {
    let out = Command::new("gh")
        .args(args)
        .output()
        .map_err(|e| format!("gh: {e} — install the GitHub CLI and `gh auth login`"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(format!(
            "gh {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

// ── .env + key ────────────────────────────────────────────────────────────────────────

/// `KEY=value` lines from `.env` in the repo root; the process environment wins.
struct Env(BTreeMap<String, String>);

impl Env {
    fn load(root: &Path) -> Self {
        let text = std::fs::read_to_string(root.join(".env")).unwrap_or_default();
        Self(parse_env(&text))
    }

    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| self.0.get(key).cloned())
    }

    /// The minisign private key: the key itself, or read from the file the path names.
    fn signing_key(&self) -> Result<String> {
        if let Some(key) = self.get("TAURI_SIGNING_PRIVATE_KEY") {
            return Ok(key);
        }
        if let Some(path) = self.get("TAURI_SIGNING_PRIVATE_KEY_PATH") {
            return std::fs::read_to_string(&path)
                .map(|k| k.trim().to_string())
                .map_err(|e| format!("TAURI_SIGNING_PRIVATE_KEY_PATH {path}: {e}"));
        }
        Err(
            "no signing key: TAURI_SIGNING_PRIVATE_KEY_PATH (or _KEY) in .env — \
             `cargo xtask release keygen` makes one"
                .into(),
        )
    }
}

fn parse_env(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let (key, value) = l.split_once('=')?;
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            (!value.is_empty()).then(|| (key.trim().to_string(), value.to_string()))
        })
        .collect()
}

/// A minisign key pair for the updater, outside the repo (`~/.yappa/updater.key`), and the
/// lines to put into `.env` and `tauri.conf.json`.
fn keygen(root: &Path, force: bool) -> Result {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .ok_or("no home directory")?;
    let key = Path::new(&home).join(".yappa").join("updater.key");
    if key.exists() && !force {
        return Err(format!(
            "{} exists — the apps out there trust its public key; --force to replace it anyway",
            key.display()
        ));
    }
    std::fs::create_dir_all(key.parent().expect("parent")).map_err(|e| e.to_string())?;
    let mut args = vec!["tauri", "signer", "generate", "-w"];
    let key_text = key.display().to_string();
    args.push(&key_text);
    if force {
        args.push("--force");
    }
    crate::run(
        Command::new(crate::tools::exe("npx"))
            .current_dir(root.join(crate::DESKTOP))
            .args(&args),
    )?;
    let public = std::fs::read_to_string(format!("{key_text}.pub")).map_err(|e| e.to_string())?;
    println!(
        "\n.env (repo root, gitignored):\n  TAURI_SIGNING_PRIVATE_KEY_PATH={key_text}\n  \
         TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<the password, if one was set>\n\n\
         apps/desktop/src-tauri/tauri.conf.json → plugins.updater.pubkey:\n  {}\n\n\
         Keep the private key out of the repo; back it up — without it no app out there \
         accepts an update.",
        public.trim()
    );
    Ok(())
}

fn today() -> String {
    now_rfc3339()[..10].to_string()
}

/// UTC, `YYYY-MM-DDTHH:MM:SSZ`, without a date crate (civil-from-days, Howard Hinnant).
fn now_rfc3339() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86_400) as i64;
    let (h, m, s) = ((secs / 3600) % 24, (secs / 60) % 60, secs % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(mo <= 2);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    const CARGO: &str = "[workspace]\nmembers = [\"x\"]\n\n[workspace.package]\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace.dependencies]\nfoo = { version = \"0.1.0\" }\n";

    #[test]
    fn version_is_read_and_bumped_under_workspace_package_only() {
        assert_eq!(workspace_version(CARGO).as_deref(), Some("0.1.0"));
        let bumped = bump_cargo_toml(CARGO, "0.1.0", "0.2.0").unwrap();
        assert!(bumped.contains("version = \"0.2.0\"\nedition"));
        assert!(
            bumped.contains("foo = { version = \"0.1.0\" }"),
            "dependencies untouched"
        );
        assert!(bump_cargo_toml(CARGO, "9.9.9", "1.0.0").is_err());
    }

    #[test]
    fn bump_parts() {
        assert_eq!(Bump::Patch.apply("0.1.9").unwrap(), "0.1.10");
        assert_eq!(Bump::Minor.apply("0.1.9").unwrap(), "0.2.0");
        assert_eq!(Bump::Major.apply("0.1.9").unwrap(), "1.0.0");
        assert_eq!(Bump::To("1.2.3".into()).apply("0.1.9").unwrap(), "1.2.3");
        assert!(Bump::To("1.2".into()).apply("0.1.9").is_err());
        assert!(Bump::part("huge").is_err());
    }

    #[test]
    fn package_lock_changes_only_its_first_two_versions() {
        let lock = "{\n  \"name\": \"d\",\n  \"version\": \"0.1.0\",\n  \"packages\": {\n    \"\": {\n      \"version\": \"0.1.0\"\n    },\n    \"node_modules/dep\": {\n      \"version\": \"0.1.0\"\n    }\n  }\n}\n";
        let bumped = bump_json(lock, "0.1.0", "0.2.0", 2).unwrap();
        assert_eq!(bumped.matches("\"version\": \"0.2.0\"").count(), 2);
        assert!(bumped.contains("node_modules/dep\": {\n      \"version\": \"0.1.0\""));
        assert!(bump_json("{}", "0.1.0", "0.2.0", 1).is_err());
    }

    const CHANGELOG: &str = "# Changelog\n\nOne line per task.\n\n## Unreleased\n\n- Second thing (B).\n- First thing (A).\n\n## 0.1.0 (2026-10-01)\n\n- Older.\n";

    #[test]
    fn changelog_cut_moves_unreleased_into_a_dated_section() {
        let cut = cut_changelog(CHANGELOG, "0.2.0", "2026-10-25").unwrap();
        assert_eq!(cut.notes, "- Second thing (B).\n- First thing (A).");
        assert_eq!(
            cut.text,
            "# Changelog\n\nOne line per task.\n\n## Unreleased\n\n## 0.2.0 (2026-10-25)\n\n- Second thing (B).\n- First thing (A).\n\n## 0.1.0 (2026-10-01)\n\n- Older.\n"
        );
        // Cutting again (second channel, same version): nothing moves, same notes.
        let again = cut_changelog(&cut.text, "0.2.0", "2026-10-26").unwrap();
        assert_eq!(again.text, cut.text);
        assert_eq!(again.notes, cut.notes);
        // Nothing to release.
        assert!(cut_changelog(&cut.text, "0.3.0", "2026-10-26").is_err());
    }

    #[test]
    fn manifest_keeps_other_platforms_of_the_same_version_only() {
        let entry = |url: &str| PlatformEntry {
            signature: "sig".into(),
            url: url.into(),
        };
        let old = Manifest {
            version: "0.2.0".into(),
            notes: "old".into(),
            pub_date: "2026-10-24T10:00:00Z".into(),
            platforms: [
                (
                    "darwin-aarch64".to_string(),
                    entry("https://h/stable/yAPPA.app.tar.gz"),
                ),
                (
                    "windows-x86_64".to_string(),
                    entry("https://h/stable/old-setup.exe"),
                ),
            ]
            .into(),
        };
        let fresh = [(
            "windows-x86_64".to_string(),
            entry("https://h/stable/yAPPA_0.2.0_x64-setup.exe"),
        )];
        let merged = build_manifest(
            Some(&old),
            "0.2.0",
            "- Notes",
            "2026-10-25T12:00:00Z",
            fresh.clone(),
        );
        insta::assert_snapshot!(serde_json::to_string_pretty(&merged).unwrap());
        let newer = build_manifest(
            Some(&old),
            "0.3.0",
            "- Notes",
            "2026-10-25T12:00:00Z",
            fresh,
        );
        assert_eq!(
            newer.platforms.len(),
            1,
            "an older macOS build is not offered as 0.3.0"
        );
    }

    #[test]
    fn upload_names_have_no_spaces() {
        assert_eq!(
            upload_name("yAPPA B_0.2.0_x64-setup.exe"),
            "yAPPA-B_0.2.0_x64-setup.exe"
        );
        assert_eq!(upload_name("yAPPA.app.tar.gz"), "yAPPA.app.tar.gz");
    }

    #[test]
    fn targets_are_told_apart() {
        assert!(matches!(Target::parse("github"), Target::Github(None)));
        assert!(
            matches!(Target::parse("github:crisin/YAPPA"), Target::Github(Some(r)) if r == "crisin/YAPPA")
        );
        assert!(
            matches!(Target::parse("chris@box:/srv/yappa/updates"), Target::Scp { host, path } if host == "chris@box" && path == "/srv/yappa/updates")
        );
        assert!(matches!(Target::parse(r"C:\updates"), Target::Dir(_)));
        assert!(matches!(
            Target::parse(r"\\wsl.localhost\Ubuntu\home\c\updates"),
            Target::Dir(_)
        ));
        assert!(matches!(Target::parse("/srv/updates"), Target::Dir(_)));
    }

    #[test]
    fn env_file_is_parsed_loosely() {
        let env = parse_env("# comment\nA=1\nB = \"two words\"\nC='x'\nEMPTY=\n\nbroken line\n");
        assert_eq!(env.get("A").map(String::as_str), Some("1"));
        assert_eq!(env.get("B").map(String::as_str), Some("two words"));
        assert_eq!(env.get("C").map(String::as_str), Some("x"));
        assert!(!env.contains_key("EMPTY"));
        assert_eq!(env.len(), 3);
    }

    #[test]
    fn rfc3339_now_has_the_shape() {
        let now = now_rfc3339();
        assert_eq!(now.len(), 20);
        assert!(now.starts_with("20") && now.ends_with('Z'));
        assert_eq!(today().len(), 10);
    }
}
