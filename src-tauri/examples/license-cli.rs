// 本地 license dev/QA CLI，不走 GUI 直接调 LicenseManager。
//
// 用法:
//   DIMKEY_API_BASE=http://localhost:8080/api/v1 \
//   cargo run --example license-cli -- activate <license_key> <email>
//   cargo run --example license-cli -- status
//   cargo run --example license-cli -- deactivate
//   cargo run --example license-cli -- read-cert

use dimkey_lib::license::{
    api_client,
    fingerprint::compute_fingerprint,
    state::{LicenseManager, LicenseState},
    storage::build_default_stores,
};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;

fn shared_config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("com.dimkey"))
        .unwrap_or_else(|| PathBuf::from("."))
}

fn print_state(state: &LicenseState) {
    println!("=== LicenseState ===");
    match state {
        LicenseState::Activated {
            email,
            plan,
            max_devices,
            active_devices,
            device_id,
            license_id,
            fingerprint_mismatch,
        } => {
            println!("status: Activated");
            println!("  email:       {}", email);
            println!("  plan:        {}", plan);
            println!("  active/max:  {}/{}", active_devices, max_devices);
            println!("  device_id:   {}", device_id);
            println!("  license_id:  {}", license_id);
            println!("  fp_mismatch: {}", fingerprint_mismatch);
        }
        LicenseState::Trial { days_remaining } => {
            println!("status: Trial ({} 天剩余)", days_remaining);
        }
        LicenseState::TrialExpired => println!("status: TrialExpired"),
        LicenseState::GraceMode {
            email,
            days_until_block,
        } => {
            println!(
                "status: GraceMode (email={} days_until_block={})",
                email, days_until_block
            );
        }
        LicenseState::Revoked { reason } => println!("status: Revoked ({})", reason),
        LicenseState::Unknown => println!("status: Unknown"),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("用法:");
        eprintln!("  cargo run --example license-cli -- activate <license_key> <email>");
        eprintln!("  cargo run --example license-cli -- status");
        eprintln!("  cargo run --example license-cli -- heartbeat");
        eprintln!("  cargo run --example license-cli -- deactivate");
        eprintln!("  cargo run --example license-cli -- read-cert");
        std::process::exit(1);
    }

    let config_dir = shared_config_dir();
    std::fs::create_dir_all(&config_dir).ok();
    let machine_fp = compute_fingerprint();
    println!("config_dir:  {}", config_dir.display());
    println!("machine_fp:  {}", machine_fp);
    println!(
        "API_BASE:    {}",
        env::var("DIMKEY_API_BASE")
            .unwrap_or_else(|_| "https://dimkey.app/api/v1 (default)".into())
    );
    println!();

    let trial_stores = build_default_stores(config_dir.clone());
    let manager = Arc::new(LicenseManager::new(trial_stores, config_dir, machine_fp));
    let initial = manager.boot();
    println!("boot 后状态:");
    print_state(&initial);
    println!();

    match args[1].as_str() {
        "activate" => {
            if args.len() < 4 {
                eprintln!("activate 需要 <license_key> <email>");
                std::process::exit(1);
            }
            let license_key = &args[2];
            let email = &args[3];
            println!(
                "调用 try_activate(license_key={}, email={})...",
                license_key, email
            );
            let hostname =
                sysinfo::System::host_name().unwrap_or_else(|| "Unknown".to_string());
            let machine_label = format!("{} (cli-dev)", hostname);
            let os = if cfg!(target_os = "macos") {
                "macos"
            } else {
                "windows"
            };
            let app_version = env!("CARGO_PKG_VERSION");
            println!("machine_label: {}", machine_label);
            match manager
                .try_activate(license_key, email, &machine_label, os, "zh", app_version)
                .await
            {
                Ok(data) => {
                    println!("✅ 激活成功");
                    println!(
                        "  device_summary: active={}/{} current_device_id={}",
                        data.device_summary.active_count,
                        data.device_summary.max_devices,
                        data.device_summary.current_device_id
                    );
                    print_state(&manager.current());
                }
                Err(e) => {
                    eprintln!("❌ 激活失败: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        "status" => {
            // boot 已经打印了
        }
        "deactivate" => {
            println!("调用 deactivate_local...");
            match manager.deactivate_local().await {
                Ok(_) => {
                    println!("✅ 已解绑");
                    print_state(&manager.current());
                }
                Err(e) => {
                    eprintln!("❌ 解绑失败: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        "heartbeat" => {
            let payload = match manager.current_payload() {
                Some(p) => p,
                None => {
                    eprintln!("❌ 本地无 .lic（或验签失败），无法 heartbeat");
                    std::process::exit(1);
                }
            };
            println!(
                "调用 heartbeat(license_id={}, device_id={})...",
                payload.license_id, payload.device_id
            );
            let body = api_client::HeartbeatBody {
                license_id: &payload.license_id,
                device_id: &payload.device_id,
                fingerprint: &payload.fingerprint,
            };
            match api_client::heartbeat(&body).await {
                Ok(data) => {
                    println!("✅ heartbeat 返回");
                    println!("  status:        {}", data.status);
                    println!("  next_check_at: {} (unix)", data.next_check_at);
                    if data.status == "revoked" {
                        println!("⚠️  服务端已吊销，client 应进入 Revoked 态");
                    }
                }
                Err(e) => {
                    eprintln!("❌ heartbeat 失败: {:?}", e);
                    std::process::exit(1);
                }
            }
        }
        "read-cert" => match manager.current_payload() {
            Some(p) => {
                println!("=== current_payload ===");
                println!("  license_id:    {}", p.license_id);
                println!("  license_key:   {}", p.license_key);
                println!("  email:         {}", p.email);
                println!("  plan:          {}", p.plan);
                println!("  device_id:     {}", p.device_id);
                println!("  fingerprint:   {}", p.fingerprint);
                println!("  issued_at:     {}", p.issued_at);
                println!("  expires_at:    {:?}", p.expires_at);
                println!("  next_check_at: {}", p.next_check_at);
                println!("  key_version:   {:?}", p.key_version);
            }
            None => println!("(无 .lic 或验签失败)"),
        },
        other => {
            eprintln!("未知子命令: {}", other);
            std::process::exit(1);
        }
    }
}
