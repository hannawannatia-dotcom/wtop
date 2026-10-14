use std::io::{self, Write};
use std::thread;
use std::time::Duration;
use sysinfo::{Disks, System, Components, Networks};
use std::path::Path;
use std::fs;

fn main() {
    let mut refesh_time = 0;
    let mut sys = System::new_all();
    let mut components = Components::new_with_refreshed_list();
    let mut disk: Disks = Disks::new_with_refreshed_list();
    let mut networks: Networks = Networks::new_with_refreshed_list();

    print!("\x1B[2J\x1B[H\x1B[?25l");
    io::stdout().flush().unwrap();
    
    loop {
        sys.refresh_all();
        disk.refresh(true);
        components.refresh(true);
        networks.refresh(true);
        
        //CPU
        let cpu_used = sys.global_cpu_usage();
        let cpu_temp = components.iter().find(|c| c.label().contains("k10temp")).and_then(|c| c.temperature()).unwrap_or(0.0);
        let cpu_freq = if !sys.cpus().is_empty() {
            (sys.cpus().iter().map(|c| c.frequency()).sum::<u64>() / sys.cpus().len() as u64) as f64 / 1000.0
        } else {
            0.0
        };

        let cpus = sys.cpus();
        let half_core = cpus.len() / 2;
        let mut cpu_cores = String::new();
        for i in 0..half_core {
            let left_usage = cpus[i].cpu_usage();
            let left_str = format!("C{:02} {} {:>3.0}%",i , make_bar(left_usage, 2), left_usage);

            let right_str = if i + half_core < cpus.len() {
                let right_usage = cpus[i + half_core].cpu_usage();
                format!("  C{:02}: {} {:>3.0}%", i + half_core, make_bar(right_usage, 2), right_usage)
            } else {
                String::new()
            };

            cpu_cores.push_str(&format!("{left_str}{right_str}\x1B[K\n"));
        }

        //GPU
        let mut gpu_used = 0;
        if let Ok(content) = fs::read_to_string("/sys/class/drm/card1/device/gpu_busy_percent") {
            gpu_used = content.trim().parse::<u32>().unwrap_or(0);
        }
        let gpu_temp = components.iter().find(|c| c.label().contains("amdgpu")).and_then(|c| c.temperature()).unwrap_or(0.0);
        let gpu_freq = if let Ok(content) = fs::read_to_string("/sys/class/drm/card1/device/pp_dpm_sclk") {
            content.lines().find(|line| line.contains('*')).and_then(|line| line.split_whitespace().nth(1)).unwrap_or("N/A").to_string()
        } else {
            "N/A".to_string()
        };

        //NET
        let mut rx_speed = 0;
        let mut tx_speed = 0;
        for (_name, data) in &networks{
            rx_speed += data.received();
            tx_speed += data.transmitted();
        }

        //RAM
        let ram_used = sys.used_memory() as f64 / 1024.0 / 1024.0 / 1024.0;
        let ram_total = sys.total_memory() as f64 / 1024.0 / 1024.0 / 1024.0; // <-- เพิ่มบรรทัดนี้

        //Disk
        let (disk_used, disk_free, disk_total) = if let Some(root) = disk.list().iter().find(|d| d.mount_point() == Path::new("/")) {
            let total = root.total_space() as f64 / 1024.0 / 1024.0 / 1024.0;
            let free = root.available_space() as f64 / 1024.0 / 1024.0 / 1024.0;
            let used = total - free;
            (used, free, total)
        } else {
            (0.0, 0.0, 0.0)
        };

        if refesh_time == 20{
            print!("\x1B[2J\x1B[H\x1B[?25l");
            refesh_time = 0;
        }
        

        print!(
            "\x1B[H\
            OS: {} \nHost: {}\n\
            \n\
            Power: {}\n\
            CPU used  : {:.2} %\n> {:.2} °C -> {} GHz\n{}\n{}\n\
            GPU used  : {} %\n{:.2} °C -> {}\n{}\n\
            \n\
            Network: ↓ {:.2} KB/s \n         ↑ {:.2} KB/s\n\
            \n\
            RAM used  : {:.2} GB\n{}\n\
            Disk total: {:.2} GB\n\
            Disk used : {:.2} GB\n\
            Disk free : {:.2} GB\x1B[J",
            System::name().unwrap_or_default(),
            System::host_name().unwrap_or_default(),
            get_system_power(),
            cpu_used, cpu_temp, cpu_freq,make_bar(cpu_used, 25), cpu_cores,
            gpu_used, gpu_temp, gpu_freq, make_bar(gpu_used as f32, 25),
            rx_speed as f64 / 1024.0, tx_speed as f64 / 1024.0,
            ram_used,make_bar((ram_used / ram_total * 100.0) as f32, 25),
            disk_total,
            disk_used,
            disk_free
        );       

        io::stdout().flush().unwrap();

        thread::sleep(Duration::from_millis(500));
        refesh_time += 1;
    }

    fn make_bar(percent: f32, width: usize) -> String {
        let filled = ((percent.clamp(0.0, 100.0) / 100.0) * width as f32).round() as usize;
        let empty = width.saturating_sub(filled);
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    }

    fn get_system_power() -> String {
    if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.starts_with("BAT") {
                if let Ok(p) = fs::read_to_string(path.join("power_now")) {
                    if let Ok(uw) = p.trim().parse::<f64>() {
                        if uw > 100_000.0 {
                            return format!("{:.2} W", uw / 1_000_000.0);
                        }
                    }
                }
                if let (Ok(v), Ok(c)) = (
                    fs::read_to_string(path.join("voltage_now")),
                    fs::read_to_string(path.join("current_now")),
                ) {
                    if let (Ok(v_num), Ok(c_num)) = (v.trim().parse::<f64>(), c.trim().parse::<f64>()) {
                        let watts = (v_num * c_num) / 1_000_000_000_000.0;
                        if watts > 0.1 {
                            return format!("{:.2} W", watts);
                        }
                    }
                }
            }
        }
    }
    let mut total_hw_microwatts = 0.0;
    let mut found_sensor = false;
    if let Ok(entries) = fs::read_dir("/sys/class/hwmon") {
        for entry in entries.flatten() {
            let p = entry.path();
            for file in &["power1_input", "power1_average"] {
                if let Ok(content) = fs::read_to_string(p.join(file)) {
                    if let Ok(val) = content.trim().parse::<f64>() {
                        if val > 0.0 {
                            total_hw_microwatts += val;
                            found_sensor = true;
                            break;
                        }
                    }
                }
            }
        }
    }
    if found_sensor {
        return format!("{:.2} W (HW)", total_hw_microwatts / 1_000_000.0);
    }
    "N/A".to_string()
}
}
