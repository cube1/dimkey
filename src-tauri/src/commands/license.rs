// src-tauri/src/commands/license.rs
//
// Phase 9：把 LicenseManager 通过 Tauri State 注入前端可调用的 10 个 commands。
// 错误统一返回 LicenseError（已 derive Serialize → 前端 reject(payload)）。
//
// 与前端契约：
// - license_get_state 返回 LicenseState（kind=PascalCase）
// - license_get_fingerprint 返回 32 hex 字符串
// - license_get_fingerprint_mismatch_hint：仅在本地存在 .lic 且 fingerprint 与本机不匹配时
//   返回旧指纹前 8 字符（spec §4.5 Step 3）
// - license_activate 接 license_key + email，自动取 hostname/os/flavor/app_version

use crate::license::api_client::{self, DeviceDto};
use crate::license::errors::LicenseError;
use crate::license::state::{LicenseManager, LicenseState};
use serde::Serialize;
use std::sync::Arc;
use tauri::State;

/// LicenseManager 全局状态 newtype（Tauri State::manage 用）
pub struct LicenseManagerState(pub Arc<LicenseManager>);

#[derive(Serialize)]
pub struct TrialInfoDto {
    pub days_remaining: u32,
    pub expired: bool,
}

#[derive(Serialize)]
pub struct ActivateResultDto {
    pub email: String,
    pub max_devices: u32,
    pub active_devices: u32,
    pub device_id: String,
}

#[tauri::command]
pub fn license_get_state(state: State<LicenseManagerState>) -> LicenseState {
    state.0.current()
}

#[tauri::command]
pub fn license_get_fingerprint(state: State<LicenseManagerState>) -> String {
    state.0.machine_fp().to_string()
}

/// 当本地存在 .lic 但其指纹与本机不匹配时，返回旧指纹前 8 字符；否则 None。
/// 用于 About 面板顶部展示"此授权文件属于另一台机器"提示（spec §4.5 Step 3）。
#[tauri::command]
pub fn license_get_fingerprint_mismatch_hint(
    state: State<LicenseManagerState>,
) -> Option<String> {
    let payload = state.0.current_payload()?;
    if payload.fingerprint != state.0.machine_fp() {
        Some(payload.fingerprint[..8.min(payload.fingerprint.len())].to_string())
    } else {
        None
    }
}

#[tauri::command]
pub fn license_get_trial_info(state: State<LicenseManagerState>) -> TrialInfoDto {
    match state.0.current() {
        LicenseState::Trial { days_remaining } => TrialInfoDto {
            days_remaining,
            expired: false,
        },
        LicenseState::TrialExpired => TrialInfoDto {
            days_remaining: 0,
            expired: true,
        },
        _ => TrialInfoDto {
            days_remaining: 0,
            expired: false,
        },
    }
}

#[tauri::command]
pub async fn license_activate(
    state: State<'_, LicenseManagerState>,
    license_key: String,
    email: String,
) -> Result<ActivateResultDto, LicenseError> {
    let machine_label = hostname();
    let os = if cfg!(target_os = "macos") { "macos" } else { "windows" };
    let flavor = if cfg!(feature = "lang-en") { "en" } else { "zh" };
    let app_version = env!("CARGO_PKG_VERSION");
    let result = state
        .0
        .try_activate(&license_key, &email, &machine_label, os, flavor, app_version)
        .await?;
    Ok(ActivateResultDto {
        email,
        max_devices: result.device_summary.max_devices,
        active_devices: result.device_summary.active_count,
        device_id: result.device_summary.current_device_id,
    })
}

#[tauri::command]
pub async fn license_deactivate_current(
    state: State<'_, LicenseManagerState>,
) -> Result<(), LicenseError> {
    state.0.deactivate_local().await
}

#[tauri::command]
pub async fn license_list_devices(
    state: State<'_, LicenseManagerState>,
) -> Result<Vec<DeviceDto>, LicenseError> {
    let payload = state
        .0
        .current_payload()
        .ok_or(LicenseError::InvalidLicense)?;
    let fp = state.0.machine_fp().to_string();
    let body = api_client::DevicesListBody {
        license_key: &payload.license_key,
        email: &payload.email,
        fingerprint: Some(&fp),
    };
    let data = api_client::list_devices(&body).await?;
    Ok(data.devices)
}

#[tauri::command]
pub async fn license_deactivate_device(
    state: State<'_, LicenseManagerState>,
    device_id: String,
) -> Result<(), LicenseError> {
    let payload = state
        .0
        .current_payload()
        .ok_or(LicenseError::InvalidLicense)?;
    let body = api_client::DeactivateBody {
        license_key: &payload.license_key,
        email: &payload.email,
        device_id: Some(&device_id),
        fingerprint: None,
    };
    api_client::deactivate(&body).await
}

#[tauri::command]
pub async fn license_recover_email(email: String) -> Result<(), LicenseError> {
    let body = api_client::RecoverBody { email: &email };
    api_client::recover(&body).await
}

/// 用户主动触发的强制 heartbeat — 用于 GraceMode "立即联网恢复" 或 Activated "重新验证"。
/// 绕过周期任务的 next_check_at 检查，立刻发请求；错误冒泡到前端。
#[tauri::command]
pub async fn license_force_heartbeat(
    app: tauri::AppHandle,
    state: State<'_, LicenseManagerState>,
) -> Result<(), LicenseError> {
    crate::license::heartbeat::force_check(&app, &state.0).await
}

#[tauri::command]
pub fn license_open_purchase_page(app: tauri::AppHandle) -> Result<(), LicenseError> {
    use tauri_plugin_opener::OpenerExt;
    let url = "https://dimkey.app/buy";
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| LicenseError::ServerError {
            code: "OPEN".into(),
            message: e.to_string(),
        })
}

fn hostname() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "Unknown".to_string())
}
