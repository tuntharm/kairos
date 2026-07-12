use std::path::Path;
use std::process::Command;

use serde::Serialize;

pub const OLLAMA_MACOS_DOWNLOAD_URL: &str = "https://ollama.com/download/mac";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineCapability {
    pub architecture: String,
    pub unified_memory_bytes: Option<u64>,
    pub unified_memory_gib: Option<u16>,
    pub free_disk_bytes: Option<u64>,
    pub detected_memory_budget_gib: Option<u16>,
}

pub fn machine_capability() -> MachineCapability {
    let unified_memory_bytes = detect_memory_bytes();
    let unified_memory_gib = unified_memory_bytes.map(bytes_to_gib);
    MachineCapability {
        architecture: std::env::consts::ARCH.to_owned(),
        unified_memory_bytes,
        unified_memory_gib,
        free_disk_bytes: detect_free_disk_bytes(),
        detected_memory_budget_gib: unified_memory_gib,
    }
}

pub fn ollama_is_installed() -> bool {
    known_program_path("ollama").is_some()
        || Path::new("/Applications/Ollama.app").exists()
        || Path::new("/Applications/Ollama.app/Contents/Resources/ollama").exists()
}

pub fn cli_is_available(program: &str) -> bool {
    known_program_path(program).is_some()
}

pub fn open_ollama_install_page() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open")
            .arg(OLLAMA_MACOS_DOWNLOAD_URL)
            .spawn()
            .map_err(|error| format!("could not open Ollama's download page: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("Kairos local setup currently supports macOS only.".to_owned())
    }
}

pub fn known_program_path(program: &str) -> Option<std::path::PathBuf> {
    let mut candidates = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join(program))
        .collect::<Vec<_>>();
    if let Some(home) = std::env::var_os("HOME") {
        let home = std::path::PathBuf::from(home);
        candidates.extend([
            home.join(".local/bin").join(program),
            home.join(".cargo/bin").join(program),
        ]);
    }
    candidates.extend([
        std::path::PathBuf::from("/opt/homebrew/bin").join(program),
        std::path::PathBuf::from("/usr/local/bin").join(program),
        std::path::PathBuf::from("/usr/bin").join(program),
        std::path::PathBuf::from("/Applications/Ollama.app/Contents/Resources").join(program),
    ]);
    candidates.into_iter().find(|path| path.is_file())
}

fn detect_memory_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/usr/sbin/sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        parse_memory_bytes(&String::from_utf8_lossy(&output.stdout))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn detect_free_disk_bytes() -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        let output = Command::new("/bin/df").args(["-k", "/"]).output().ok()?;
        if !output.status.success() {
            return None;
        }
        parse_df_available_kib(&String::from_utf8_lossy(&output.stdout))
            .map(|available| available.saturating_mul(1024))
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

fn bytes_to_gib(bytes: u64) -> u16 {
    const GIB: u64 = 1024 * 1024 * 1024;
    ((bytes.saturating_add(GIB / 2) / GIB).min(u16::MAX as u64)) as u16
}

fn parse_memory_bytes(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

fn parse_df_available_kib(value: &str) -> Option<u64> {
    value
        .lines()
        .skip(1)
        .last()?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_macos_capabilities_without_assuming_the_machine() {
        assert_eq!(parse_memory_bytes("51539607552\n"), Some(51_539_607_552));
        assert_eq!(bytes_to_gib(51_539_607_552), 48);
        assert_eq!(
            parse_df_available_kib(
                "Filesystem 1024-blocks Used Available Capacity iused ifree %iused Mounted on\n/dev/disk3s5 971350180 372393316 557299096 41% 3213070 5572990960 0% /System/Volumes/Data\n"
            ),
            Some(557_299_096)
        );
    }
}
