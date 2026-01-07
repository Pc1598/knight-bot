//! System status command for Knight Bot
//!
//! Supports:
//! - CPU usage (with delta calculation)
//! - Memory usage (MiB)
//! - Adreno Freedreno/Mainline GPU usage
//! - Battery percentage
//! - Kernel version
//!
//! Optimized for Mainline Linux on SM8150 (Xiaomi Raphael)

use grammers_client::types::{InputMessage, Message};
use sysinfo::System;
use std::time::Duration;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

pub async fn knightcmd_status(message: Message) -> Result {
    let mut sys = System::new_all();

    // 1. CPU: Average usage over ~1.5s to avoid wakeup spikes on mobile SoCs
    let cpu_usage = read_cpu_avg(&mut sys).await;

    // 2. Memory: sysinfo 0.30 returns Bytes. Divide by 1024^2 for MiB
    let total_mem = sys.total_memory() / 1_048_576;
    let used_mem = sys.used_memory() / 1_048_576;

    // 3. GPU: Using the Freedreno/Devfreq node for SM8150
    let gpu = read_freedreno_gpu().unwrap_or_else(|| "N/A".into());

    // 4. Battery & Kernel
    let battery = read_battery_percentage();
    let kernel = System::kernel_version().unwrap_or_else(|| "unknown".into());

    let text = format!(
        "🖥 <b>System Status</b>\n\
         ─────────────────\n\
         <b>CPU:</b> {:.1}%\n\
         <b>Memory:</b> {} / {} MiB\n\
         <b>GPU (Adreno 640):</b> {}\n\
         <b>Battery:</b> {}\n\
         <b>Kernel:</b> {}",
        cpu_usage,
        used_mem,
        total_mem,
        gpu,
        battery,
        kernel
    );

    message.reply(InputMessage::html(text)).await?;
    Ok(())
}

/// Average CPU usage over multiple samples to avoid instantaneous spikes
async fn read_cpu_avg(sys: &mut System) -> f32 {
    let samples = 5;
    let mut total = 0.0;

    // Warm-up (discard first sample)
    sys.refresh_cpu();
    tokio::time::sleep(Duration::from_millis(300)).await;

    for _ in 0..samples {
        sys.refresh_cpu();
        tokio::time::sleep(Duration::from_millis(300)).await;
        total += sys.global_cpu_info().cpu_usage();
    }

    total / samples as f32
}


/// Read Freedreno GPU stats via devfreq and drm sysfs
fn read_freedreno_gpu() -> Option<String> {
    let base = "/sys/class/devfreq/2c00000.gpu";

    // Load/Busy percentage (Check device node or devfreq utilization)
    let load = read_u64(&format!("{}/device/gpu_busy", base))
        .or_else(|| read_u64(&format!("{}/device/load", base)))
        .unwrap_or(0);

    // Frequencies (Convert Hz to MHz)
    let freq = read_u64(&format!("{}/cur_freq", base)).unwrap_or(0);
    let max_freq = read_u64(&format!("{}/max_freq", base)).unwrap_or(0);

    Some(format!(
        "{}% | {:.0}/{:.0} MHz",
        load,
        freq as f64 / 1_000_000.0,
        max_freq as f64 / 1_000_000.0,
    ))
}

/// Read battery percentage from power_supply sysfs
fn read_battery_percentage() -> String {
    let base = "/sys/class/power_supply";

    if let Ok(entries) = std::fs::read_dir(base) {
        for entry in entries.flatten() {
            let cap = entry.path().join("capacity");
            if let Ok(v) = std::fs::read_to_string(cap) {
                return format!("{}%", v.trim());
            }
        }
    }

    "N/A".into()
}

/// Helper to read u64 from sysfs
fn read_u64(path: &str) -> Option<u64> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
}
