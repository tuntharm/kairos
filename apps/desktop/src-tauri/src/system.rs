use std::path::Path;
use std::process::Command;

use serde::Serialize;

use kairos_core::HardwareProfile;

pub const OLLAMA_MACOS_DOWNLOAD_URL: &str = "https://ollama.com/download/mac";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineCapability {
    pub architecture: String,
    pub detected_hardware_profile: HardwareProfile,
    pub unified_memory_bytes: Option<u64>,
    pub unified_memory_gib: Option<u16>,
    pub nvidia_vram_gib: Option<u16>,
    pub free_disk_bytes: Option<u64>,
    /// The only physical capacity Auto may use for local-model fit. It is
    /// unified memory on Apple Silicon, VRAM on NVIDIA, or system RAM only
    /// when neither accelerator is present.
    pub primary_fit_capacity_gib: Option<u16>,
    pub detected_memory_budget_gib: Option<u16>,
}

pub fn machine_capability() -> MachineCapability {
    let unified_memory_bytes = detect_memory_bytes();
    let unified_memory_gib = unified_memory_bytes.map(bytes_to_gib);
    let nvidia_vram_gib = detect_nvidia_vram_gib();
    let detected_hardware_profile = if is_apple_silicon() {
        HardwareProfile::AppleUnified
    } else if nvidia_vram_gib.is_some() {
        HardwareProfile::NvidiaVram
    } else {
        HardwareProfile::CpuOnly
    };
    let primary_fit_capacity_gib = match detected_hardware_profile {
        HardwareProfile::AppleUnified => unified_memory_gib,
        HardwareProfile::NvidiaVram => nvidia_vram_gib,
        HardwareProfile::CpuOnly | HardwareProfile::Auto => unified_memory_gib,
    };
    MachineCapability {
        architecture: std::env::consts::ARCH.to_owned(),
        detected_hardware_profile,
        unified_memory_bytes,
        unified_memory_gib,
        nvidia_vram_gib,
        free_disk_bytes: detect_free_disk_bytes(),
        primary_fit_capacity_gib,
        detected_memory_budget_gib: primary_fit_capacity_gib,
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

pub fn reveal_in_finder(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|error| format!("could not reveal item in Finder: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Finder reveal is currently available on macOS only.".to_owned())
    }
}

pub fn open_in_finder(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("/usr/bin/open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("could not open folder in Finder: {error}"))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Finder opening is currently available on macOS only.".to_owned())
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

fn is_apple_silicon() -> bool {
    #[cfg(target_os = "macos")]
    {
        if matches!(std::env::consts::ARCH, "aarch64" | "arm64") {
            return true;
        }
        Command::new("/usr/sbin/sysctl")
            .args(["-in", "hw.optional.arm64"])
            .output()
            .ok()
            .is_some_and(|output| {
                output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == "1"
            })
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// The largest single NVIDIA GPU is deliberate: v1 does not assume model
/// sharding across cards. Extra system RAM is never substituted for VRAM.
fn detect_nvidia_vram_gib() -> Option<u16> {
    let program = known_program_path("nvidia-smi").unwrap_or_else(|| "nvidia-smi".into());
    let output = Command::new(program)
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_nvidia_vram_mib(&String::from_utf8_lossy(&output.stdout))
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

fn parse_nvidia_vram_mib(value: &str) -> Option<u16> {
    value
        .lines()
        .filter_map(|line| line.split_whitespace().next()?.parse::<u64>().ok())
        .max()
        .map(|mib| ((mib + 512) / 1024).min(u16::MAX as u64) as u16)
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

    #[test]
    fn uses_the_largest_single_nvidia_vram_value_without_combining_cards() {
        assert_eq!(parse_nvidia_vram_mib("12288\n24576\n"), Some(24));
        assert_eq!(parse_nvidia_vram_mib("not a gpu"), None);
    }
}
