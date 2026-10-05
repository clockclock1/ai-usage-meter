use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine as _,
};
use reqwest::{Client as HttpClient, Proxy};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    env,
    ffi::OsString,
    fs,
    fs::File,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering},
        mpsc, Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalSize, Manager, Monitor, PhysicalPosition, State, WebviewWindow,
    WindowEvent,
};

#[cfg(target_os = "windows")]
mod windows_integration;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

const FLOATING_CARD_WIDTH: f64 = 276.0;
const FLOATING_CARD_HEIGHT: f64 = 150.0;
// User-configured floating dimensions are logical CSS pixels. Tauri and the
// Per-Monitor-V2 WebView scale them to physical pixels for each display.
const DEFAULT_FLOATING_CARD_SCALE: u32 = 100;
const MIN_FLOATING_CARD_SCALE: u32 = 80;
const MAX_FLOATING_CARD_SCALE: u32 = 140;
const FLOATING_ORB_SIZE: u32 = 56;
const MIN_FLOATING_ORB_SIZE: u32 = 48;
const MAX_FLOATING_ORB_SIZE: u32 = 88;
const FLOATING_ORB_VISUAL_INSET: f64 = 6.0;
const ORB_EDGE_DOCK_TOLERANCE: f64 = 2.0;
const FLOATING_ORB_PANEL_WIDTH: f64 = 162.0;
const DEFAULT_ORB_WAVE_SPEED: f64 = 2.0;
const MIN_ORB_WAVE_SPEED: f64 = 0.5;
const MAX_ORB_WAVE_SPEED: f64 = 3.0;
const DEFAULT_ORB_WAVE_AMPLITUDE: f64 = 1.0;
const MIN_ORB_WAVE_AMPLITUDE: f64 = 0.5;
const MAX_ORB_WAVE_AMPLITUDE: f64 = 4.0;
const DEFAULT_SYNC_INTERVAL_SECS: u64 = 60;
const MIN_SYNC_INTERVAL_SECS: u64 = 5;
const MAX_SYNC_INTERVAL_SECS: u64 = 86_400;
const CURSOR_TOKEN_SYNC_INTERVAL: Duration = Duration::from_secs(5 * 60);
const CURSOR_TOKEN_SYNC_TIMEOUT: Duration = Duration::from_secs(150);
static CURSOR_LAST_SYNC_ATTEMPT: Mutex<Option<Instant>> = Mutex::new(None);
static CURSOR_CREDENTIALS_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RateWindow {
    used_percent: f64,
    window_duration_mins: u64,
    resets_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageSnapshot {
    email: Option<String>,
    plan_type: Option<String>,
    primary: Option<RateWindow>,
    secondary: Option<RateWindow>,
    credit_balance: Option<String>,
    has_credits: bool,
    unlimited: bool,
    reset_credits: u64,
    fetched_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TokenUsageStats {
    total_tokens: u64,
    today_tokens: u64,
    sessions_scanned: u64,
    unreadable_sessions: u64,
    available: bool,
    updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorQuotaSnapshot {
    status: String,
    message: Option<String>,
    email: Option<String>,
    membership_type: Option<String>,
    cursor_models_percent: Option<f64>,
    other_models_percent: Option<f64>,
    total_percent: Option<f64>,
    plan_used_usd: Option<f64>,
    plan_limit_usd: Option<f64>,
    plan_remaining_usd: Option<f64>,
    on_demand_used_usd: Option<f64>,
    on_demand_limit_usd: Option<f64>,
    on_demand_remaining_usd: Option<f64>,
    requests_used: Option<u64>,
    requests_limit: Option<u64>,
    billing_cycle_end: Option<String>,
    fetched_at: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorTokenUsageStats {
    total_tokens: u64,
    today_tokens: u64,
    events_scanned: u64,
    cache_available: bool,
    sync_performed: bool,
    sync_error: Option<String>,
    updated_at: Option<u64>,
}

#[derive(Default)]
struct SessionTokenUsage {
    total_tokens: u64,
    fallback_total_tokens: u64,
    has_total_usage: bool,
    today_tokens: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct SavedPosition {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct PersistedState {
    active_platform: String,
    codex_path: Option<PathBuf>,
    last_usage: Option<UsageSnapshot>,
    last_cursor_quota: Option<CursorQuotaSnapshot>,
    main_position: Option<SavedPosition>,
    #[serde(alias = "floatingPosition")]
    floating_position: Option<SavedPosition>,
    floating_card_position: Option<SavedPosition>,
    floating_orb_position: Option<SavedPosition>,
    floating_visible: bool,
    floating_pinned: bool,
    floating_opacity: f64,
    floating_always_on_top: bool,
    floating_style: String,
    floating_orb_expand_direction: String,
    floating_orb_size: u32,
    floating_card_scale: u32,
    orb_wave_speed: f64,
    orb_wave_amplitude: f64,
    sync_interval_secs: u64,
    #[serde(default, rename = "syncIntervalMins", skip_serializing)]
    legacy_sync_interval_mins: Option<u64>,
    last_quota_sync_at_ms: Option<u64>,
    display_mode: String,
    theme: String,
    proxy_mode: String,
    proxy_address: String,
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            active_platform: "codex".to_owned(),
            codex_path: None,
            last_usage: None,
            last_cursor_quota: None,
            main_position: None,
            floating_position: None,
            floating_card_position: None,
            floating_orb_position: None,
            floating_visible: false,
            floating_pinned: false,
            floating_opacity: 0.92,
            floating_always_on_top: true,
            floating_style: "card".to_owned(),
            floating_orb_expand_direction: "auto".to_owned(),
            floating_orb_size: FLOATING_ORB_SIZE,
            floating_card_scale: DEFAULT_FLOATING_CARD_SCALE,
            orb_wave_speed: DEFAULT_ORB_WAVE_SPEED,
            orb_wave_amplitude: DEFAULT_ORB_WAVE_AMPLITUDE,
            sync_interval_secs: DEFAULT_SYNC_INTERVAL_SECS,
            legacy_sync_interval_mins: None,
            last_quota_sync_at_ms: None,
            display_mode: "available".to_owned(),
            theme: "obsidian".to_owned(),
            proxy_mode: "system".to_owned(),
            proxy_address: String::new(),
        }
    }
}

struct AppState {
    file_path: PathBuf,
    data: Mutex<PersistedState>,
    save_sender: mpsc::Sender<PersistedState>,
    floating_pinned: Arc<AtomicBool>,
    floating_orb: Arc<AtomicBool>,
    floating_orb_size: Arc<AtomicU32>,
    floating_card_scale: Arc<AtomicU32>,
    floating_orb_dragging: AtomicBool,
    floating_orb_expanded: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FloatingSettings {
    active_platform: String,
    visible: bool,
    pinned: bool,
    opacity: f64,
    always_on_top: bool,
    style: String,
    orb_expand_direction: String,
    orb_size: u32,
    card_scale: u32,
    orb_wave_speed: f64,
    orb_wave_amplitude: f64,
    sync_interval_secs: u64,
    display_mode: String,
    theme: String,
    proxy_mode: String,
    proxy_address: String,
    data_directory: String,
}

#[derive(Debug, Clone)]
struct NetworkSettings {
    mode: String,
    address: String,
}

#[derive(Debug)]
struct CodexLauncher {
    source_path: PathBuf,
    program: PathBuf,
    args: Vec<OsString>,
}

fn load_state(path: &Path) -> PersistedState {
    let data = fs::read_to_string(path)
        .ok()
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default();
    migrate_sync_interval(data)
}

fn migrate_sync_interval(mut data: PersistedState) -> PersistedState {
    if let Some(legacy_minutes) = data.legacy_sync_interval_mins.take() {
        data.sync_interval_secs = legacy_minutes.saturating_mul(60);
    }
    data.sync_interval_secs = data
        .sync_interval_secs
        .clamp(MIN_SYNC_INTERVAL_SECS, MAX_SYNC_INTERVAL_SECS);
    data.floating_orb_size = data
        .floating_orb_size
        .clamp(MIN_FLOATING_ORB_SIZE, MAX_FLOATING_ORB_SIZE);
    data.floating_card_scale = data
        .floating_card_scale
        .clamp(MIN_FLOATING_CARD_SCALE, MAX_FLOATING_CARD_SCALE);
    data
}

fn quota_sync_wait_ms(last_sync_at_ms: Option<u64>, interval_secs: u64, now_ms: u64) -> u64 {
    let Some(last_sync_at_ms) = last_sync_at_ms else {
        return 0;
    };
    let interval_ms = interval_secs.saturating_mul(1_000);
    last_sync_at_ms
        .saturating_add(interval_ms)
        .saturating_sub(now_ms)
}

fn reserve_quota_sync(data: &mut PersistedState, now_ms: u64, force: bool) -> u64 {
    let wait_ms = quota_sync_wait_ms(data.last_quota_sync_at_ms, data.sync_interval_secs, now_ms);
    if !force && wait_ms > 0 {
        return wait_ms;
    }
    data.last_quota_sync_at_ms = Some(now_ms);
    0
}

fn executable_data_dir() -> Result<PathBuf, String> {
    let executable =
        env::current_exe().map_err(|error| format!("无法定位应用程序目录：{error}"))?;
    let directory = executable
        .parent()
        .ok_or_else(|| "无法定位应用程序目录".to_owned())?;
    Ok(directory.join("data"))
}

fn migrate_legacy_state(legacy_file: &Path, target_file: &Path) {
    if !target_file.exists() && legacy_file.is_file() {
        let _ = fs::copy(legacy_file, target_file);
    }
}

fn save_state(file_path: &Path, snapshot: &PersistedState) -> Result<(), String> {
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("无法创建 data 目录：{error}"))?;
    }
    let contents = serde_json::to_vec_pretty(snapshot)
        .map_err(|error| format!("无法序列化本地状态：{error}"))?;
    fs::write(file_path, contents).map_err(|error| format!("无法保存本地状态：{error}"))
}

fn update_state(state: &AppState, update: impl FnOnce(&mut PersistedState)) -> Result<(), String> {
    let snapshot = {
        let mut data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        update(&mut data);
        data.clone()
    };
    state
        .save_sender
        .send(snapshot)
        .map_err(|_| "后台存储线程已停止".to_owned())
}

fn start_state_writer(file_path: PathBuf) -> mpsc::Sender<PersistedState> {
    let (sender, receiver) = mpsc::channel::<PersistedState>();
    thread::spawn(move || {
        while let Ok(mut latest) = receiver.recv() {
            while let Ok(newer) = receiver.try_recv() {
                latest = newer;
            }
            let _ = save_state(&file_path, &latest);
        }
    });
    sender
}

fn add_candidate(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if path.is_file() && !candidates.iter().any(|item| item == &path) {
        candidates.push(path);
    }
}

fn launcher_from_path(path: PathBuf) -> CodexLauncher {
    let script = path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("cmd") || value.eq_ignore_ascii_case("bat")
        });
    if script {
        let command = format!(
            "\"{}\" app-server --listen stdio://",
            path.to_string_lossy()
        );
        CodexLauncher {
            source_path: path,
            program: PathBuf::from("cmd.exe"),
            args: ["/D", "/S", "/C"]
                .into_iter()
                .map(OsString::from)
                .chain([OsString::from(command)])
                .collect(),
        }
    } else {
        CodexLauncher {
            source_path: path.clone(),
            program: path,
            args: ["app-server", "--listen", "stdio://"]
                .into_iter()
                .map(OsString::from)
                .collect(),
        }
    }
}

#[cfg(target_os = "windows")]
fn find_codex_launcher(cached: Option<&Path>) -> Result<CodexLauncher, String> {
    let mut candidates = Vec::new();
    if let Some(path) = cached {
        add_candidate(&mut candidates, path.to_path_buf());
    }
    if let Some(path) = env::var_os("CODEX_QUOTA_CODEX_PATH") {
        add_candidate(&mut candidates, PathBuf::from(path));
    }
    if let Some(local) = env::var_os("LOCALAPPDATA") {
        let bin = PathBuf::from(local).join("OpenAI/Codex/bin");
        add_candidate(&mut candidates, bin.join("codex.exe"));
        if let Ok(entries) = fs::read_dir(&bin) {
            let mut paths: Vec<_> = entries
                .flatten()
                .map(|entry| entry.path().join("codex.exe"))
                .filter(|path| path.is_file())
                .collect();
            paths.sort_by_key(|path| {
                fs::metadata(path)
                    .and_then(|metadata| metadata.modified())
                    .ok()
            });
            paths.reverse();
            for path in paths {
                add_candidate(&mut candidates, path);
            }
        }
    }
    if let Some(value) = env::var_os("PATH") {
        for directory in env::split_paths(&value) {
            add_candidate(&mut candidates, directory.join("codex.exe"));
            add_candidate(&mut candidates, directory.join("codex.cmd"));
            add_candidate(&mut candidates, directory.join("codex.bat"));
        }
    }
    if let Some(app_data) = env::var_os("APPDATA") {
        add_candidate(
            &mut candidates,
            PathBuf::from(app_data).join("npm/codex.cmd"),
        );
    }
    candidates.into_iter().next().map(launcher_from_path).ok_or_else(||
        "未找到 Codex。请先安装 Codex 桌面应用或 Codex CLI，并登录 ChatGPT 账号；也可以通过 CODEX_QUOTA_CODEX_PATH 指定 codex.exe。".to_owned())
}

#[cfg(not(target_os = "windows"))]
fn find_codex_launcher(cached: Option<&Path>) -> Result<CodexLauncher, String> {
    Ok(launcher_from_path(
        cached.unwrap_or_else(|| Path::new("codex")).to_path_buf(),
    ))
}

fn parse_window(value: Option<&Value>) -> Option<RateWindow> {
    let value = value?;
    Some(RateWindow {
        used_percent: value.get("usedPercent")?.as_f64()?,
        window_duration_mins: value.get("windowDurationMins")?.as_u64()?,
        resets_at: value.get("resetsAt")?.as_u64()?,
    })
}

fn parse_usage_responses(
    rate: &Value,
    account: Option<&Value>,
    fetched_at: u64,
) -> Result<UsageSnapshot, String> {
    let account_result = account.and_then(|value| value.get("result"));
    let account_info = account_result.and_then(|value| value.get("account"));
    if account_result
        .and_then(|value| value.get("requiresOpenaiAuth"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        && account_info.is_none_or(Value::is_null)
    {
        return Err("这台电脑上的 Codex 尚未登录。请先打开 Codex 登录 ChatGPT 账号，或在终端执行 codex login，然后返回应用重试。".to_owned());
    }
    if account_info
        .and_then(|value| value.get("type"))
        .and_then(Value::as_str)
        .is_some_and(|value| value.eq_ignore_ascii_case("apiKey"))
    {
        return Err("当前 Codex 使用 API Key 登录，无法读取 ChatGPT 订阅额度。请改用 ChatGPT 账号登录 Codex。".to_owned());
    }
    if let Some(error) = rate.get("error") {
        return Err(format!(
            "Codex 无法读取额度：{}",
            error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("未知错误")
        ));
    }
    let result = rate.get("result").ok_or("Codex 没有返回额度结果")?;
    let limits = result
        .pointer("/rateLimitsByLimitId/codex")
        .or_else(|| result.get("rateLimits"))
        .ok_or("Codex 没有返回可识别的额度数据")?;
    let credits = limits.get("credits");
    Ok(UsageSnapshot {
        email: account_info
            .and_then(|value| value.get("email"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        plan_type: limits
            .get("planType")
            .and_then(Value::as_str)
            .or_else(|| {
                account_info
                    .and_then(|value| value.get("planType"))
                    .and_then(Value::as_str)
            })
            .map(str::to_owned),
        primary: parse_window(limits.get("primary")),
        secondary: parse_window(limits.get("secondary")),
        credit_balance: credits
            .and_then(|value| value.get("balance"))
            .and_then(Value::as_str)
            .map(str::to_owned),
        has_credits: credits
            .and_then(|value| value.get("hasCredits"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        unlimited: credits
            .and_then(|value| value.get("unlimited"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        reset_credits: result
            .pointer("/rateLimitResetCredits/availableCount")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        fetched_at,
    })
}

fn configure_proxy(command: &mut Command, network: &NetworkSettings) -> Result<(), String> {
    const PROXY_VARS: [&str; 6] = [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ];
    match network.mode.as_str() {
        "system" => {}
        "none" => {
            for key in PROXY_VARS {
                command.env_remove(key);
            }
            command.env("NO_PROXY", "*").env("no_proxy", "*");
        }
        "custom" => {
            let address = normalize_proxy_address(&network.address)?;
            for key in PROXY_VARS {
                command.env(key, &address);
            }
            command.env("NO_PROXY", "localhost,127.0.0.1,::1");
        }
        _ => return Err("代理模式无效".to_owned()),
    }
    Ok(())
}

fn normalize_proxy_address(address: &str) -> Result<String, String> {
    let address = address.trim();
    if address.is_empty() {
        return Err("请填写本地代理地址，例如 http://127.0.0.1:7890".to_owned());
    }
    let normalized = if address.contains("://") {
        address.to_owned()
    } else {
        format!("http://{address}")
    };
    if !(normalized.starts_with("http://")
        || normalized.starts_with("https://")
        || normalized.starts_with("socks5://"))
    {
        return Err("代理地址仅支持 http、https 或 socks5".to_owned());
    }
    Ok(normalized)
}

fn user_home_directory() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
}

fn cursor_desktop_database_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(app_data) = env::var_os("APPDATA") {
        candidates.push(
            PathBuf::from(app_data)
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb"),
        );
    }
    if let Some(home) = user_home_directory() {
        candidates.push(
            home.join("AppData")
                .join("Roaming")
                .join("Cursor")
                .join("User")
                .join("globalStorage")
                .join("state.vscdb"),
        );
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn read_cursor_desktop_access_token() -> Result<Option<String>, String> {
    for path in cursor_desktop_database_candidates() {
        if !path.is_file() {
            continue;
        }
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| "无法读取 Cursor 本机登录状态；请关闭 Cursor 后重试。".to_owned())?;
        connection
            .busy_timeout(Duration::from_secs(2))
            .map_err(|_| "无法读取 Cursor 本机登录状态。".to_owned())?;
        let token = connection
            .query_row(
                "SELECT value FROM ItemTable WHERE key = ?1",
                ["cursorAuth/accessToken"],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(|_| "无法读取 Cursor 本机登录状态；请更新 Cursor 后重试。".to_owned())?;
        if token
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok(token);
        }
    }
    Ok(None)
}

fn canonical_cursor_user_id(value: &str) -> Option<String> {
    let start = value.find("user_")?;
    let user_id: String = value[start..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect();
    (user_id.starts_with("user_") && user_id.len() > 5).then_some(user_id)
}

fn cursor_user_id_from_token(token: &str) -> Option<String> {
    for separator in ["%3A%3A", "::"] {
        if let Some((prefix, _)) = token.split_once(separator) {
            if let Some(user_id) = canonical_cursor_user_id(prefix) {
                return Some(user_id);
            }
        }
    }

    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE
        .decode(payload)
        .or_else(|_| URL_SAFE_NO_PAD.decode(payload))
        .ok()?;
    let value: Value = serde_json::from_slice(&decoded).ok()?;
    canonical_cursor_user_id(value.get("sub")?.as_str()?)
}

fn normalize_cursor_session_token(input: &str) -> Option<(String, String)> {
    let mut token = input.trim().to_owned();
    if token.len() > 16 * 1024 || token.is_empty() {
        return None;
    }
    if let Ok(decoded) = serde_json::from_str::<String>(&token) {
        token = decoded.trim().to_owned();
    }
    if token.to_ascii_lowercase().starts_with("cookie:") {
        token = token[7..].trim().to_owned();
    }
    if let Some((_, value)) = token
        .split_once("WorkosCursorSessionToken=")
        .or_else(|| token.split_once("workoscursorsessiontoken="))
    {
        token = value.split([';', ' ', '\t', '\r', '\n']).next()?.to_owned();
    }
    if token.len() > 1
        && ((token.starts_with('"') && token.ends_with('"'))
            || (token.starts_with('\'') && token.ends_with('\'')))
    {
        token = token[1..token.len() - 1].trim().to_owned();
    }
    if token.is_empty() || token.chars().any(char::is_whitespace) {
        return None;
    }

    if let Some((prefix, jwt)) = token.split_once("::") {
        token = format!("{prefix}%3A%3A{jwt}");
    }
    let user_id = cursor_user_id_from_token(&token)?;
    if !token.contains("%3A%3A") {
        token = format!("{user_id}%3A%3A{token}");
    }
    Some((user_id, token))
}

fn read_saved_cursor_session_token(home: &Path) -> Option<String> {
    let path = home
        .join(".config")
        .join("tokscale")
        .join("cursor-credentials.json");
    let store: Value = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    let accounts = store.get("accounts")?.as_object()?;
    let active = store.get("activeAccountId").and_then(Value::as_str);
    active
        .and_then(|id| accounts.get(id))
        .and_then(|account| account.get("sessionToken"))
        .and_then(Value::as_str)
        .or_else(|| {
            accounts
                .values()
                .find_map(|account| account.get("sessionToken").and_then(Value::as_str))
        })
        .map(str::to_owned)
}

fn persist_cursor_session_token(home: &Path, user_id: &str, token: &str) -> Result<(), String> {
    let _guard = CURSOR_CREDENTIALS_LOCK
        .lock()
        .map_err(|_| "Tokscale Cursor 凭据正在被其他任务更新。".to_owned())?;
    let path = home
        .join(".config")
        .join("tokscale")
        .join("cursor-credentials.json");
    let mut store = if path.exists() {
        let raw =
            fs::read(&path).map_err(|_| "无法更新 Tokscale 的 Cursor 本地凭据。".to_owned())?;
        serde_json::from_slice::<Value>(&raw)
            .map_err(|_| "Tokscale 的 Cursor 凭据文件格式无效，未覆盖原文件。".to_owned())?
    } else {
        json!({ "version": 1, "activeAccountId": user_id, "accounts": {} })
    };
    if !store.is_object() {
        return Err("Tokscale 的 Cursor 凭据文件格式无效，未覆盖原文件。".to_owned());
    }
    {
        let accounts = store
            .as_object_mut()
            .and_then(|object| {
                object
                    .entry("accounts")
                    .or_insert_with(|| json!({}))
                    .as_object_mut()
            })
            .ok_or_else(|| "Tokscale 的 Cursor 凭据文件格式无效，未覆盖原文件。".to_owned())?;
        let previous_created_at = accounts
            .get(user_id)
            .and_then(|account| account.get("createdAt"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| {
                time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default()
            });
        accounts.insert(
            user_id.to_owned(),
            json!({
                "sessionToken": token,
                "userId": user_id,
                "createdAt": previous_created_at,
                "expiresAt": null,
                "label": null
            }),
        );
    }
    // Cursor's local session is authoritative; never leave Tokscale pointed at a previously valid account.
    store["activeAccountId"] = json!(user_id);
    if store.get("version").is_none() {
        store["version"] = json!(1);
    }

    let parent = path
        .parent()
        .ok_or_else(|| "无法定位 Tokscale 配置目录。".to_owned())?;
    fs::create_dir_all(parent).map_err(|_| "无法创建 Tokscale 的 Cursor 配置目录。".to_owned())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary_path = path.with_extension(format!("json.{}-{nonce}.tmp", std::process::id()));
    let serialized = serde_json::to_vec_pretty(&store)
        .map_err(|_| "无法序列化 Tokscale Cursor 凭据。".to_owned())?;
    fs::write(&temporary_path, serialized)
        .map_err(|_| "无法写入 Tokscale 的 Cursor 本地凭据。".to_owned())?;
    if let Err(error) = fs::rename(&temporary_path, &path) {
        let _ = fs::remove_file(&temporary_path);
        return Err(format!("无法安全更新 Tokscale 的 Cursor 凭据：{error}"));
    }
    Ok(())
}

fn resolve_current_cursor_session() -> Result<Option<(String, String)>, String> {
    let Some(home) = user_home_directory() else {
        return Ok(None);
    };
    if let Some(access_token) = read_cursor_desktop_access_token()? {
        let (user_id, session_token) =
            normalize_cursor_session_token(&access_token).ok_or_else(|| {
                "Cursor 本机登录令牌格式无法识别；请更新 Cursor 并重新登录。".to_owned()
            })?;
        persist_cursor_session_token(&home, &user_id, &session_token)?;
        return Ok(Some((user_id, session_token)));
    }
    Ok(None)
}

fn resolve_cursor_session_token() -> Result<Option<String>, String> {
    if let Some((_, session_token)) = resolve_current_cursor_session()? {
        return Ok(Some(session_token));
    }
    let Some(home) = user_home_directory() else {
        return Ok(None);
    };
    Ok(read_saved_cursor_session_token(&home))
}

fn cursor_http_client(network: &NetworkSettings) -> Result<HttpClient, String> {
    let builder = HttpClient::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/136.0.0.0 Safari/537.36");
    // Cursor's edge protection fingerprints TLS ClientHello and has been
    // observed to challenge rustls clients. On Windows, use the system TLS
    // stack (Schannel) for Cursor only; other app traffic remains on rustls.
    #[cfg(windows)]
    let builder = builder.use_native_tls();
    let builder = match network.mode.as_str() {
        "system" => builder,
        "none" => builder.no_proxy(),
        "custom" => {
            let address = normalize_proxy_address(&network.address)?;
            builder
                .no_proxy()
                .proxy(Proxy::all(&address).map_err(|_| "本地代理地址无效。".to_owned())?)
        }
        _ => return Err("代理模式无效。".to_owned()),
    };
    builder
        .build()
        .map_err(|_| "无法初始化 Cursor 用量连接。请检查代理设置。".to_owned())
}

fn cursor_http_status_error(status: reqwest::StatusCode) -> Option<String> {
    match status {
        reqwest::StatusCode::UNAUTHORIZED => Some("unauthorized".to_owned()),
        reqwest::StatusCode::FORBIDDEN => Some("forbidden".to_owned()),
        _ if !status.is_success() => Some(format!("http_status_{}", status.as_u16())),
        _ => None,
    }
}

fn cursor_fetch_error_snapshot(error: &str, context: &str) -> CursorQuotaSnapshot {
    match error {
        "unauthorized" => cursor_quota_status(
            "unauthorized",
            "Cursor 返回 HTTP 401，当前登录认证已失效；请在 Cursor 桌面端重新登录。",
        ),
        "forbidden" => cursor_quota_status(
            "unavailable",
            "Cursor 返回 HTTP 403，当前请求被服务拒绝；这不一定代表登录失效。请检查网络/代理后重试。",
        ),
        _ if error.starts_with("http_status_") => {
            let status = error.trim_start_matches("http_status_");
            cursor_quota_status(
                "unavailable",
                &format!("Cursor 用量服务返回 HTTP {status}，请稍后重试。"),
            )
        }
        _ => cursor_quota_status("unavailable", &format!("{context}：{error}")),
    }
}

async fn fetch_cursor_json(
    client: &HttpClient,
    url: &str,
    session_token: &str,
) -> Result<Value, String> {
    let response = client
        .get(url)
        .header(reqwest::header::ACCEPT, "*/*")
        .header(reqwest::header::ACCEPT_LANGUAGE, "en-US,en;q=0.9")
        .header(reqwest::header::REFERER, "https://cursor.com/dashboard")
        .header(
            reqwest::header::COOKIE,
            format!("WorkosCursorSessionToken={session_token}"),
        )
        .send()
        .await
        .map_err(|_| "无法连接 Cursor 用量服务；请检查网络与代理设置。".to_owned())?;
    let status = response.status();
    if let Some(error) = cursor_http_status_error(status) {
        return Err(error);
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let body = response
        .text()
        .await
        .map_err(|_| "无法读取 Cursor 用量服务响应。".to_owned())?;
    serde_json::from_str::<Value>(&body).map_err(|_| {
        if content_type.contains("text/html") || body.trim_start().starts_with('<') {
            "Cursor 返回了网页验证响应（非 JSON），不是登录失效；请检查网络/代理后重试。".to_owned()
        } else {
            "Cursor 用量服务返回了无法识别的数据。".to_owned()
        }
    })
}

fn number_at(value: &Value, path: &str) -> Option<f64> {
    value
        .pointer(path)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
}

fn clamp_usage_percent(value: Option<f64>) -> Option<f64> {
    value.map(|number| number.clamp(0.0, 100.0))
}

fn percent_from_usage(used: Option<f64>, limit: Option<f64>) -> Option<f64> {
    match (used, limit) {
        (Some(used), Some(limit)) if limit > 0.0 => clamp_usage_percent(Some(used / limit * 100.0)),
        _ => None,
    }
}

fn cents_to_dollars(value: Option<f64>) -> Option<f64> {
    value.map(|cents| (cents.round()) / 100.0)
}

fn parse_cursor_usage_summary(
    summary: &Value,
    account: &Value,
    request_usage: Option<&Value>,
    fetched_at: u64,
) -> CursorQuotaSnapshot {
    let plan_used_cents = number_at(summary, "/individualUsage/plan/used");
    let plan_limit_cents = number_at(summary, "/individualUsage/plan/limit");
    let overall_used_cents = number_at(summary, "/individualUsage/overall/used");
    let overall_limit_cents = number_at(summary, "/individualUsage/overall/limit");
    let pooled_used_cents = number_at(summary, "/teamUsage/pooled/used");
    let pooled_limit_cents = number_at(summary, "/teamUsage/pooled/limit");
    let used_cents = plan_used_cents.or(overall_used_cents).or(pooled_used_cents);
    let limit_cents = plan_limit_cents
        .or(overall_limit_cents)
        .or(pooled_limit_cents);
    let remaining_cents = number_at(summary, "/individualUsage/plan/remaining")
        .or_else(|| number_at(summary, "/individualUsage/overall/remaining"))
        .or_else(|| number_at(summary, "/teamUsage/pooled/remaining"))
        .or_else(|| {
            used_cents
                .zip(limit_cents)
                .map(|(used, limit)| (limit - used).max(0.0))
        });
    let auto_percent =
        clamp_usage_percent(number_at(summary, "/individualUsage/plan/autoPercentUsed"));
    let api_percent =
        clamp_usage_percent(number_at(summary, "/individualUsage/plan/apiPercentUsed"));
    let total_percent =
        clamp_usage_percent(number_at(summary, "/individualUsage/plan/totalPercentUsed"))
            .or_else(|| percent_from_usage(used_cents, limit_cents));
    let request_bucket =
        request_usage.and_then(|usage| usage.get("gpt-4").or_else(|| usage.get("gpt4")));
    let requests_used = request_bucket
        .and_then(|usage| {
            usage
                .get("numRequestsTotal")
                .or_else(|| usage.get("numRequests"))
        })
        .and_then(Value::as_u64);
    let requests_limit = request_bucket
        .and_then(|usage| usage.get("maxRequestUsage"))
        .and_then(Value::as_u64);
    CursorQuotaSnapshot {
        status: "connected".to_owned(),
        message: None,
        email: account
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_owned),
        membership_type: summary
            .get("membershipType")
            .and_then(Value::as_str)
            .map(str::to_owned),
        cursor_models_percent: auto_percent,
        other_models_percent: api_percent,
        total_percent,
        plan_used_usd: cents_to_dollars(used_cents),
        plan_limit_usd: cents_to_dollars(limit_cents),
        plan_remaining_usd: cents_to_dollars(remaining_cents),
        on_demand_used_usd: cents_to_dollars(number_at(summary, "/individualUsage/onDemand/used")),
        on_demand_limit_usd: cents_to_dollars(number_at(
            summary,
            "/individualUsage/onDemand/limit",
        )),
        on_demand_remaining_usd: cents_to_dollars(number_at(
            summary,
            "/individualUsage/onDemand/remaining",
        )),
        requests_used,
        requests_limit,
        billing_cycle_end: summary
            .get("billingCycleEnd")
            .and_then(Value::as_str)
            .map(str::to_owned),
        fetched_at,
    }
}

fn cursor_quota_status(status: &str, message: &str) -> CursorQuotaSnapshot {
    CursorQuotaSnapshot {
        status: status.to_owned(),
        message: Some(message.to_owned()),
        email: None,
        membership_type: None,
        cursor_models_percent: None,
        other_models_percent: None,
        total_percent: None,
        plan_used_usd: None,
        plan_limit_usd: None,
        plan_remaining_usd: None,
        on_demand_used_usd: None,
        on_demand_limit_usd: None,
        on_demand_remaining_usd: None,
        requests_used: None,
        requests_limit: None,
        billing_cycle_end: None,
        fetched_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

async fn query_cursor_quota(
    client: &HttpClient,
    session_token: &str,
    local_user_id: &str,
) -> CursorQuotaSnapshot {
    let fetched_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let account =
        match fetch_cursor_json(client, "https://cursor.com/api/auth/me", session_token).await {
            Ok(value) => value,
            Err(error) => return cursor_fetch_error_snapshot(&error, "同步 Cursor 当前账户失败"),
        };
    if account
        .get("email")
        .and_then(Value::as_str)
        .is_none_or(|email| email.trim().is_empty())
    {
        return cursor_quota_status(
            "unavailable",
            "Cursor 未返回当前账户信息，已跳过额度同步；请在 Cursor 桌面端确认登录状态。",
        );
    }
    if account
        .get("sub")
        .and_then(Value::as_str)
        .and_then(canonical_cursor_user_id)
        .is_some_and(|account_id| account_id != local_user_id)
    {
        return cursor_quota_status(
            "unavailable",
            "Cursor 本机登录账号与在线账号不一致，已跳过额度同步；请重启 Cursor 后重试。",
        );
    }
    let summary = match fetch_cursor_json(
        client,
        "https://cursor.com/api/usage-summary",
        session_token,
    )
    .await
    {
        Ok(value) => value,
        Err(error) => return cursor_fetch_error_snapshot(&error, "同步 Cursor 额度失败"),
    };
    let request_usage = if let Some(user_id) = account.get("sub").and_then(Value::as_str) {
        match reqwest::Url::parse("https://cursor.com/api/usage") {
            Ok(mut url) => {
                url.query_pairs_mut().append_pair("user", user_id);
                fetch_cursor_json(client, url.as_str(), session_token)
                    .await
                    .ok()
            }
            Err(_) => None,
        }
    } else {
        None
    };
    parse_cursor_usage_summary(&summary, &account, request_usage.as_ref(), fetched_at)
}

#[tauri::command]
async fn get_cursor_quota(state: State<'_, AppState>) -> Result<CursorQuotaSnapshot, String> {
    let network = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        NetworkSettings {
            mode: data.proxy_mode.clone(),
            address: data.proxy_address.clone(),
        }
    };
    let local_session = tauri::async_runtime::spawn_blocking(resolve_current_cursor_session)
        .await
        .map_err(|_| "读取 Cursor 登录状态的后台线程异常。".to_owned())?;
    let (local_user_id, session_token) = match local_session {
        Ok(Some(session)) => session,
        Ok(None) => {
            return Ok(cursor_quota_status(
                "notConfigured",
                "未检测到 Cursor 当前本机登录账户；请先在 Cursor 桌面端登录。",
            ))
        }
        Err(error) => return Ok(cursor_quota_status("unavailable", &error)),
    };
    let client = match cursor_http_client(&network) {
        Ok(client) => client,
        Err(error) => return Ok(cursor_quota_status("unavailable", &error)),
    };
    let snapshot = query_cursor_quota(&client, &session_token, &local_user_id).await;
    if snapshot.status == "connected" {
        update_state(&state, |data| {
            data.last_cursor_quota = Some(snapshot.clone())
        })?;
    }
    Ok(snapshot)
}

fn cursor_tokscale_executable(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        candidates.push(resource_dir.join("tokscale.exe"));
        candidates.push(resource_dir.join("resources").join("tokscale.exe"));
    }
    if let Ok(executable) = env::current_exe() {
        if let Some(parent) = executable.parent() {
            candidates.push(parent.join("tokscale.exe"));
        }
    }
    candidates.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("node_modules")
            .join("@tokscale")
            .join("cli-win32-x64-msvc")
            .join("bin")
            .join("tokscale.exe"),
    );
    candidates.into_iter().find(|path| path.is_file())
}

fn run_tokscale_cursor_sync(app: &AppHandle, network: &NetworkSettings) -> Result<(), String> {
    let executable = cursor_tokscale_executable(app)
        .ok_or_else(|| "Tokscale 组件缺失，请重新安装本应用。".to_owned())?;
    let mut command = Command::new(executable);
    command
        .args(["cursor", "sync", "--json"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_proxy(&mut command, network)
        .map_err(|_| "Cursor Token 同步失败，请检查代理设置。".to_owned())?;
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command
        .spawn()
        .map_err(|_| "无法启动 Cursor Token 同步组件。请重新安装应用后重试。".to_owned())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 Cursor Token 同步结果。".to_owned())?;
    let stdout_reader = thread::spawn(move || {
        let mut stdout = stdout;
        let mut captured = Vec::new();
        let mut buffer = [0u8; 8192];
        loop {
            let Ok(read) = stdout.read(&mut buffer) else {
                break;
            };
            if read == 0 {
                break;
            }
            let remaining = (1024 * 1024usize).saturating_sub(captured.len());
            captured.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        String::from_utf8_lossy(&captured).into_owned()
    });
    let deadline = Instant::now() + CURSOR_TOKEN_SYNC_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = stdout_reader.join().unwrap_or_default();
                if !status.success() {
                    return Err(
                        "Cursor Token 同步失败；请确认 Cursor 账号仍处于登录状态。".to_owned()
                    );
                }
                let result = serde_json::from_str::<Value>(&output)
                    .map_err(|_| "Tokscale 返回了无法识别的 Cursor 同步结果。".to_owned())?;
                if result.get("synced").and_then(Value::as_bool) == Some(true) {
                    return Ok(());
                }
                let error = result
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                if error.contains("not authenticated") || error.contains("unauthorized") {
                    return Err(
                        "Tokscale 无法通过 Cursor 账号认证；请在 Cursor 桌面端重新登录。"
                            .to_owned(),
                    );
                }
                return Err("Tokscale 当前没有返回可同步的 Cursor 用量事件。".to_owned());
            }
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(100)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                return Err("Cursor Token 同步超时；稍后可重试。".to_owned());
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout_reader.join();
                return Err("读取 Cursor Token 同步进程状态失败。".to_owned());
            }
        }
    }
}

fn claim_cursor_token_sync() -> bool {
    let Ok(mut last_attempt) = CURSOR_LAST_SYNC_ATTEMPT.lock() else {
        return false;
    };
    if last_attempt.is_some_and(|instant| instant.elapsed() < CURSOR_TOKEN_SYNC_INTERVAL) {
        return false;
    }
    *last_attempt = Some(Instant::now());
    true
}

fn cursor_event_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if let Some(timestamp) = value.as_i64() {
        return Some(if timestamp > 100_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        });
    }
    let text = value.as_str()?;
    if let Ok(timestamp) = text.parse::<i64>() {
        return Some(if timestamp > 100_000_000_000 {
            timestamp / 1000
        } else {
            timestamp
        });
    }
    time::OffsetDateTime::parse(text, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp())
}

fn cursor_token_number(value: Option<&Value>) -> u64 {
    value
        .and_then(|value| value.as_u64().or_else(|| value.as_str()?.parse().ok()))
        .unwrap_or(0)
}

fn cursor_usage_cache_files(home: &Path) -> (Vec<PathBuf>, Option<u64>) {
    let cache_dir = home.join(".config").join("tokscale").join("cursor-cache");
    let mut files = Vec::new();
    let mut newest_updated_at = None;
    for directory in [cache_dir.clone(), cache_dir.join("archive")] {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !name.starts_with("usage") || !name.ends_with(".json") || !path.is_file() {
                continue;
            }
            if directory == cache_dir {
                if let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) {
                    let timestamp = modified
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    newest_updated_at = Some(
                        newest_updated_at.map_or(timestamp, |latest: u64| latest.max(timestamp)),
                    );
                }
            }
            files.push(path);
        }
    }
    files.sort();
    (files, newest_updated_at)
}

fn read_cursor_token_usage_stats(
    home: &Path,
    today_start: i64,
    tomorrow_start: i64,
    sync_performed: bool,
    sync_error: Option<String>,
) -> CursorTokenUsageStats {
    let (files, updated_at) = cursor_usage_cache_files(home);
    let mut total_tokens = 0u64;
    let mut today_tokens = 0u64;
    let mut events_scanned = 0u64;
    let mut cache_readable = false;
    for path in &files {
        let Ok(bytes) = fs::read(path) else { continue };
        let Ok(cache) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(events) = cache.get("usageEventsDisplay").and_then(Value::as_array) else {
            continue;
        };
        cache_readable = true;
        for event in events {
            let Some(timestamp) = cursor_event_timestamp(event.get("timestamp")) else {
                continue;
            };
            if event
                .get("model")
                .and_then(Value::as_str)
                .is_none_or(str::is_empty)
            {
                continue;
            }
            let usage = event.get("tokenUsage").unwrap_or(&Value::Null);
            let event_tokens = [
                "inputTokens",
                "outputTokens",
                "cacheReadTokens",
                "cacheWriteTokens",
            ]
            .into_iter()
            .map(|key| cursor_token_number(usage.get(key)))
            .fold(0u64, u64::saturating_add);
            total_tokens = total_tokens.saturating_add(event_tokens);
            events_scanned = events_scanned.saturating_add(1);
            if timestamp >= today_start && timestamp < tomorrow_start {
                today_tokens = today_tokens.saturating_add(event_tokens);
            }
        }
    }
    CursorTokenUsageStats {
        total_tokens,
        today_tokens,
        events_scanned,
        cache_available: cache_readable,
        sync_performed,
        sync_error,
        updated_at,
    }
}

fn sync_and_read_cursor_tokens(
    app: &AppHandle,
    network: &NetworkSettings,
    today_start: i64,
    tomorrow_start: i64,
) -> CursorTokenUsageStats {
    let Some(home) = user_home_directory() else {
        return CursorTokenUsageStats {
            total_tokens: 0,
            today_tokens: 0,
            events_scanned: 0,
            cache_available: false,
            sync_performed: false,
            sync_error: Some("无法定位 Windows 用户目录。".to_owned()),
            updated_at: None,
        };
    };
    let mut sync_performed = false;
    let mut sync_error = None;
    let (_, newest_cache_at) = cursor_usage_cache_files(&home);
    let cache_recently_synced = newest_cache_at.is_some_and(|timestamp| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(timestamp)
            < CURSOR_TOKEN_SYNC_INTERVAL.as_secs()
    });
    match resolve_cursor_session_token() {
        Ok(Some(_)) => {
            if !cache_recently_synced && claim_cursor_token_sync() {
                sync_performed = true;
                sync_error = run_tokscale_cursor_sync(app, network).err();
            }
        }
        Ok(None) => {
            sync_error = Some("未检测到 Cursor 登录状态；请先在 Cursor 桌面端登录。".to_owned())
        }
        Err(error) => sync_error = Some(error),
    }
    read_cursor_token_usage_stats(
        &home,
        today_start,
        tomorrow_start,
        sync_performed,
        sync_error,
    )
}

#[tauri::command]
async fn get_cursor_token_usage_stats(
    app: AppHandle,
    state: State<'_, AppState>,
    today_start: i64,
    tomorrow_start: i64,
) -> Result<CursorTokenUsageStats, String> {
    let network = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        NetworkSettings {
            mode: data.proxy_mode.clone(),
            address: data.proxy_address.clone(),
        }
    };
    tauri::async_runtime::spawn_blocking(move || {
        sync_and_read_cursor_tokens(&app, &network, today_start, tomorrow_start)
    })
    .await
    .map_err(|_| "读取 Cursor Token 统计的后台线程异常。".to_owned())
}

fn query_codex(
    launcher: CodexLauncher,
    network: &NetworkSettings,
) -> Result<(UsageSnapshot, PathBuf), String> {
    let resolved = launcher.source_path.clone();
    let mut command = Command::new(&launcher.program);
    command
        .args(&launcher.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    configure_proxy(&mut command, network)?;
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 Codex（{}）：{error}", launcher.program.display()))?;
    let mut stdin = child.stdin.take().ok_or("无法连接 Codex 输入流")?;
    let stdout = child.stdout.take().ok_or("无法连接 Codex 输出流")?;
    let (sender, receiver) = mpsc::channel::<Value>();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(message) = serde_json::from_str::<Value>(&line) {
                if sender.send(message).is_err() {
                    break;
                }
            }
        }
    });
    for message in [
        json!({"method":"initialize","id":1,"params":{"clientInfo":{"name":"codex_quota","title":"Codex 额度","version":env!("CARGO_PKG_VERSION")}}}),
        json!({"method":"initialized","params":{}}),
        json!({"method":"account/rateLimits/read","id":2,"params":{}}),
        json!({"method":"account/read","id":3,"params":{"refreshToken":false}}),
    ] {
        writeln!(stdin, "{message}").map_err(|error| format!("向 Codex 发送请求失败：{error}"))?;
    }
    let (mut rate, mut account) = (None, None);
    let deadline = SystemTime::now() + Duration::from_secs(30);
    while SystemTime::now() < deadline && (rate.is_none() || account.is_none()) {
        match receiver.recv_timeout(Duration::from_millis(500)) {
            Ok(message) => match message.get("id").and_then(Value::as_i64) {
                Some(2) => rate = Some(message),
                Some(3) => account = Some(message),
                _ => {}
            },
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        }
    }
    drop(stdin);
    let _ = child.kill();
    let fetched_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let rate =
        rate.ok_or("Codex App Server 没有返回额度数据。请确认 Codex 已更新到最新版本并已登录。")?;
    Ok((
        parse_usage_responses(&rate, account.as_ref(), fetched_at)?,
        resolved,
    ))
}

fn read_codex_usage(
    cached: Option<&Path>,
    network: NetworkSettings,
) -> Result<(UsageSnapshot, PathBuf), String> {
    let launcher = find_codex_launcher(cached)?;
    let attempted_path = launcher.source_path.clone();
    match query_codex(launcher, &network) {
        Ok(result) => Ok(result),
        Err(cached_error) if cached.is_some_and(|path| path == attempted_path) => {
            // The cached executable can still exist after Codex updates while no longer
            // being the correct app-server. Rescan without it and persist the replacement
            // after the retry succeeds.
            let fresh_launcher = find_codex_launcher(None)?;
            if fresh_launcher.source_path == attempted_path {
                return Err(cached_error);
            }
            query_codex(fresh_launcher, &network).map_err(|fresh_error| {
                format!("缓存的 Codex 路径已失效，自动重新检索后仍无法读取额度：{fresh_error}")
            })
        }
        Err(error) => Err(error),
    }
}

#[tauri::command]
async fn get_codex_usage(state: State<'_, AppState>) -> Result<UsageSnapshot, String> {
    let (cached, network) = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        (
            data.codex_path.clone(),
            NetworkSettings {
                mode: data.proxy_mode.clone(),
                address: data.proxy_address.clone(),
            },
        )
    };
    let (usage, path) =
        tauri::async_runtime::spawn_blocking(move || read_codex_usage(cached.as_deref(), network))
            .await
            .map_err(|error| format!("额度后台线程异常：{error}"))??;
    update_state(&state, |data| {
        data.codex_path = Some(path);
        data.last_usage = Some(usage.clone());
    })?;
    Ok(usage)
}

#[tauri::command]
fn get_cached_usage(state: State<'_, AppState>) -> Result<Option<UsageSnapshot>, String> {
    Ok(state
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?
        .last_usage
        .clone())
}

#[tauri::command]
fn get_cached_cursor_quota(
    state: State<'_, AppState>,
) -> Result<Option<CursorQuotaSnapshot>, String> {
    Ok(state
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?
        .last_cursor_quota
        .clone())
}

fn codex_home_directory() -> Option<PathBuf> {
    env::var_os("CODEX_HOME").map(PathBuf::from).or_else(|| {
        env::var_os("USERPROFILE")
            .or_else(|| env::var_os("HOME"))
            .map(PathBuf::from)
            .map(|home| home.join(".codex"))
    })
}

fn read_session_log(
    path: &Path,
    today_start: i64,
    tomorrow_start: i64,
    stats: &mut TokenUsageStats,
) {
    stats.sessions_scanned = stats.sessions_scanned.saturating_add(1);
    let Ok(file) = File::open(path) else {
        stats.unreadable_sessions = stats.unreadable_sessions.saturating_add(1);
        return;
    };

    let mut session_usage = SessionTokenUsage::default();
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if let Ok(record) = serde_json::from_str::<Value>(&line) {
                    add_token_usage_record(
                        &record,
                        today_start,
                        tomorrow_start,
                        &mut session_usage,
                    );
                }
            }
            Err(_) => {
                stats.unreadable_sessions = stats.unreadable_sessions.saturating_add(1);
                break;
            }
        }
    }

    stats.total_tokens = stats
        .total_tokens
        .saturating_add(if session_usage.has_total_usage {
            session_usage.total_tokens
        } else {
            session_usage.fallback_total_tokens
        });
    stats.today_tokens = stats
        .today_tokens
        .saturating_add(session_usage.today_tokens);
}

fn collect_session_logs(
    directory: &Path,
    today_start: i64,
    tomorrow_start: i64,
    stats: &mut TokenUsageStats,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_session_logs(&path, today_start, tomorrow_start, stats);
        } else if file_type.is_file()
            && path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jsonl"))
        {
            read_session_log(&path, today_start, tomorrow_start, stats);
        }
    }
}

fn token_usage_number(value: Option<&Value>) -> Option<u64> {
    value?.get("total_tokens")?.as_u64()
}

fn parse_session_timestamp(value: Option<&Value>) -> Option<i64> {
    let timestamp = value?.get("timestamp")?.as_str()?;
    time::OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp())
}

fn add_token_usage_record(
    record: &Value,
    today_start: i64,
    tomorrow_start: i64,
    usage: &mut SessionTokenUsage,
) {
    if record.get("type").and_then(Value::as_str) != Some("event_msg")
        || record.pointer("/payload/type").and_then(Value::as_str) != Some("token_count")
    {
        return;
    }

    let info = record.pointer("/payload/info");
    let total_tokens = token_usage_number(info.and_then(|info| info.get("total_token_usage")));
    let last_tokens = token_usage_number(info.and_then(|info| info.get("last_token_usage")));

    if let Some(total_tokens) = total_tokens {
        usage.has_total_usage = true;
        usage.total_tokens = usage.total_tokens.max(total_tokens);
    } else if let Some(last_tokens) = last_tokens {
        usage.fallback_total_tokens = usage.fallback_total_tokens.saturating_add(last_tokens);
    }

    if parse_session_timestamp(Some(record))
        .is_some_and(|timestamp| timestamp >= today_start && timestamp < tomorrow_start)
    {
        if let Some(last_tokens) = last_tokens {
            usage.today_tokens = usage.today_tokens.saturating_add(last_tokens);
        }
    }
}

fn empty_token_usage_stats(available: bool) -> TokenUsageStats {
    TokenUsageStats {
        total_tokens: 0,
        today_tokens: 0,
        sessions_scanned: 0,
        unreadable_sessions: 0,
        available,
        updated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    }
}

fn read_token_usage_stats(
    codex_home: &Path,
    today_start: i64,
    tomorrow_start: i64,
) -> TokenUsageStats {
    let sessions_directory = codex_home.join("sessions");
    if !sessions_directory.is_dir() {
        return empty_token_usage_stats(false);
    }

    let mut stats = empty_token_usage_stats(true);
    collect_session_logs(&sessions_directory, today_start, tomorrow_start, &mut stats);

    stats.updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    stats
}

#[tauri::command]
async fn get_token_usage_stats(
    today_start: i64,
    tomorrow_start: i64,
) -> Result<TokenUsageStats, String> {
    if tomorrow_start <= today_start {
        return Err("Token 统计的本地日期范围无效".to_owned());
    }

    let Some(codex_home) = codex_home_directory() else {
        return Ok(empty_token_usage_stats(false));
    };
    tauri::async_runtime::spawn_blocking(move || {
        read_token_usage_stats(&codex_home, today_start, tomorrow_start)
    })
    .await
    .map_err(|error| format!("读取本机 Token 历史失败：{error}"))
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LaunchAction {
    ShowMain,
    ToggleFloating,
    TogglePin,
    Quit,
}

fn launch_action(args: &[String]) -> LaunchAction {
    if args.iter().any(|value| value == "--toggle-floating") {
        LaunchAction::ToggleFloating
    } else if args.iter().any(|value| value == "--toggle-pin") {
        LaunchAction::TogglePin
    } else if args.iter().any(|value| value == "--quit") {
        LaunchAction::Quit
    } else {
        LaunchAction::ShowMain
    }
}

fn handle_launch_action(app: &AppHandle, args: &[String]) {
    match launch_action(args) {
        LaunchAction::ShowMain => show_main_window(app),
        LaunchAction::ToggleFloating => {
            let _ = toggle_floating(app);
        }
        LaunchAction::TogglePin => {
            let _ = toggle_pin(app);
        }
        LaunchAction::Quit => app.exit(0),
    }
}

fn set_floating_visible(app: &AppHandle, visible: bool) -> Result<bool, String> {
    let (style, pinned, always_on_top) = app
        .state::<AppState>()
        .data
        .lock()
        .map(|data| {
            (
                data.floating_style.clone(),
                data.floating_pinned,
                data.floating_always_on_top,
            )
        })
        .map_err(|_| "本地状态暂时不可用".to_owned())?;
    let selected_label = floating_window_label(&style);
    for label in ["compact", "orb"] {
        if let Some(window) = app.get_webview_window(label) {
            if !visible || label != selected_label {
                let _ = window.hide();
            }
        }
    }
    if visible {
        let window = app
            .get_webview_window(selected_label)
            .ok_or("悬浮窗尚未创建")?;
        window
            .set_always_on_top(always_on_top)
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        if !pinned {
            window.set_focus().map_err(|error| error.to_string())?;
        }
    }
    update_state(&app.state::<AppState>(), |data| {
        data.floating_visible = visible
    })?;
    if let Ok(settings) = get_floating_settings(app.state::<AppState>()) {
        let _ = app.emit("floating-settings-changed", settings);
    }
    Ok(visible)
}

fn toggle_floating(app: &AppHandle) -> Result<bool, String> {
    let visible = app
        .state::<AppState>()
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?
        .floating_visible;
    set_floating_visible(app, !visible)
}

fn floating_window_label(style: &str) -> &'static str {
    if style == "orb" {
        "orb"
    } else {
        "compact"
    }
}

fn is_floating_pin_hit(
    cursor_x: i32,
    cursor_y: i32,
    left: i32,
    top: i32,
    width: i32,
    height: i32,
) -> bool {
    let scale_x = width as f64 / 308.0;
    let scale_y = height as f64 / 174.0;
    let button_left = left + (210.0 * scale_x).round() as i32;
    let button_right = left + (252.0 * scale_x).round() as i32;
    let button_bottom = top + (38.0 * scale_y).round() as i32;
    cursor_x >= button_left
        && cursor_x <= button_right
        && cursor_y >= top
        && cursor_y <= button_bottom
}

#[cfg(target_os = "windows")]
fn start_click_through_controller(
    window: WebviewWindow,
    pinned: Arc<AtomicBool>,
    orb_mode: Arc<AtomicBool>,
) {
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        UI::WindowsAndMessaging::{GetCursorPos, GetWindowRect},
    };

    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };
    let hwnd_value = raw_hwnd.0 as isize;
    thread::spawn(move || {
        let hwnd = hwnd_value as windows_sys::Win32::Foundation::HWND;
        let mut last_passthrough = false;
        loop {
            let mut point = POINT { x: 0, y: 0 };
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let valid =
                unsafe { GetCursorPos(&mut point) != 0 && GetWindowRect(hwnd, &mut rect) != 0 };
            if !valid {
                break;
            }

            let over_pin = is_floating_pin_hit(
                point.x,
                point.y,
                rect.left,
                rect.top,
                rect.right - rect.left,
                rect.bottom - rect.top,
            );
            let passthrough =
                pinned.load(Ordering::Relaxed) && !orb_mode.load(Ordering::Relaxed) && !over_pin;
            if passthrough != last_passthrough {
                let _ = window.set_ignore_cursor_events(passthrough);
                last_passthrough = passthrough;
            }
            thread::sleep(Duration::from_millis(20));
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn start_click_through_controller(_: WebviewWindow, _: Arc<AtomicBool>, _: Arc<AtomicBool>) {}

#[cfg(target_os = "windows")]
fn start_orb_click_through_controller(
    window: WebviewWindow,
    orb_mode: Arc<AtomicBool>,
    expanded: Arc<AtomicBool>,
    orb_size: Arc<AtomicU32>,
) {
    use windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
        },
        UI::{
            HiDpi::GetDpiForWindow,
            WindowsAndMessaging::{GetCursorPos, GetWindowRect, IsWindowVisible},
        },
    };

    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };
    let hwnd_value = raw_hwnd.0 as isize;
    let app = window.app_handle().clone();
    thread::spawn(move || {
        let hwnd = hwnd_value as windows_sys::Win32::Foundation::HWND;
        let mut last_passthrough = false;
        let mut last_over_orb = false;
        loop {
            let state = app.state::<AppState>();
            let is_active = orb_mode.load(Ordering::Relaxed)
                && unsafe { IsWindowVisible(hwnd) != 0 }
                && !state.floating_orb_dragging.load(Ordering::Relaxed);
            if !is_active {
                last_over_orb = false;
                if last_passthrough {
                    let _ = window.set_ignore_cursor_events(false);
                    last_passthrough = false;
                }
                thread::sleep(Duration::from_millis(100));
                continue;
            }

            let mut point = POINT { x: 0, y: 0 };
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            let valid =
                unsafe { GetCursorPos(&mut point) != 0 && GetWindowRect(hwnd, &mut rect) != 0 };
            if !valid {
                break;
            }

            let mut passthrough = false;
            let mut over_orb = false;
            if !expanded.load(Ordering::Relaxed) {
                let monitor_handle = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
                let mut monitor_info: MONITORINFO = unsafe { std::mem::zeroed() };
                monitor_info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
                let has_monitor = !monitor_handle.is_null()
                    && unsafe { GetMonitorInfoW(monitor_handle, &mut monitor_info) != 0 };
                let dpi = unsafe { GetDpiForWindow(hwnd) };
                let scale = if dpi == 0 { 1.0 } else { dpi as f64 / 96.0 };
                let monitor_x = if has_monitor {
                    monitor_info.rcMonitor.left
                } else {
                    rect.left
                };
                let monitor_width = if has_monitor {
                    (monitor_info.rcMonitor.right - monitor_info.rcMonitor.left).max(0) as u32
                } else {
                    (rect.right - rect.left).max(0) as u32
                };
                let side = state
                    .data
                    .lock()
                    .ok()
                    .map(|data| {
                        if let Some(anchor) = data.floating_orb_position {
                            let collapsed_width =
                                (floating_orb_shell_size(orb_size.load(Ordering::Relaxed)) * scale)
                                    .round() as u32;
                            orb_layout_side_from_metrics(
                                &data.floating_orb_expand_direction,
                                anchor.x,
                                collapsed_width,
                                monitor_x,
                                monitor_width,
                                scale,
                            )
                        } else {
                            edge_side(
                                rect.left,
                                (rect.right - rect.left).max(0) as u32,
                                monitor_x,
                                monitor_width,
                            )
                        }
                    })
                    .unwrap_or("right");
                let diameter = (orb_size.load(Ordering::Relaxed) as f64 * scale).round() as i32;
                let inset = (FLOATING_ORB_VISUAL_INSET * scale).round() as i32;
                over_orb = is_pointer_inside_orb(
                    point.x,
                    point.y,
                    rect.left,
                    rect.top,
                    rect.right,
                    rect.bottom,
                    diameter,
                    inset,
                    side,
                );
                passthrough = !over_orb;
                if over_orb && !last_over_orb {
                    let _ = app.emit("floating-orb-hover-entered", ());
                }
            }
            last_over_orb = over_orb;

            if passthrough != last_passthrough {
                let _ = window.set_ignore_cursor_events(passthrough);
                last_passthrough = passthrough;
            }
            // Pointer-hit testing is visual interaction, so a 60 Hz poll is
            // sufficient and avoids starving the application's UI loop.
            thread::sleep(Duration::from_millis(16));
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn start_orb_click_through_controller(
    _: WebviewWindow,
    _: Arc<AtomicBool>,
    _: Arc<AtomicBool>,
    _: Arc<AtomicU32>,
) {
}

#[tauri::command]
fn set_floating_window(app: AppHandle, visible: bool) -> Result<bool, String> {
    set_floating_visible(&app, visible)
}

#[tauri::command]
fn set_active_platform(
    app: AppHandle,
    state: State<'_, AppState>,
    platform: String,
) -> Result<String, String> {
    if !matches!(platform.as_str(), "codex" | "cursor") {
        return Err("统计平台无效".to_owned());
    }
    update_state(&state, |data| data.active_platform = platform.clone())?;
    let settings = get_floating_settings(state)?;
    let _ = app.emit("floating-settings-changed", settings);
    Ok(platform)
}

#[tauri::command]
fn get_floating_settings(state: State<'_, AppState>) -> Result<FloatingSettings, String> {
    let data = state
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?;
    Ok(FloatingSettings {
        active_platform: data.active_platform.clone(),
        visible: data.floating_visible,
        pinned: data.floating_pinned,
        opacity: data.floating_opacity,
        always_on_top: data.floating_always_on_top,
        style: data.floating_style.clone(),
        orb_expand_direction: data.floating_orb_expand_direction.clone(),
        orb_size: data.floating_orb_size,
        card_scale: data.floating_card_scale,
        orb_wave_speed: data.orb_wave_speed,
        orb_wave_amplitude: data.orb_wave_amplitude,
        sync_interval_secs: data.sync_interval_secs,
        display_mode: data.display_mode.clone(),
        theme: data.theme.clone(),
        proxy_mode: data.proxy_mode.clone(),
        proxy_address: data.proxy_address.clone(),
        data_directory: state
            .file_path
            .parent()
            .unwrap_or(Path::new("data"))
            .display()
            .to_string(),
    })
}

fn floating_orb_shell_size(orb_size: u32) -> f64 {
    orb_size as f64 + 2.0 * FLOATING_ORB_VISUAL_INSET
}

fn floating_logical_size(
    style: &str,
    expanded: bool,
    orb_size: u32,
    card_scale: u32,
) -> LogicalSize<f64> {
    if style == "orb" {
        let height = floating_orb_shell_size(orb_size);
        let width = if expanded {
            height + FLOATING_ORB_PANEL_WIDTH
        } else {
            height
        };
        LogicalSize::new(width, height)
    } else {
        let scale = card_scale as f64 / 100.0;
        LogicalSize::new(FLOATING_CARD_WIDTH * scale, FLOATING_CARD_HEIGHT * scale)
    }
}

fn orb_size_for_window(window: &WebviewWindow) -> u32 {
    window
        .app_handle()
        .state::<AppState>()
        .floating_orb_size
        .load(Ordering::Relaxed)
}

fn card_scale_for_window(window: &WebviewWindow) -> u32 {
    window
        .app_handle()
        .state::<AppState>()
        .floating_card_scale
        .load(Ordering::Relaxed)
}

fn edge_side(
    position_x: i32,
    window_width: u32,
    monitor_left: i32,
    monitor_width: u32,
) -> &'static str {
    let window_center = position_x as i64 + window_width as i64 / 2;
    let monitor_center = monitor_left as i64 + monitor_width as i64 / 2;
    if window_center <= monitor_center {
        "left"
    } else {
        "right"
    }
}

fn clamp_vertical(y: i32, height: u32, monitor_top: i32, monitor_height: u32, margin: i32) -> i32 {
    let minimum = monitor_top + margin;
    let maximum = monitor_top + monitor_height as i32 - height as i32 - margin;
    y.clamp(minimum, maximum.max(minimum))
}

fn normalize_orb_expand_direction(direction: &str) -> Option<&'static str> {
    match direction {
        "auto" => Some("auto"),
        "left" => Some("left"),
        "right" => Some("right"),
        _ => None,
    }
}

fn orb_side_for_direction(direction: &str, automatic_side: &'static str) -> &'static str {
    match direction {
        // The panel opens to the left, so the orb stays on the right side.
        "left" => "right",
        // The panel opens to the right, so the orb stays on the left side.
        "right" => "left",
        _ => automatic_side,
    }
}

fn orb_window_x(orb_left: i32, orb_width: i32, margin: i32, side: &str, window_width: u32) -> i32 {
    if side == "right" {
        orb_left + orb_width + margin - window_width as i32
    } else {
        orb_left - margin
    }
}

fn orb_anchor_x_from_window(window_x: i32, window_width: u32, orb_size: i32, side: &str) -> i32 {
    if side == "right" {
        window_x + window_width as i32 - orb_size
    } else {
        window_x
    }
}

fn orb_anchor_dock_edges(
    monitor_x: i32,
    monitor_width: u32,
    collapsed_width: u32,
    visual_inset: i32,
) -> (i32, i32) {
    (
        monitor_x - visual_inset,
        monitor_x + monitor_width as i32 - collapsed_width as i32 + visual_inset,
    )
}

fn migrate_legacy_orb_dock_anchor(
    anchor_x: i32,
    legacy_left: i32,
    legacy_right: i32,
    dock_left: i32,
    dock_right: i32,
    scale_factor: f64,
) -> Option<i32> {
    let tolerance = (ORB_EDGE_DOCK_TOLERANCE * scale_factor).round() as i32;
    if (anchor_x - legacy_left).abs() <= tolerance {
        Some(dock_left)
    } else if (anchor_x - legacy_right).abs() <= tolerance {
        Some(dock_right)
    } else {
        None
    }
}

fn normalize_orb_dock_anchor(
    window: &WebviewWindow,
    mut anchor: SavedPosition,
) -> Result<SavedPosition, String> {
    let monitor = monitor_for_orb_anchor(window, anchor)?;
    let scale = monitor.scale_factor();
    let collapsed_size = floating_orb_shell_size(orb_size_for_window(window));
    let visual_inset = (FLOATING_ORB_VISUAL_INSET * scale).round() as i32;
    let collapsed_width = (collapsed_size * scale).round() as u32;
    let (dock_left, dock_right) = orb_anchor_dock_edges(
        monitor.position().x,
        monitor.size().width,
        collapsed_width,
        visual_inset,
    );

    // Older releases persisted a six-pixel window margin, leaving the orb
    // itself twelve pixels away from the display edge.
    let legacy_left = monitor.position().x + visual_inset;
    let legacy_right =
        monitor.position().x + monitor.size().width as i32 - collapsed_width as i32 - visual_inset;
    if let Some(x) = migrate_legacy_orb_dock_anchor(
        anchor.x,
        legacy_left,
        legacy_right,
        dock_left,
        dock_right,
        scale,
    ) {
        anchor.x = x;
    }
    Ok(anchor)
}

fn monitor_for_orb_anchor(
    window: &WebviewWindow,
    anchor: SavedPosition,
) -> Result<Monitor, String> {
    let monitors = window
        .available_monitors()
        .map_err(|error| error.to_string())?;
    if let Some(monitor) = monitors.into_iter().find(|monitor| {
        let half_orb =
            (floating_orb_shell_size(orb_size_for_window(window)) * monitor.scale_factor() / 2.0)
                .round() as i32;
        let center_x = anchor.x + half_orb;
        let center_y = anchor.y + half_orb;
        center_x >= monitor.position().x
            && center_x < monitor.position().x + monitor.size().width as i32
            && center_y >= monitor.position().y
            && center_y < monitor.position().y + monitor.size().height as i32
    }) {
        return Ok(monitor);
    }
    window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or("找不到当前显示器".to_owned())
}

fn orb_layout_side(window: &WebviewWindow, direction: &str) -> Result<&'static str, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or("找不到当前显示器")?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    Ok(orb_side_for_direction(
        direction,
        edge_side(
            position.x,
            size.width,
            monitor.position().x,
            monitor.size().width,
        ),
    ))
}

fn orb_layout_side_for_anchor(
    window: &WebviewWindow,
    direction: &str,
    anchor: SavedPosition,
) -> Result<&'static str, String> {
    let monitor = monitor_for_orb_anchor(window, anchor)?;
    let scale = monitor.scale_factor();
    let collapsed_width =
        (floating_orb_shell_size(orb_size_for_window(window)) * scale).round() as u32;
    Ok(orb_layout_side_from_metrics(
        direction,
        anchor.x,
        collapsed_width,
        monitor.position().x,
        monitor.size().width,
        scale,
    ))
}

fn orb_layout_side_from_metrics(
    direction: &str,
    anchor_x: i32,
    collapsed_width: u32,
    monitor_x: i32,
    monitor_width: u32,
    scale: f64,
) -> &'static str {
    let margin = (6.0 * scale).round() as i32;
    let left = monitor_x + margin;
    let right = monitor_x + monitor_width as i32 - collapsed_width as i32 - margin;
    let threshold = (28.0 * scale).round() as i32;
    if (anchor_x - left).abs() <= threshold {
        return "left";
    }
    if (anchor_x - right).abs() <= threshold {
        return "right";
    }
    orb_side_for_direction(
        direction,
        edge_side(anchor_x, collapsed_width, monitor_x, monitor_width),
    )
}

fn orb_anchor_from_window(
    window: &WebviewWindow,
    state: &AppState,
    position: SavedPosition,
) -> Option<SavedPosition> {
    let current_monitor = window
        .current_monitor()
        .ok()?
        .or(window.primary_monitor().ok()?)?;
    let size = window.outer_size().ok()?;
    let (direction, previous_anchor, orb_size) = {
        let data = state.data.lock().ok()?;
        (
            data.floating_orb_expand_direction.clone(),
            data.floating_orb_position,
            data.floating_orb_size,
        )
    };
    let collapsed_size = floating_orb_shell_size(orb_size);
    let collapsed_now = size.width
        <= (collapsed_size * current_monitor.scale_factor()).round() as u32
            + (12.0 * current_monitor.scale_factor()).round() as u32;
    let monitor = if collapsed_now {
        current_monitor
    } else {
        previous_anchor
            .and_then(|anchor| monitor_for_orb_anchor(window, anchor).ok())
            .unwrap_or(current_monitor)
    };
    let scale = monitor.scale_factor();
    let current_side = if collapsed_now {
        orb_layout_side(window, &direction).ok()?
    } else if let Some(previous_anchor) = previous_anchor {
        orb_layout_side_for_anchor(window, &direction, previous_anchor).ok()?
    } else {
        orb_layout_side(window, &direction).ok()?
    };
    let margin = (6.0 * scale).round() as i32;
    let orb_width = (orb_size as f64 * scale).round() as i32;
    let anchor_x =
        orb_anchor_x_from_window(position.x, size.width, orb_width + 2 * margin, current_side);
    Some(SavedPosition {
        x: anchor_x,
        y: position.y,
    })
}

fn orb_edge_anchor(
    window: &WebviewWindow,
    state: &AppState,
    position: SavedPosition,
) -> Option<(SavedPosition, &'static str)> {
    let anchor = orb_anchor_from_window(window, state, position)?;
    orb_edge_anchor_for_size(window, anchor, orb_size_for_window(window))
}

fn orb_edge_anchor_for_size(
    window: &WebviewWindow,
    anchor: SavedPosition,
    orb_size: u32,
) -> Option<(SavedPosition, &'static str)> {
    let monitor = monitor_for_orb_anchor(window, anchor).ok()?;
    let scale = monitor.scale_factor();
    let margin = (6.0 * scale).round() as i32;
    let collapsed_size = (floating_orb_shell_size(orb_size) * scale).round() as u32;
    let (left_x, right_x) = orb_anchor_dock_edges(
        monitor.position().x,
        monitor.size().width,
        collapsed_size,
        margin,
    );
    let physical_side = if (anchor.x - left_x).abs() <= (anchor.x - right_x).abs() {
        "left"
    } else {
        "right"
    };
    Some((
        SavedPosition {
            x: if physical_side == "left" {
                left_x
            } else {
                right_x
            },
            y: clamp_vertical(
                anchor.y,
                collapsed_size,
                monitor.position().y,
                monitor.size().height,
                margin,
            ),
        },
        physical_side,
    ))
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn start_floating_orb_drag(
    app: AppHandle,
    cursor_start_x: i32,
    cursor_start_y: i32,
) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::POINT,
        UI::{
            Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON},
            WindowsAndMessaging::{
                GetCursorPos, SetWindowPos, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
            },
        },
    };

    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    let state = app.state::<AppState>();
    let (direction, saved_anchor, orb_size) = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        (
            data.floating_orb_expand_direction.clone(),
            data.floating_orb_position,
            data.floating_orb_size,
        )
    };
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let current = SavedPosition {
        x: position.x,
        y: position.y,
    };
    let anchor = orb_anchor_from_window(&window, &state, current)
        .or(saved_anchor)
        .ok_or("无法计算悬浮球位置")?;
    if state.floating_orb_dragging.swap(true, Ordering::Relaxed) {
        return Ok(());
    }
    let cursor_start = POINT {
        x: cursor_start_x,
        y: cursor_start_y,
    };
    if let Err(error) = resize_orb_window_from_anchor(&window, false, &direction, anchor) {
        state.floating_orb_dragging.store(false, Ordering::Relaxed);
        return Err(error);
    }
    let drag_side = orb_layout_side_for_anchor(&window, &direction, anchor).unwrap_or("left");
    if let Err(error) = set_orb_window_region(&window, true, drag_side) {
        state.floating_orb_dragging.store(false, Ordering::Relaxed);
        return Err(error);
    }
    let window_start = match window.outer_position() {
        Ok(position) => position,
        Err(error) => {
            state.floating_orb_dragging.store(false, Ordering::Relaxed);
            return Err(error.to_string());
        }
    };
    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd.0 as isize,
        Err(error) => {
            state.floating_orb_dragging.store(false, Ordering::Relaxed);
            return Err(error.to_string());
        }
    };

    thread::spawn(move || {
        let mut moved = false;
        loop {
            let mut cursor = POINT { x: 0, y: 0 };
            if unsafe { GetCursorPos(&mut cursor) } != 0 {
                let delta_x = cursor.x - cursor_start.x;
                let delta_y = cursor.y - cursor_start.y;
                moved |= delta_x.abs() > 1 || delta_y.abs() > 1;
                unsafe {
                    SetWindowPos(
                        hwnd as _,
                        std::ptr::null_mut(),
                        window_start.x + delta_x,
                        window_start.y + delta_y,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
                    );
                }
            }
            if unsafe { GetAsyncKeyState(VK_LBUTTON as i32) } >= 0 {
                break;
            }
            thread::sleep(Duration::from_millis(8));
        }
        let state = app.state::<AppState>();
        let mut at_edge = false;
        let mut side = "right".to_owned();
        if let Ok(position) = window.outer_position() {
            let mut final_anchor = SavedPosition {
                x: position.x,
                y: position.y,
            };
            if let Ok(monitor) = monitor_for_orb_anchor(&window, final_anchor) {
                let scale = monitor.scale_factor();
                let size = (floating_orb_shell_size(orb_size) * scale).round() as i32;
                let visual_inset = (FLOATING_ORB_VISUAL_INSET * scale).round() as i32;
                let (left_x, right_x) = orb_anchor_dock_edges(
                    monitor.position().x,
                    monitor.size().width,
                    size as u32,
                    visual_inset,
                );
                final_anchor.x = final_anchor.x.clamp(left_x, right_x);
                final_anchor.y = clamp_vertical(
                    final_anchor.y,
                    size as u32,
                    monitor.position().y,
                    monitor.size().height,
                    visual_inset,
                );
                if final_anchor.x != position.x || final_anchor.y != position.y {
                    let _ =
                        window.set_position(PhysicalPosition::new(final_anchor.x, final_anchor.y));
                }
            }
            at_edge = orb_is_near_edge(&window, &state, final_anchor).unwrap_or(false);
            if at_edge {
                if let Some((snapped, _)) = orb_edge_anchor(&window, &state, final_anchor) {
                    final_anchor = snapped;
                    let _ = resize_orb_window_from_anchor(&window, false, &direction, final_anchor);
                }
            }
            side = orb_layout_side_for_anchor(&window, &direction, final_anchor)
                .unwrap_or("right")
                .to_owned();
            let _ = remember_floating_position_for_style(&state, "orb", final_anchor);
        }
        state.floating_orb_dragging.store(false, Ordering::Relaxed);
        let _ = app.emit(
            "floating-orb-drag-ended",
            OrbDragResult {
                moved,
                at_edge,
                side,
            },
        );
    });

    Ok(())
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct OrbDragResult {
    moved: bool,
    at_edge: bool,
    side: String,
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn start_floating_orb_drag(
    app: AppHandle,
    _cursor_start_x: i32,
    _cursor_start_y: i32,
) -> Result<(), String> {
    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    window.start_dragging().map_err(|error| error.to_string())
}

fn orb_is_near_edge(
    window: &WebviewWindow,
    state: &AppState,
    position: SavedPosition,
) -> Result<bool, String> {
    let anchor = orb_anchor_from_window(window, state, position).ok_or("无法读取悬浮球位置")?;
    orb_anchor_is_near_edge(window, anchor)
}

fn orb_anchor_is_near_edge(window: &WebviewWindow, anchor: SavedPosition) -> Result<bool, String> {
    let monitor = monitor_for_orb_anchor(window, anchor)?;
    let scale = monitor.scale_factor();
    let margin = (6.0 * scale).round() as i32;
    let collapsed_width =
        (floating_orb_shell_size(orb_size_for_window(window)) * scale).round() as u32;
    let (left_x, right_x) = orb_anchor_dock_edges(
        monitor.position().x,
        monitor.size().width,
        collapsed_width,
        margin,
    );
    Ok(orb_anchor_is_near_dock_edge(
        anchor.x, left_x, right_x, scale,
    ))
}

fn orb_anchor_is_near_dock_edge(
    anchor_x: i32,
    left_x: i32,
    right_x: i32,
    scale_factor: f64,
) -> bool {
    let tolerance = (ORB_EDGE_DOCK_TOLERANCE * scale_factor).round() as i32;
    (anchor_x - left_x).abs() <= tolerance || (anchor_x - right_x).abs() <= tolerance
}

fn is_pointer_inside_orb(
    cursor_x: i32,
    cursor_y: i32,
    window_left: i32,
    window_top: i32,
    window_right: i32,
    window_bottom: i32,
    orb_size: i32,
    inset: i32,
    side: &str,
) -> bool {
    if orb_size <= 0 || inset < 0 || window_right <= window_left || window_bottom <= window_top {
        return false;
    }
    let left = if side == "right" {
        window_right - inset - orb_size
    } else {
        window_left + inset
    };
    let center_x = left + orb_size / 2;
    let center_y = window_top + inset + orb_size / 2;
    let radius = (orb_size as f64 / 2.0 - 0.5).max(0.0);
    let dx = cursor_x as f64 - center_x as f64;
    let dy = cursor_y as f64 - center_y as f64;
    dx * dx + dy * dy <= radius * radius
}

const ORB_NONCLIENT_FRAME_STYLE_MASK: isize = 0x00CF_0000; // WS_CAPTION|WS_THICKFRAME|WS_SYSMENU|WS_MINIMIZEBOX|WS_MAXIMIZEBOX
const ORB_WINDOW_EDGE_EX_STYLE_MASK: isize = 0x0002_0301; // WS_EX_WINDOWEDGE|WS_EX_CLIENTEDGE|WS_EX_DLGMODALFRAME|WS_EX_STATICEDGE

#[cfg(target_os = "windows")]
static ORB_WNDPROC_INSTALLED: AtomicBool = AtomicBool::new(false);
#[cfg(target_os = "windows")]
static ORB_HOOKED_HWND: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_os = "windows")]
static ORB_ORIG_WNDPROC: AtomicUsize = AtomicUsize::new(0);
#[cfg(target_os = "windows")]
static ORB_FRAME_REPAIR_REENTRY: AtomicBool = AtomicBool::new(false);

fn strip_orb_native_frame_styles(style: isize, extended_style: isize) -> (isize, isize) {
    (
        style & !ORB_NONCLIENT_FRAME_STYLE_MASK,
        extended_style & !ORB_WINDOW_EDGE_EX_STYLE_MASK,
    )
}

#[cfg(target_os = "windows")]
unsafe fn apply_orb_clip_region_to_hwnd(
    hwnd: windows_sys::Win32::Foundation::HWND,
    _clip: bool,
) {
    use windows_sys::Win32::Graphics::Gdi::SetWindowRgn;

    // Never shape-clip with a GDI region. CreateEllipticRgn / RoundRectRgn are
    // binary masks, so the orb/capsule outline becomes a jagged 1-bit fringe.
    // CSS border-radius / clip-path already antialias the visible shape.
    SetWindowRgn(hwnd, std::ptr::null_mut(), 1);
}

#[cfg(target_os = "windows")]
fn orb_hwnd_has_caption_chrome(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowLongPtrW, GWL_STYLE};

    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    (style & ORB_NONCLIENT_FRAME_STYLE_MASK) != 0
}

#[cfg(target_os = "windows")]
fn repair_orb_frame_and_clip(hwnd: windows_sys::Win32::Foundation::HWND) {
    if ORB_FRAME_REPAIR_REENTRY.swap(true, Ordering::SeqCst) {
        return;
    }
    remove_orb_native_frame(hwnd);
    unsafe { apply_orb_clip_region_to_hwnd(hwnd, true) };
    ORB_FRAME_REPAIR_REENTRY.store(false, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn orb_window_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, DefWindowProcW, WM_ACTIVATE, WM_ERASEBKGND, WM_NCACTIVATE, WM_NCCALCSIZE,
        WM_NCPAINT, WM_WINDOWPOSCHANGED,
    };

    // Focus / NC messages can make WebView2 or DWM repaint the rectangular
    // frame and (on Win11) reinstate caption chrome. Re-strip + re-clip.
    match msg {
        WM_NCACTIVATE => {
            repair_orb_frame_and_clip(hwnd);
            return 1;
        }
        WM_ACTIVATE => {
            repair_orb_frame_and_clip(hwnd);
        }
        WM_WINDOWPOSCHANGED => {
            // Caption chrome can come back after resize; only repair then.
            // Always-applying the region here re-enters via SetWindowRgn.
            if orb_hwnd_has_caption_chrome(hwnd) {
                repair_orb_frame_and_clip(hwnd);
            }
        }
        WM_NCPAINT => return 0,
        WM_ERASEBKGND => return 1,
        WM_NCCALCSIZE if wparam != 0 => return 0,
        _ => {}
    }

    let orig = ORB_ORIG_WNDPROC.load(Ordering::Relaxed);
    if orig == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let proc = Some(std::mem::transmute::<
        usize,
        unsafe extern "system" fn(
            windows_sys::Win32::Foundation::HWND,
            u32,
            windows_sys::Win32::Foundation::WPARAM,
            windows_sys::Win32::Foundation::LPARAM,
        ) -> windows_sys::Win32::Foundation::LRESULT,
    >(orig));
    CallWindowProcW(proc, hwnd, msg, wparam, lparam)
}

#[cfg(target_os = "windows")]
fn install_orb_window_message_hook(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GWLP_WNDPROC};

    let hwnd_bits = hwnd as usize;
    if ORB_HOOKED_HWND.load(Ordering::SeqCst) == hwnd_bits && ORB_WNDPROC_INSTALLED.load(Ordering::SeqCst)
    {
        return;
    }
    let previous = unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            orb_window_proc as *const () as usize as isize,
        )
    };
    ORB_ORIG_WNDPROC.store(previous as usize, Ordering::SeqCst);
    ORB_HOOKED_HWND.store(hwnd_bits, Ordering::SeqCst);
    ORB_WNDPROC_INSTALLED.store(true, Ordering::SeqCst);
}

#[cfg(target_os = "windows")]
fn remove_orb_native_frame(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, GWL_EXSTYLE, GWL_STYLE,
        SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    };

    let current_style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
    let current_extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    let (style, extended_style) =
        strip_orb_native_frame_styles(current_style, current_extended_style);
    let style_changed = current_style != 0 && current_style != style;
    let extended_style_changed =
        current_extended_style != 0 && current_extended_style != extended_style;

    if style_changed {
        unsafe { SetWindowLongPtrW(hwnd, GWL_STYLE, style) };
    }
    if extended_style_changed {
        unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, extended_style) };
    }
    let empty: [u16; 1] = [0];
    unsafe { SetWindowTextW(hwnd, empty.as_ptr()) };

    if style_changed || extended_style_changed {
        unsafe {
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }
    // FRAMECHANGED can drop the shaped region / resurrect caption paint — clip last.
    unsafe { apply_orb_clip_region_to_hwnd(hwnd, true) };
}

#[cfg(target_os = "windows")]
fn disable_floating_orb_dwm_border(hwnd: windows::Win32::Foundation::HWND) {
    use windows::Win32::Graphics::Dwm::{
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMNCRP_DISABLED,
        DWMNCRENDERINGPOLICY, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_NCRENDERING_POLICY,
        DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMSBT_NONE, DWMWCP_DONOTROUND,
        DWM_SYSTEMBACKDROP_TYPE, DWM_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::UI::Controls::MARGINS;

    // Keep the orb outline fully under CSS control. Windows 11 may otherwise
    // paint a light system border, caption chrome, Mica backdrop, or rounded
    // frame that reads as a white strip with a close button around the capsule.
    let no_border = DWMWA_COLOR_NONE;
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            (&no_border as *const u32).cast(),
            std::mem::size_of_val(&no_border) as u32,
        )
    };
    let nc_policy = DWMNCRP_DISABLED;
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            (&nc_policy as *const DWMNCRENDERINGPOLICY).cast(),
            std::mem::size_of_val(&nc_policy) as u32,
        )
    };
    let corner = DWMWCP_DONOTROUND;
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&corner as *const DWM_WINDOW_CORNER_PREFERENCE).cast(),
            std::mem::size_of_val(&corner) as u32,
        )
    };
    let backdrop = DWMSBT_NONE;
    let _ = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            (&backdrop as *const DWM_SYSTEMBACKDROP_TYPE).cast(),
            std::mem::size_of_val(&backdrop) as u32,
        )
    };
    let margins = MARGINS {
        cxLeftWidth: 0,
        cxRightWidth: 0,
        cyTopHeight: 0,
        cyBottomHeight: 0,
    };
    let _ = unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins) };
}

#[cfg(target_os = "windows")]
fn set_orb_window_region(
    window: &WebviewWindow,
    collapsed: bool,
    side: &str,
) -> Result<(), String> {
    let focused = window.is_focused().unwrap_or(true);
    set_orb_window_region_for_focus(window, collapsed, side, focused)
}

#[cfg(target_os = "windows")]
fn set_orb_window_region_for_focus(
    window: &WebviewWindow,
    collapsed: bool,
    side: &str,
    focused: bool,
) -> Result<(), String> {
    use tauri::window::Color;

    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    // Do not call set_decorations here — toggling it on focus churn reintroduces
    // Win11 caption chrome (light strip + ✕) over the orb.
    let _ = window.set_title("");
    remove_orb_native_frame(hwnd.0 as _);
    disable_floating_orb_dwm_border(hwnd);
    install_orb_window_message_hook(hwnd.0 as _);
    let _ = collapsed;
    let _ = side;
    let _ = focused;

    // Keep WebView2 fully transparent so CSS can antialias the capsule/circle
    // against the desktop. An opaque controller color fills the rectangular
    // HWND and reads as a hard box around the rounded CSS outline.
    let _ = window.set_background_color(Some(Color(0, 0, 0, 0)));
    unsafe { apply_orb_clip_region_to_hwnd(hwnd.0 as _, false) };
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_orb_window_region_for_focus(
    window: &WebviewWindow,
    collapsed: bool,
    side: &str,
    _focused: bool,
) -> Result<(), String> {
    set_orb_window_region(window, collapsed, side)
}

#[cfg(not(target_os = "windows"))]
fn set_orb_window_region(
    _window: &WebviewWindow,
    _collapsed: bool,
    _side: &str,
) -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn set_orb_window_bounds(
    window: &WebviewWindow,
    logical_size: LogicalSize<f64>,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<(), String> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOACTIVATE, SWP_NOZORDER};

    if window.outer_position().ok() == Some(PhysicalPosition::new(x, y))
        && window
            .outer_size()
            .ok()
            .is_some_and(|size| size.width == width && size.height == height)
    {
        return Ok(());
    }

    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let result = unsafe {
        SetWindowPos(
            hwnd.0 as _,
            std::ptr::null_mut(),
            x,
            y,
            width as i32,
            height as i32,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
    };
    if result == 0 {
        return Err("无法同步悬浮球窗口位置和尺寸".to_owned());
    }
    let _ = logical_size;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn set_orb_window_bounds(
    window: &WebviewWindow,
    logical_size: LogicalSize<f64>,
    x: i32,
    y: i32,
    _width: u32,
    _height: u32,
) -> Result<(), String> {
    window
        .set_size(logical_size)
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

fn resize_orb_window_from_side(
    window: &WebviewWindow,
    expanded: bool,
    direction: &str,
    current_side: Option<&str>,
) -> Result<String, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or("找不到当前显示器")?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let old_size = window.outer_size().map_err(|error| error.to_string())?;
    let side = orb_layout_side(window, direction)?;
    let current_side = current_side.unwrap_or(side);
    let logical = floating_logical_size(
        "orb",
        expanded,
        orb_size_for_window(window),
        card_scale_for_window(window),
    );
    let scale = monitor.scale_factor();
    let width = (logical.width * scale).round() as u32;
    let height = (logical.height * scale).round() as u32;
    let margin = (6.0 * scale).round() as i32;
    let orb_width = (orb_size_for_window(window) as f64 * scale).round() as i32;
    let orb_left = if current_side == "right" {
        position.x + old_size.width as i32 - margin - orb_width
    } else {
        position.x + margin
    };

    // Keep the orb's physical position fixed. Only the transparent window
    // boundary moves around it while the teaser panel opens or closes.
    let x = orb_window_x(orb_left, orb_width, margin, side, width);
    let y = clamp_vertical(
        position.y,
        height,
        monitor.position().y,
        monitor.size().height,
        margin,
    );
    set_orb_window_bounds(window, logical, x, y, width, height)?;
    Ok(side.to_owned())
}

fn resize_orb_window_from_anchor(
    window: &WebviewWindow,
    expanded: bool,
    direction: &str,
    anchor: SavedPosition,
) -> Result<String, String> {
    let monitor = monitor_for_orb_anchor(window, anchor)?;
    let side = orb_layout_side_for_anchor(window, direction, anchor)?;
    let logical = floating_logical_size(
        "orb",
        expanded,
        orb_size_for_window(window),
        card_scale_for_window(window),
    );
    let scale = monitor.scale_factor();
    let width = (logical.width * scale).round() as u32;
    let height = (logical.height * scale).round() as u32;
    let margin = (6.0 * scale).round() as i32;
    let orb_width = (orb_size_for_window(window) as f64 * scale).round() as i32;
    let orb_left = anchor.x + margin;
    let x = orb_window_x(orb_left, orb_width, margin, side, width);
    let y = clamp_vertical(
        anchor.y,
        height,
        monitor.position().y,
        monitor.size().height,
        margin,
    );
    set_orb_window_bounds(window, logical, x, y, width, height)?;
    Ok(side.to_owned())
}

fn apply_orb_display(
    window: &WebviewWindow,
    direction: &str,
    anchor: SavedPosition,
    panel_visible: bool,
) -> Result<String, String> {
    let side = resize_orb_window_from_anchor(window, true, direction, anchor)?;
    // Keep expanded WebView bounds for a smooth hover reveal. Refresh chrome /
    // transparency; when unfocused the region clip hides WebView white fill.
    set_orb_window_region(window, !panel_visible, &side)?;
    Ok(side)
}

fn snap_orb_to_edge(window: &WebviewWindow) -> Result<String, String> {
    let monitor = window
        .current_monitor()
        .map_err(|error| error.to_string())?
        .or(window
            .primary_monitor()
            .map_err(|error| error.to_string())?)
        .ok_or("找不到当前显示器")?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let side = edge_side(
        position.x,
        size.width,
        monitor.position().x,
        monitor.size().width,
    );
    let margin = (6.0 * monitor.scale_factor()).round() as i32;
    let (left_x, right_x) = orb_anchor_dock_edges(
        monitor.position().x,
        monitor.size().width,
        size.width,
        margin,
    );
    let x = if side == "left" { left_x } else { right_x };
    let y = clamp_vertical(
        position.y,
        size.height,
        monitor.position().y,
        monitor.size().height,
        margin,
    );
    window
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    Ok(side.to_owned())
}

#[tauri::command]
fn get_floating_orb_side(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    let (direction, anchor) = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        (
            data.floating_orb_expand_direction.clone(),
            data.floating_orb_position,
        )
    };
    if let Some(anchor) = anchor {
        return Ok(orb_layout_side_for_anchor(&window, &direction, anchor)?.to_owned());
    }
    Ok(orb_layout_side(&window, &direction)?.to_owned())
}

#[tauri::command]
fn get_floating_orb_edge_state(app: AppHandle, state: State<'_, AppState>) -> Result<bool, String> {
    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    orb_is_near_edge(
        &window,
        &state,
        SavedPosition {
            x: position.x,
            y: position.y,
        },
    )
}

#[cfg(target_os = "windows")]
#[tauri::command]
fn get_floating_orb_pointer_inside(app: AppHandle) -> Result<bool, String> {
    use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::GetCursorPos};
    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let size = window.outer_size().map_err(|error| error.to_string())?;
    let mut cursor = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut cursor) } == 0 {
        return Err("无法读取鼠标位置".to_owned());
    }
    Ok(cursor.x >= position.x
        && cursor.x < position.x + size.width as i32
        && cursor.y >= position.y
        && cursor.y < position.y + size.height as i32)
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
fn get_floating_orb_pointer_inside() -> bool {
    false
}

fn saved_floating_position(data: &PersistedState, style: &str) -> Option<SavedPosition> {
    match style {
        "orb" => data.floating_orb_position,
        _ => data.floating_card_position.or(data.floating_position),
    }
}

fn remember_floating_position_for_style(
    state: &AppState,
    style: &str,
    position: SavedPosition,
) -> Result<(), String> {
    update_state(state, |data| match style {
        "orb" => data.floating_orb_position = Some(position),
        _ => data.floating_card_position = Some(position),
    })
}

#[tauri::command]
fn set_floating_style(
    app: AppHandle,
    state: State<'_, AppState>,
    style: String,
) -> Result<FloatingSettings, String> {
    if style != "card" && style != "orb" {
        return Err("悬浮窗样式无效".to_owned());
    }
    let (current_style, visible, always_on_top, orb_size, card_scale) = state
        .data
        .lock()
        .map(|data| {
            (
                data.floating_style.clone(),
                data.floating_visible,
                data.floating_always_on_top,
                data.floating_orb_size,
                data.floating_card_scale,
            )
        })
        .map_err(|_| "本地状态暂时不可用".to_owned())?;
    if current_style == style {
        return get_floating_settings(state);
    }
    let current_window = app
        .get_webview_window(floating_window_label(&current_style))
        .ok_or("当前悬浮窗尚未创建")?;
    if current_style != style {
        if let Ok(position) = current_window.outer_position() {
            let saved = SavedPosition {
                x: position.x,
                y: position.y,
            };
            let remembered = if current_style == "orb" {
                orb_anchor_from_window(&current_window, &state, saved).unwrap_or(saved)
            } else {
                saved
            };
            remember_floating_position_for_style(&state, &current_style, remembered)?;
        }
    }
    let compact_window = app
        .get_webview_window("compact")
        .ok_or("紧凑卡片窗口尚未创建")?;
    let orb_window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    let target_window = if style == "orb" {
        &orb_window
    } else {
        &compact_window
    };
    let _ = compact_window.hide();
    let _ = orb_window.hide();
    state.floating_orb.store(style == "orb", Ordering::Relaxed);
    state.floating_orb_expanded.store(false, Ordering::Relaxed);
    update_state(&state, |data| data.floating_style = style.clone())?;
    target_window
        .set_size(floating_logical_size(&style, false, orb_size, card_scale))
        .map_err(|error| error.to_string())?;
    let target_position = state
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?
        .clone();
    if let Some(position) = saved_floating_position(&target_position, &style) {
        let position = if style == "orb" {
            normalize_orb_dock_anchor(target_window, position)?
        } else {
            position
        };
        if style == "orb" {
            let _ = remember_floating_position_for_style(&state, "orb", position);
        }
        target_window
            .set_position(PhysicalPosition::new(position.x, position.y))
            .map_err(|error| error.to_string())?;
        if style == "orb" {
            let side = orb_layout_side_for_anchor(
                target_window,
                &target_position.floating_orb_expand_direction,
                position,
            )
            .unwrap_or("left");
            set_orb_window_region(&orb_window, true, side)?;
        }
    } else if style == "orb" {
        let _ = snap_orb_to_edge(target_window);
        set_orb_window_region(&orb_window, true, "left")?;
    } else {
        position_floating(&app);
    }
    if visible {
        target_window
            .set_always_on_top(always_on_top)
            .map_err(|error| error.to_string())?;
        target_window.show().map_err(|error| error.to_string())?;
    }
    let settings = get_floating_settings(state)?;
    let _ = app.emit("floating-settings-changed", settings.clone());
    Ok(settings)
}

#[tauri::command]
fn set_floating_orb_expanded(
    app: AppHandle,
    state: State<'_, AppState>,
    expanded: bool,
) -> Result<String, String> {
    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    // A delayed hover transition must never resize the native window while a
    // drag is in progress. The front end invalidates stale transitions too,
    // but this native guard closes the race between queued IPC commands.
    if state.floating_orb_dragging.load(Ordering::Relaxed) {
        return Ok(orb_layout_side(&window, "auto")?.to_owned());
    }
    let (direction, anchor) = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        (
            data.floating_orb_expand_direction.clone(),
            data.floating_orb_position,
        )
    };
    let anchor = if let Some(anchor) = anchor {
        anchor
    } else {
        let position = window.outer_position().map_err(|error| error.to_string())?;
        orb_anchor_from_window(
            &window,
            &state,
            SavedPosition {
                x: position.x,
                y: position.y,
            },
        )
        .ok_or("无法计算悬浮球位置")?
    };
    let normalized_anchor = normalize_orb_dock_anchor(&window, anchor)?;
    if normalized_anchor.x != anchor.x || normalized_anchor.y != anchor.y {
        remember_floating_position_for_style(&state, "orb", normalized_anchor)?;
    }
    let anchor = normalized_anchor;
    if !expanded && !orb_anchor_is_near_edge(&window, anchor)? {
        return Ok(orb_layout_side_for_anchor(&window, &direction, anchor)?.to_owned());
    }
    let side = apply_orb_display(&window, &direction, anchor, expanded)?;
    state
        .floating_orb_expanded
        .store(expanded, Ordering::Relaxed);
    Ok(side)
}

#[tauri::command]
fn set_floating_orb_expand_direction(
    app: AppHandle,
    state: State<'_, AppState>,
    direction: String,
) -> Result<FloatingSettings, String> {
    let direction = normalize_orb_expand_direction(&direction)
        .ok_or_else(|| "小球展开方向无效".to_owned())?
        .to_owned();
    let (previous_direction, anchor) = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        (
            data.floating_orb_expand_direction.clone(),
            data.floating_orb_position,
        )
    };
    update_state(&state, |data| {
        data.floating_orb_expand_direction = direction.clone()
    })?;

    if let Some(window) = app.get_webview_window("orb") {
        let is_orb = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?
            .floating_style
            == "orb";
        if is_orb {
            let expanded = state.floating_orb_expanded.load(Ordering::Relaxed);
            if let Some(anchor) = anchor {
                let _ = apply_orb_display(&window, &direction, anchor, expanded);
            } else {
                let current_side = if expanded {
                    orb_layout_side(&window, &previous_direction).ok()
                } else {
                    None
                };
                let _ = resize_orb_window_from_side(&window, true, &direction, current_side);
            }
        }
    }

    let settings = get_floating_settings(state)?;
    let _ = app.emit("floating-settings-changed", settings.clone());
    Ok(settings)
}

#[tauri::command]
fn snap_floating_to_edge(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let window = app
        .get_webview_window("orb")
        .ok_or("悬浮小球窗口尚未创建")?;
    let position = window.outer_position().map_err(|error| error.to_string())?;
    let (anchor, _) = orb_edge_anchor(
        &window,
        &state,
        SavedPosition {
            x: position.x,
            y: position.y,
        },
    )
    .ok_or("无法计算悬浮球吸附位置")?;
    let direction = state
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?
        .floating_orb_expand_direction
        .clone();
    let side = apply_orb_display(&window, &direction, anchor, false)?;
    state.floating_orb_expanded.store(false, Ordering::Relaxed);
    remember_floating_position_for_style(&state, "orb", anchor)?;
    Ok(side)
}

#[tauri::command]
fn show_main_window_command(app: AppHandle) {
    show_main_window(&app);
}

#[tauri::command]
fn set_floating_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    pinned: bool,
) -> Result<bool, String> {
    state.floating_pinned.store(pinned, Ordering::Relaxed);
    update_state(&state, |data| data.floating_pinned = pinned)?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(pinned)
}

#[tauri::command]
fn set_floating_opacity(
    app: AppHandle,
    state: State<'_, AppState>,
    opacity: f64,
) -> Result<f64, String> {
    let opacity = opacity.clamp(0.0, 1.0);
    update_state(&state, |data| data.floating_opacity = opacity)?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(opacity)
}

fn validate_floating_orb_size(size: u32) -> Result<u32, String> {
    if !(MIN_FLOATING_ORB_SIZE..=MAX_FLOATING_ORB_SIZE).contains(&size) || size % 2 != 0 {
        return Err(format!(
            "小球尺寸需为 {MIN_FLOATING_ORB_SIZE} 到 {MAX_FLOATING_ORB_SIZE} 之间的偶数"
        ));
    }
    Ok(size)
}

fn validate_floating_card_scale(scale: u32) -> Result<u32, String> {
    if !(MIN_FLOATING_CARD_SCALE..=MAX_FLOATING_CARD_SCALE).contains(&scale) || scale % 5 != 0 {
        return Err(format!(
            "紧凑卡片尺寸需为 {MIN_FLOATING_CARD_SCALE}% 到 {MAX_FLOATING_CARD_SCALE}% 之间的 5% 倍数"
        ));
    }
    Ok(scale)
}

#[tauri::command]
fn set_floating_orb_size(
    app: AppHandle,
    state: State<'_, AppState>,
    size: u32,
) -> Result<u32, String> {
    let size = validate_floating_orb_size(size)?;
    let (style, direction, saved_anchor) = {
        let data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        (
            data.floating_style.clone(),
            data.floating_orb_expand_direction.clone(),
            data.floating_orb_position,
        )
    };
    let orb_window = app.get_webview_window("orb");
    let current_anchor = saved_anchor.or_else(|| {
        orb_window.as_ref().and_then(|window| {
            let position = window.outer_position().ok()?;
            orb_anchor_from_window(
                window,
                &state,
                SavedPosition {
                    x: position.x,
                    y: position.y,
                },
            )
        })
    });
    let was_docked = if style == "orb" {
        current_anchor
            .and_then(|anchor| orb_anchor_is_near_edge(orb_window.as_ref()?, anchor).ok())
            .unwrap_or(false)
    } else {
        false
    };

    update_state(&state, |data| data.floating_orb_size = size)?;
    state.floating_orb_size.store(size, Ordering::Relaxed);
    if style == "orb" {
        if let (Some(window), Some(anchor)) = (orb_window, current_anchor) {
            let normalized = if was_docked {
                orb_edge_anchor_for_size(&window, anchor, size)
                    .map(|(position, _)| position)
                    .unwrap_or(anchor)
            } else {
                anchor
            };
            let side = apply_orb_display(&window, &direction, normalized, false)?;
            let _ = side;
            remember_floating_position_for_style(&state, "orb", normalized)?;
        }
    }
    let settings = get_floating_settings(state)?;
    let _ = app.emit("floating-settings-changed", settings);
    Ok(size)
}

#[tauri::command]
fn set_floating_card_scale(
    app: AppHandle,
    state: State<'_, AppState>,
    scale: u32,
) -> Result<u32, String> {
    let scale = validate_floating_card_scale(scale)?;
    update_state(&state, |data| data.floating_card_scale = scale)?;
    state.floating_card_scale.store(scale, Ordering::Relaxed);

    if let Some(window) = app.get_webview_window("compact") {
        let old_position = window.outer_position().ok();
        let old_size = window.outer_size().ok();
        window
            .set_size(floating_logical_size(
                "card",
                false,
                orb_size_for_window(&window),
                scale,
            ))
            .map_err(|error| error.to_string())?;
        if let (Some(position), Some(size)) = (old_position, old_size) {
            if let Ok(new_size) = window.outer_size() {
                let x = position.x + (size.width as i32 - new_size.width as i32) / 2;
                let y = position.y + (size.height as i32 - new_size.height as i32) / 2;
                let _ = window.set_position(PhysicalPosition::new(x, y));
            }
        }
    }

    let settings = get_floating_settings(state)?;
    let _ = app.emit("floating-settings-changed", settings);
    Ok(scale)
}

#[tauri::command]
fn set_orb_wave_speed(
    app: AppHandle,
    state: State<'_, AppState>,
    speed: f64,
) -> Result<f64, String> {
    if !speed.is_finite() {
        return Err("水波速度无效".to_owned());
    }
    let speed = speed.clamp(MIN_ORB_WAVE_SPEED, MAX_ORB_WAVE_SPEED);
    update_state(&state, |data| data.orb_wave_speed = speed)?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(speed)
}

fn normalize_sync_interval(interval_secs: u64) -> Result<u64, String> {
    if !(MIN_SYNC_INTERVAL_SECS..=MAX_SYNC_INTERVAL_SECS).contains(&interval_secs) {
        return Err(format!(
            "额度同步间隔需在 {MIN_SYNC_INTERVAL_SECS} 到 {MAX_SYNC_INTERVAL_SECS} 秒之间"
        ));
    }
    Ok(interval_secs)
}

#[tauri::command]
fn set_sync_interval(
    app: AppHandle,
    state: State<'_, AppState>,
    interval_secs: u64,
) -> Result<u64, String> {
    let interval_secs = normalize_sync_interval(interval_secs)?;
    update_state(&state, |data| data.sync_interval_secs = interval_secs)?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(interval_secs)
}

#[tauri::command]
fn begin_quota_sync(state: State<'_, AppState>, force: bool) -> Result<u64, String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64;
    let snapshot = {
        let mut data = state
            .data
            .lock()
            .map_err(|_| "本地状态暂时不可用".to_owned())?;
        if let wait_ms @ 1.. = reserve_quota_sync(&mut data, now_ms, force) {
            return Ok(wait_ms);
        }
        data.clone()
    };
    state
        .save_sender
        .send(snapshot)
        .map_err(|_| "后台存储线程已停止".to_owned())?;
    Ok(0)
}

fn normalize_orb_wave_amplitude(amplitude: f64) -> Result<f64, String> {
    if !amplitude.is_finite() {
        return Err("水波幅度无效".to_owned());
    }
    Ok(amplitude.clamp(MIN_ORB_WAVE_AMPLITUDE, MAX_ORB_WAVE_AMPLITUDE))
}

#[tauri::command]
fn set_orb_wave_amplitude(
    app: AppHandle,
    state: State<'_, AppState>,
    amplitude: f64,
) -> Result<f64, String> {
    let amplitude = normalize_orb_wave_amplitude(amplitude)?;
    update_state(&state, |data| data.orb_wave_amplitude = amplitude)?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(amplitude)
}

#[tauri::command]
fn set_floating_always_on_top(
    app: AppHandle,
    state: State<'_, AppState>,
    always_on_top: bool,
) -> Result<bool, String> {
    for label in ["compact", "orb"] {
        app.get_webview_window(label)
            .ok_or("悬浮窗尚未创建")?
            .set_always_on_top(always_on_top)
            .map_err(|error| error.to_string())?;
    }
    update_state(&state, |data| data.floating_always_on_top = always_on_top)?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(always_on_top)
}

#[tauri::command]
fn set_display_mode(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
) -> Result<String, String> {
    if mode != "available" && mode != "used" {
        return Err("显示形式无效".to_owned());
    }
    update_state(&state, |data| data.display_mode = mode.clone())?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(mode)
}

#[tauri::command]
fn set_theme(app: AppHandle, state: State<'_, AppState>, theme: String) -> Result<String, String> {
    if !matches!(
        theme.as_str(),
        "obsidian" | "titanium" | "spruce" | "dusk" | "abyss" | "cashmere" | "cinnabar" | "cyber"
    ) {
        return Err("主题配色无效".to_owned());
    }
    update_state(&state, |data| data.theme = theme.clone())?;
    let _ = app.emit("floating-settings-changed", get_floating_settings(state)?);
    Ok(theme)
}

#[tauri::command]
fn set_proxy_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    mode: String,
    address: String,
) -> Result<FloatingSettings, String> {
    if !matches!(mode.as_str(), "system" | "none" | "custom") {
        return Err("代理模式无效".to_owned());
    }
    let address = if mode == "custom" {
        normalize_proxy_address(&address)?
    } else {
        address.trim().to_owned()
    };
    update_state(&state, |data| {
        data.proxy_mode = mode;
        data.proxy_address = address;
    })?;
    let settings = get_floating_settings(state)?;
    let _ = app.emit("floating-settings-changed", settings.clone());
    Ok(settings)
}

fn toggle_pin(app: &AppHandle) -> Result<bool, String> {
    let next = !app
        .state::<AppState>()
        .data
        .lock()
        .map_err(|_| "本地状态暂时不可用".to_owned())?
        .floating_pinned;
    set_floating_pinned(app.clone(), app.state::<AppState>(), next)
}

fn position_floating(app: &AppHandle) {
    let Some(window) = app.get_webview_window("compact") else {
        return;
    };
    let Ok(Some(monitor)) = window.primary_monitor() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let margin = (18.0 * monitor.scale_factor()).round() as i32;
    let x = monitor.position().x + monitor.size().width as i32 - size.width as i32 - margin;
    let y = monitor.position().y + margin;
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn position_intersects_monitor(
    position: SavedPosition,
    window_width: u32,
    window_height: u32,
    monitor_x: i32,
    monitor_y: i32,
    monitor_width: u32,
    monitor_height: u32,
) -> bool {
    let left = position.x.max(monitor_x);
    let top = position.y.max(monitor_y);
    let right = (position.x + window_width as i32).min(monitor_x + monitor_width as i32);
    let bottom = (position.y + window_height as i32).min(monitor_y + monitor_height as i32);
    let minimum_visible_width = (window_width.min(180) as i32).max(80);
    let minimum_visible_height = (window_height.min(140) as i32).max(70);
    right - left >= minimum_visible_width && bottom - top >= minimum_visible_height
}

fn position_is_visible(window: &WebviewWindow, position: SavedPosition) -> bool {
    let Ok(size) = window.outer_size() else {
        return false;
    };
    let Ok(monitors) = window.available_monitors() else {
        return false;
    };
    monitors.into_iter().any(|monitor| {
        let monitor_position = monitor.position();
        let monitor_size = monitor.size();
        position_intersects_monitor(
            position,
            size.width,
            size.height,
            monitor_position.x,
            monitor_position.y,
            monitor_size.width,
            monitor_size.height,
        )
    })
}

fn center_main_window(window: &WebviewWindow) {
    let Ok(Some(monitor)) = window.primary_monitor() else {
        let _ = window.center();
        return;
    };
    let Ok(size) = window.outer_size() else {
        let _ = window.center();
        return;
    };
    let monitor_position = monitor.position();
    let monitor_size = monitor.size();
    let x = monitor_position.x + ((monitor_size.width as i32 - size.width as i32) / 2).max(0);
    let y = monitor_position.y + ((monitor_size.height as i32 - size.height as i32) / 2).max(0);
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

fn restore_windows(app: &AppHandle, data: &PersistedState) {
    if let Some(window) = app.get_webview_window("main") {
        if let Some(position) = data
            .main_position
            .filter(|position| position_is_visible(&window, *position))
        {
            let _ = window.set_position(PhysicalPosition::new(position.x, position.y));
        } else {
            center_main_window(&window);
        }
    }
    if let Some(window) = app.get_webview_window("compact") {
        let _ = window.set_size(floating_logical_size(
            "card",
            false,
            data.floating_orb_size,
            data.floating_card_scale,
        ));
        if let Some(position) = saved_floating_position(data, "card") {
            let _ = window.set_position(PhysicalPosition::new(position.x, position.y));
        } else {
            position_floating(app);
        }
        let _ = window.set_always_on_top(data.floating_always_on_top);
        let _ = window.hide();
        if data.floating_visible && data.floating_style == "card" {
            let _ = window.show();
        }
    }
    if let Some(window) = app.get_webview_window("orb") {
        let _ = window.set_size(floating_logical_size(
            "orb",
            true,
            data.floating_orb_size,
            data.floating_card_scale,
        ));
        let saved_position = saved_floating_position(data, "orb")
            .map(|position| normalize_orb_dock_anchor(&window, position).unwrap_or(position));
        let region_side = if let Some(position) = saved_position {
            let _ = window.set_position(PhysicalPosition::new(position.x, position.y));
            let _ = remember_floating_position_for_style(&app.state::<AppState>(), "orb", position);
            orb_layout_side_for_anchor(&window, &data.floating_orb_expand_direction, position)
                .unwrap_or("left")
        } else {
            let _ = snap_orb_to_edge(&window);
            "left"
        };
        let _ = set_orb_window_region(&window, true, region_side);
        let _ = window.set_always_on_top(data.floating_always_on_top);
        let _ = window.hide();
        if data.floating_visible && data.floating_style == "orb" {
            let _ = window.show();
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    {
        configure_webview2_transparent_background();
        windows_integration::initialize_taskbar();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            handle_launch_action(app, &args);
        }))
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = executable_data_dir().map_err(std::io::Error::other)?;
            fs::create_dir_all(&data_dir)?;
            let file_path = data_dir.join("state.json");
            let legacy_file = app.path().app_data_dir()?.join("data/state.json");
            migrate_legacy_state(&legacy_file, &file_path);
            let mut persisted = load_state(&file_path);
            let repair_theme_setting = !matches!(
                persisted.theme.as_str(),
                "obsidian"
                    | "titanium"
                    | "spruce"
                    | "dusk"
                    | "abyss"
                    | "cashmere"
                    | "cinnabar"
                    | "cyber"
            );
            if repair_theme_setting {
                persisted.theme = "obsidian".to_owned();
            }
            let save_sender = start_state_writer(file_path.clone());
            let floating_pinned = Arc::new(AtomicBool::new(persisted.floating_pinned));
            let floating_orb = Arc::new(AtomicBool::new(persisted.floating_style == "orb"));
            let floating_orb_size = Arc::new(AtomicU32::new(persisted.floating_orb_size));
            let floating_card_scale = Arc::new(AtomicU32::new(persisted.floating_card_scale));
            let floating_orb_expanded = Arc::new(AtomicBool::new(false));
            app.manage(AppState {
                file_path,
                data: Mutex::new(persisted.clone()),
                save_sender,
                floating_pinned: Arc::clone(&floating_pinned),
                floating_orb: Arc::clone(&floating_orb),
                floating_orb_size: Arc::clone(&floating_orb_size),
                floating_card_scale: Arc::clone(&floating_card_scale),
                floating_orb_dragging: AtomicBool::new(false),
                floating_orb_expanded: Arc::clone(&floating_orb_expanded),
            });
            if repair_theme_setting {
                let _ = update_state(&app.state::<AppState>(), |data| {
                    if repair_theme_setting {
                        data.theme = "obsidian".to_owned();
                    }
                });
            }
            restore_windows(app.handle(), &persisted);
            if let Some(window) = app.get_webview_window("compact") {
                start_click_through_controller(
                    window,
                    Arc::clone(&floating_pinned),
                    Arc::clone(&floating_orb),
                );
            }
            if let Some(window) = app.get_webview_window("orb") {
                start_orb_click_through_controller(
                    window,
                    Arc::clone(&floating_orb),
                    floating_orb_expanded,
                    floating_orb_size,
                );
            }
            let show = MenuItem::with_id(app, "show-main", "显示主窗口", true, None::<&str>)?;
            let floating = MenuItem::with_id(
                app,
                "toggle-floating",
                "显示 / 隐藏悬浮窗",
                true,
                None::<&str>,
            )?;
            let pin = MenuItem::with_id(
                app,
                "toggle-pin",
                "固定 / 取消固定悬浮窗",
                true,
                None::<&str>,
            )?;
            let quit = MenuItem::with_id(app, "quit", "完全退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &floating, &pin, &quit])?;
            TrayIconBuilder::with_id("codex-quota-tray")
                .icon(app.default_window_icon().expect("default app icon").clone())
                .tooltip("Codex 额度")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show-main" => show_main_window(app),
                    "toggle-floating" => {
                        let _ = toggle_floating(app);
                    }
                    "toggle-pin" => {
                        let _ = toggle_pin(app);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                })
                .build(app)?;
            #[cfg(target_os = "windows")]
            windows_integration::refresh_taskbar();
            let args = env::args().collect::<Vec<_>>();
            handle_launch_action(app.handle(), &args);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Moved(position) => {
                let saved = SavedPosition {
                    x: position.x,
                    y: position.y,
                };
                let label = window.label().to_owned();
                let state = window.app_handle().state::<AppState>();
                if label == "main" {
                    let _ = update_state(&state, |data| data.main_position = Some(saved));
                } else if label == "compact" {
                    let _ = remember_floating_position_for_style(&state, "card", saved);
                } else if label == "orb" {
                    if state.floating_orb_dragging.load(Ordering::Relaxed) {
                        return;
                    }
                    let orb_position = window
                        .app_handle()
                        .get_webview_window("orb")
                        .and_then(|orb| orb_anchor_from_window(&orb, &state, saved))
                        .unwrap_or(saved);
                    let _ = remember_floating_position_for_style(&state, "orb", orb_position);
                }
            }
            WindowEvent::Focused(focused) if window.label() == "orb" => {
                if let Some(orb) = window.app_handle().get_webview_window("orb") {
                    let state = window.app_handle().state::<AppState>();
                    let expanded = state.floating_orb_expanded.load(Ordering::Relaxed);
                    let side = orb
                        .outer_position()
                        .ok()
                        .and_then(|position| {
                            orb_anchor_from_window(
                                &orb,
                                &state,
                                SavedPosition {
                                    x: position.x,
                                    y: position.y,
                                },
                            )
                            .and_then(|anchor| {
                                let direction = state
                                    .data
                                    .lock()
                                    .ok()?
                                    .floating_orb_expand_direction
                                    .clone();
                                orb_layout_side_for_anchor(&orb, &direction, anchor).ok()
                            })
                        })
                        .unwrap_or("left");
                    let _ = set_orb_window_region_for_focus(&orb, !expanded, side, *focused);
                    let _ = orb.emit("orb-window-focus", *focused);
                }
            }
            WindowEvent::Resized(_)
                if window.label() == "main" && window.is_minimized().unwrap_or(false) =>
            {
                let _ = window.hide();
            }
            WindowEvent::CloseRequested { api, .. } if window.label() == "main" => {
                api.prevent_close();
                window.app_handle().exit(0);
            }
            WindowEvent::CloseRequested { api, .. }
                if window.label() == "compact" || window.label() == "orb" =>
            {
                api.prevent_close();
                let _ = set_floating_visible(window.app_handle(), false);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            get_codex_usage,
            get_cached_usage,
            get_cached_cursor_quota,
            get_token_usage_stats,
            get_cursor_quota,
            get_cursor_token_usage_stats,
            set_floating_window,
            set_active_platform,
            get_floating_settings,
            set_floating_pinned,
            set_floating_opacity,
            set_floating_orb_size,
            set_floating_card_scale,
            set_orb_wave_speed,
            set_orb_wave_amplitude,
            set_sync_interval,
            begin_quota_sync,
            set_floating_always_on_top,
            set_floating_style,
            set_floating_orb_expanded,
            set_floating_orb_expand_direction,
            get_floating_orb_side,
            get_floating_orb_edge_state,
            get_floating_orb_pointer_inside,
            snap_floating_to_edge,
            start_floating_orb_drag,
            show_main_window_command,
            set_display_mode,
            set_theme,
            set_proxy_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(target_os = "windows")]
fn configure_webview2_transparent_background() {
    // WebView2's default backing surface is white. Tauri/Wry also sets a
    // transparent controller color, but that API can take effect after the
    // first WebView frame. The environment variable is read when WebView2's
    // environment is created, preventing white pixels in transparent corners.
    env::set_var("WEBVIEW2_DEFAULT_BACKGROUND_COLOR", "00000000");
}

#[cfg(test)]
mod tests {
    use super::{
        add_token_usage_record, clamp_vertical, cursor_event_timestamp,
        cursor_fetch_error_snapshot, cursor_http_status_error, edge_side, find_codex_launcher,
        floating_logical_size, floating_orb_shell_size, is_floating_pin_hit, is_pointer_inside_orb,
        launch_action, migrate_legacy_orb_dock_anchor, migrate_sync_interval,
        normalize_cursor_session_token, normalize_orb_expand_direction,
        normalize_orb_wave_amplitude, normalize_proxy_address, normalize_sync_interval,
        orb_anchor_dock_edges, orb_anchor_is_near_dock_edge, orb_anchor_x_from_window,
        orb_layout_side_from_metrics, orb_side_for_direction, orb_window_x,
        parse_cursor_usage_summary, parse_session_timestamp, parse_usage_responses,
        persist_cursor_session_token, position_intersects_monitor, quota_sync_wait_ms,
        read_codex_usage, read_cursor_token_usage_stats, read_token_usage_stats,
        remove_orb_native_frame, reserve_quota_sync, strip_orb_native_frame_styles,
        validate_floating_card_scale, validate_floating_orb_size, LaunchAction, NetworkSettings,
        PersistedState, SavedPosition, SessionTokenUsage, DEFAULT_FLOATING_CARD_SCALE,
        DEFAULT_ORB_WAVE_AMPLITUDE, DEFAULT_ORB_WAVE_SPEED, DEFAULT_SYNC_INTERVAL_SECS,
        FLOATING_ORB_SIZE, MAX_FLOATING_CARD_SCALE, MAX_FLOATING_ORB_SIZE, MAX_ORB_WAVE_AMPLITUDE,
        MAX_SYNC_INTERVAL_SECS, MIN_FLOATING_CARD_SCALE, MIN_FLOATING_ORB_SIZE,
        MIN_ORB_WAVE_AMPLITUDE, MIN_SYNC_INTERVAL_SECS,
    };
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
    use reqwest::StatusCode;
    use serde_json::{json, Value};
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn cursor_local_login_replaces_stale_tokscale_active_account() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "cursor-active-account-{}-{unique}",
            std::process::id()
        ));
        let config = home.join(".config/tokscale");
        fs::create_dir_all(&config).unwrap();
        let credentials = config.join("cursor-credentials.json");
        fs::write(
            &credentials,
            serde_json::to_vec(&json!({
                "version": 1,
                "activeAccountId": "user_previous",
                "accounts": {
                    "user_previous": {"sessionToken": "previous-session", "createdAt": "2026-01-01T00:00:00Z"}
                }
            }))
            .unwrap(),
        )
        .unwrap();

        persist_cursor_session_token(&home, "user_current", "current-session").unwrap();

        let saved: Value = serde_json::from_slice(&fs::read(&credentials).unwrap()).unwrap();
        let _ = fs::remove_dir_all(&home);
        assert_eq!(saved["activeAccountId"], "user_current");
        assert_eq!(
            saved["accounts"]["user_current"]["sessionToken"],
            "current-session"
        );
    }

    #[test]
    fn cursor_session_token_is_normalized_to_tokscale_cookie_format() {
        let payload = URL_SAFE_NO_PAD.encode(r#"{"sub":"user_cursor_123"}"#);
        let jwt = format!("eyJhbGciOiJub25lIn0.{payload}.signature");
        let (user_id, session) = normalize_cursor_session_token(&jwt).unwrap();
        assert_eq!(user_id, "user_cursor_123");
        assert_eq!(session, format!("user_cursor_123%3A%3A{jwt}"));

        let (prefixed_user_id, prefixed_session) =
            normalize_cursor_session_token(&format!("user_cursor_123::{jwt}")).unwrap();
        assert_eq!(prefixed_user_id, "user_cursor_123");
        assert_eq!(prefixed_session, format!("user_cursor_123%3A%3A{jwt}"));
    }

    #[test]
    fn cursor_quota_parser_maps_account_usage_percent_and_usd() {
        let snapshot = parse_cursor_usage_summary(
            &json!({
                "membershipType": "pro",
                "billingCycleEnd": "2026-10-01T00:00:00Z",
                "individualUsage": {
                    "plan": {
                        "autoPercentUsed": 21.5,
                        "apiPercentUsed": 7,
                        "totalPercentUsed": 18,
                        "used": 1250,
                        "limit": 5000,
                        "remaining": 3750
                    },
                    "onDemand": {"used": 245, "limit": 1000, "remaining": 755}
                }
            }),
            &json!({"email": "cursor@example.test"}),
            Some(&json!({"gpt-4": {"numRequestsTotal": 12, "maxRequestUsage": 100}})),
            1_790_000_000,
        );

        assert_eq!(snapshot.status, "connected");
        assert_eq!(snapshot.email.as_deref(), Some("cursor@example.test"));
        assert_eq!(snapshot.membership_type.as_deref(), Some("pro"));
        assert_eq!(snapshot.cursor_models_percent, Some(21.5));
        assert_eq!(snapshot.other_models_percent, Some(7.0));
        assert_eq!(snapshot.total_percent, Some(18.0));
        assert_eq!(snapshot.plan_used_usd, Some(12.5));
        assert_eq!(snapshot.plan_limit_usd, Some(50.0));
        assert_eq!(snapshot.plan_remaining_usd, Some(37.5));
        assert_eq!(snapshot.on_demand_used_usd, Some(2.45));
        assert_eq!(snapshot.requests_used, Some(12));
    }

    #[test]
    fn cursor_http_errors_distinguish_auth_from_access_protection() {
        assert_eq!(
            cursor_http_status_error(StatusCode::UNAUTHORIZED).as_deref(),
            Some("unauthorized")
        );
        assert_eq!(
            cursor_http_status_error(StatusCode::FORBIDDEN).as_deref(),
            Some("forbidden")
        );
        assert_eq!(
            cursor_http_status_error(StatusCode::TOO_MANY_REQUESTS).as_deref(),
            Some("http_status_429")
        );
        assert_eq!(cursor_http_status_error(StatusCode::OK), None);

        let forbidden = cursor_fetch_error_snapshot("forbidden", "sync");
        assert_eq!(forbidden.status, "unavailable");
        assert!(forbidden.message.as_deref().is_some_and(
            |message| message.contains("HTTP 403") && message.contains("不一定代表登录失效")
        ));
        let unauthorized = cursor_fetch_error_snapshot("unauthorized", "sync");
        assert_eq!(unauthorized.status, "unauthorized");
        assert!(unauthorized
            .message
            .as_deref()
            .is_some_and(|message| message.contains("HTTP 401")));
    }

    #[test]
    fn cursor_token_cache_sums_input_output_and_cache_tokens_by_local_day() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let home = std::env::temp_dir().join(format!(
            "cursor-token-stats-{}-{unique}",
            std::process::id()
        ));
        let cache = home.join(".config/tokscale/cursor-cache");
        fs::create_dir_all(&cache).unwrap();
        let file = cache.join("usage.json");
        fs::write(
            &file,
            serde_json::to_vec(&json!({
                "usageEventsDisplay": [
                    {"timestamp": 1_710_000_000_000i64, "model": "gpt-4o", "tokenUsage": {
                        "inputTokens": 100, "outputTokens": 200, "cacheReadTokens": 30, "cacheWriteTokens": 4
                    }},
                    {"timestamp": "1709999999000", "model": "claude-3.7", "tokenUsage": {
                        "inputTokens": "7", "outputTokens": 3
                    }},
                    {"timestamp": 1_710_000_000_000i64, "tokenUsage": {"inputTokens": 1000}}
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            cursor_event_timestamp(Some(&json!(1_710_000_000_000i64))),
            Some(1_710_000_000)
        );
        let stats = read_cursor_token_usage_stats(&home, 1_710_000_000, 1_710_000_100, false, None);
        let _ = fs::remove_dir_all(&home);

        assert!(stats.cache_available);
        assert_eq!(stats.events_scanned, 2);
        assert_eq!(stats.total_tokens, 344);
        assert_eq!(stats.today_tokens, 334);
    }

    #[test]
    fn token_usage_stats_sum_sessions_and_only_count_local_today_events() {
        let today_start = parse_session_timestamp(Some(&json!({
            "timestamp": "2026-09-29T00:00:00+08:00"
        })))
        .unwrap();
        let tomorrow_start = today_start + 24 * 60 * 60;
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let codex_home =
            std::env::temp_dir().join(format!("codex-token-stats-{}-{unique}", std::process::id()));
        let sessions = codex_home.join("sessions/2026/09");
        fs::create_dir_all(&sessions).unwrap();
        let first_session = sessions.join("rollout-first.jsonl");
        let second_session = sessions.join("rollout-second.jsonl");
        fs::write(
            &first_session,
            concat!(
                "{\"timestamp\":\"2026-09-28T23:59:00+08:00\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"total_tokens\":100},\"last_token_usage\":{\"total_tokens\":100}}}}\n",
                "{\"timestamp\":\"2026-09-29T00:05:00+08:00\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"total_tokens\":250},\"last_token_usage\":{\"total_tokens\":150}}}}\n"
            ),
        )
        .unwrap();
        fs::write(
            &second_session,
            "{\"timestamp\":\"2026-09-28T15:00:00Z\",\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"total_tokens\":80},\"last_token_usage\":{\"total_tokens\":80}}}}\n",
        )
        .unwrap();

        let stats = read_token_usage_stats(&codex_home, today_start, tomorrow_start);
        let _ = fs::remove_dir_all(&codex_home);

        assert!(stats.available);
        assert_eq!(stats.sessions_scanned, 2);
        assert_eq!(stats.total_tokens, 330);
        assert_eq!(stats.today_tokens, 150);
        assert_eq!(stats.unreadable_sessions, 0);
    }

    #[test]
    fn ignores_non_token_records_and_uses_last_usage_when_total_is_missing() {
        let today_start = parse_session_timestamp(Some(&json!({
            "timestamp": "2026-09-29T00:00:00Z"
        })))
        .unwrap();
        let tomorrow_start = today_start + 24 * 60 * 60;
        let mut usage = SessionTokenUsage::default();
        add_token_usage_record(
            &json!({"timestamp":"2026-09-29T08:00:00Z","type":"response_item","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":9}}}}),
            today_start,
            tomorrow_start,
            &mut usage,
        );
        add_token_usage_record(
            &json!({"timestamp":"2026-09-29T08:00:00Z","type":"event_msg","payload":{"type":"token_count","info":{"last_token_usage":{"total_tokens":9}}}}),
            today_start,
            tomorrow_start,
            &mut usage,
        );

        assert_eq!(usage.fallback_total_tokens, 9);
        assert_eq!(usage.today_tokens, 9);
    }

    #[test]
    fn parses_windows_jump_list_actions() {
        assert_eq!(
            launch_action(&["codex-quota.exe".into(), "--toggle-floating".into()]),
            LaunchAction::ToggleFloating
        );
        assert_eq!(
            launch_action(&["codex-quota.exe".into(), "--toggle-pin".into()]),
            LaunchAction::TogglePin
        );
        assert_eq!(
            launch_action(&["codex-quota.exe".into(), "--quit".into()]),
            LaunchAction::Quit
        );
        assert_eq!(
            launch_action(&["codex-quota.exe".into()]),
            LaunchAction::ShowMain
        );
    }

    #[test]
    fn reports_a_clear_error_when_codex_is_not_logged_in() {
        let rate = json!({"id":2,"error":{"message":"authentication required"}});
        let account = json!({"id":3,"result":{"account":null,"requiresOpenaiAuth":true}});
        let error = parse_usage_responses(&rate, Some(&account), 0).unwrap_err();
        assert!(error.contains("尚未登录") && error.contains("codex login"));
    }

    #[test]
    fn preserves_server_reset_timestamp_without_recalculating_it() {
        let rate = json!({"id":2,"result":{"rateLimits":{"primary":{"usedPercent":12,"windowDurationMins":300,"resetsAt":1790056965},"secondary":null}}});
        let account =
            json!({"id":3,"result":{"account":{"type":"chatgpt","email":"test@example.com"}}});
        assert_eq!(
            parse_usage_responses(&rate, Some(&account), 100)
                .unwrap()
                .primary
                .unwrap()
                .resets_at,
            1790056965
        );
    }

    #[test]
    fn persisted_state_is_backward_compatible() {
        let state: PersistedState = serde_json::from_str("{}").unwrap();
        assert_eq!(state.active_platform, "codex");
        assert!(state.last_cursor_quota.is_none());
        assert_eq!(state.floating_opacity, 0.92);
        assert!(!state.floating_pinned);
        assert!(state.floating_always_on_top);
        assert_eq!(state.floating_style, "card");
        assert!(state.floating_card_position.is_none());
        assert!(state.floating_orb_position.is_none());
        assert_eq!(state.floating_orb_expand_direction, "auto");
        assert_eq!(state.floating_orb_size, FLOATING_ORB_SIZE);
        assert_eq!(state.floating_card_scale, DEFAULT_FLOATING_CARD_SCALE);
        assert_eq!(state.orb_wave_speed, DEFAULT_ORB_WAVE_SPEED);
        assert_eq!(state.orb_wave_amplitude, DEFAULT_ORB_WAVE_AMPLITUDE);
        assert_eq!(state.sync_interval_secs, DEFAULT_SYNC_INTERVAL_SECS);
        assert_eq!(state.display_mode, "available");
        assert_eq!(state.proxy_mode, "system");

        let selected: PersistedState =
            serde_json::from_str(r#"{"activePlatform":"cursor"}"#).unwrap();
        assert_eq!(selected.active_platform, "cursor");
    }

    #[test]
    fn persists_cursor_quota_cache_for_instant_floating_display() {
        let state: PersistedState = serde_json::from_value(json!({
            "lastCursorQuota": {
                "status": "connected",
                "fetchedAt": 1_790_000_000
            }
        }))
        .unwrap();
        let serialized = serde_json::to_value(state).unwrap();

        assert_eq!(
            serialized
                .get("lastCursorQuota")
                .and_then(|cache| cache.get("status"))
                .and_then(|status| status.as_str()),
            Some("connected")
        );
    }

    #[test]
    fn bounds_orb_wave_amplitude_and_rejects_non_finite_values() {
        assert_eq!(normalize_orb_wave_amplitude(1.25).unwrap(), 1.25);
        assert_eq!(
            normalize_orb_wave_amplitude(0.0).unwrap(),
            MIN_ORB_WAVE_AMPLITUDE
        );
        assert_eq!(
            normalize_orb_wave_amplitude(9.0).unwrap(),
            MAX_ORB_WAVE_AMPLITUDE
        );
        assert!(normalize_orb_wave_amplitude(f64::NAN).is_err());
    }

    #[test]
    fn migrates_legacy_minute_sync_intervals_to_seconds() {
        let legacy: PersistedState =
            serde_json::from_value(json!({ "syncIntervalMins": 7 })).unwrap();
        let migrated = migrate_sync_interval(legacy);
        assert_eq!(migrated.sync_interval_secs, 420);
        assert_eq!(migrated.legacy_sync_interval_mins, None);

        let saved = serde_json::to_value(migrated).unwrap();
        assert_eq!(
            saved
                .get("syncIntervalSecs")
                .and_then(|value| value.as_u64()),
            Some(420)
        );
        assert!(saved.get("syncIntervalMins").is_none());
    }

    #[test]
    fn validates_sync_interval_seconds_and_preserves_custom_values() {
        assert_eq!(normalize_sync_interval(5).unwrap(), 5);
        assert_eq!(normalize_sync_interval(MIN_SYNC_INTERVAL_SECS).unwrap(), 5);
        assert_eq!(
            normalize_sync_interval(MAX_SYNC_INTERVAL_SECS).unwrap(),
            86_400
        );
        assert_eq!(normalize_sync_interval(37).unwrap(), 37);
        assert!(normalize_sync_interval(0).is_err());
        assert!(normalize_sync_interval(4).is_err());
        assert!(normalize_sync_interval(86_401).is_err());
    }

    #[test]
    fn cooldown_uses_the_last_sync_time_and_shared_interval() {
        assert_eq!(quota_sync_wait_ms(None, 60, 10_000), 0);
        assert_eq!(quota_sync_wait_ms(Some(1_000), 5, 5_999), 1);
        assert_eq!(quota_sync_wait_ms(Some(1_000), 5, 6_000), 0);
        assert_eq!(quota_sync_wait_ms(Some(1_000), 5, 500), 5_500);
    }

    #[test]
    fn switching_platform_does_not_bypass_shared_quota_sync_cooldown() {
        let mut state = PersistedState::default();
        state.sync_interval_secs = 75;
        assert_eq!(reserve_quota_sync(&mut state, 1_000, false), 0);

        state.active_platform = "cursor".to_owned();
        assert_eq!(reserve_quota_sync(&mut state, 2_000, false), 74_000);
        state.active_platform = "codex".to_owned();
        assert_eq!(reserve_quota_sync(&mut state, 75_999, false), 1);
        assert_eq!(reserve_quota_sync(&mut state, 76_000, false), 0);
        assert_eq!(state.last_quota_sync_at_ms, Some(76_000));

        assert_eq!(reserve_quota_sync(&mut state, 76_500, true), 0);
        assert_eq!(state.last_quota_sync_at_ms, Some(76_500));
    }

    #[test]
    fn keeps_card_and_orb_positions_in_separate_slots() {
        let state: PersistedState = serde_json::from_value(json!({
            "floatingCardPosition": {"x": 10, "y": 20},
            "floatingOrbPosition": {"x": 1800, "y": 400}
        }))
        .unwrap();
        assert_eq!(state.floating_card_position.unwrap().x, 10);
        assert_eq!(state.floating_orb_position.unwrap().x, 1800);
    }

    #[test]
    fn chooses_the_nearest_monitor_edge_for_the_orb() {
        assert_eq!(edge_side(30, 52, 0, 1920), "left");
        assert_eq!(edge_side(1800, 52, 0, 1920), "right");
        assert_eq!(edge_side(-1880, 52, -1920, 1920), "left");
    }

    #[test]
    fn orb_hit_test_side_uses_cached_monitor_metrics_without_window_queries() {
        assert_eq!(
            orb_layout_side_from_metrics("auto", -6, 68, 0, 1920, 1.0),
            "left"
        );
        assert_eq!(
            orb_layout_side_from_metrics("auto", 1858, 68, 0, 1920, 1.0),
            "right"
        );
        assert_eq!(
            orb_layout_side_from_metrics("left", 800, 68, 0, 1920, 1.0),
            "right"
        );
        assert_eq!(
            orb_layout_side_from_metrics("right", 800, 68, 0, 1920, 1.0),
            "left"
        );
        assert_eq!(
            orb_layout_side_from_metrics("auto", -1910, 102, -1920, 1920, 1.5),
            "left"
        );
    }

    #[test]
    fn accepts_only_supported_orb_expand_directions() {
        assert_eq!(normalize_orb_expand_direction("auto"), Some("auto"));
        assert_eq!(normalize_orb_expand_direction("left"), Some("left"));
        assert_eq!(normalize_orb_expand_direction("right"), Some("right"));
        assert_eq!(normalize_orb_expand_direction("bottom"), None);
        assert_eq!(orb_side_for_direction("left", "left"), "right");
        assert_eq!(orb_side_for_direction("right", "right"), "left");
        assert_eq!(orb_side_for_direction("auto", "right"), "right");
    }

    #[test]
    fn keeps_the_orb_anchor_when_window_expands_or_collapses() {
        let orb_left = 506;
        let orb_width = 56;
        let margin = 6;
        assert_eq!(orb_window_x(orb_left, orb_width, margin, "left", 68), 500);
        assert_eq!(orb_window_x(orb_left, orb_width, margin, "right", 230), 338);
        assert_eq!(orb_window_x(orb_left, orb_width, margin, "right", 68), 500);
        assert_eq!(orb_anchor_x_from_window(500, 68, 68, "left"), 500);
        assert_eq!(orb_anchor_x_from_window(338, 230, 68, "right"), 500);
        assert_eq!(orb_anchor_x_from_window(500, 68, 68, "right"), 500);
    }

    #[test]
    fn keeps_the_orb_inside_the_monitor_vertical_bounds() {
        assert_eq!(clamp_vertical(-50, 52, 0, 1080, 6), 6);
        assert_eq!(clamp_vertical(500, 52, 0, 1080, 6), 500);
        assert_eq!(clamp_vertical(2000, 52, 0, 1080, 6), 1022);
    }

    #[test]
    fn orb_and_card_use_logical_dimensions_at_each_configured_size() {
        assert_eq!(floating_orb_shell_size(FLOATING_ORB_SIZE), 68.0);
        let collapsed = floating_logical_size("orb", false, FLOATING_ORB_SIZE, 100);
        let expanded = floating_logical_size("orb", true, FLOATING_ORB_SIZE, 100);
        assert_eq!((collapsed.width, collapsed.height), (68.0, 68.0));
        assert_eq!((expanded.width, expanded.height), (230.0, 68.0));

        let enlarged_orb = floating_logical_size("orb", true, 72, 100);
        assert_eq!((enlarged_orb.width, enlarged_orb.height), (246.0, 84.0));
        let small_card = floating_logical_size("card", false, 56, 80);
        let large_card = floating_logical_size("card", false, 56, 140);
        assert_eq!((small_card.width, small_card.height), (220.8, 120.0));
        assert_eq!((large_card.width, large_card.height), (386.4, 210.0));
    }

    #[test]
    fn orb_hit_testing_tracks_the_antialiased_circle_at_multiple_dpis() {
        assert!(is_pointer_inside_orb(34, 34, 0, 0, 300, 80, 56, 6, "left"));
        assert!(!is_pointer_inside_orb(6, 6, 0, 0, 300, 80, 56, 6, "left"));
        assert!(is_pointer_inside_orb(
            266, 34, 0, 0, 300, 80, 56, 6, "right"
        ));
        assert!(!is_pointer_inside_orb(
            238, 6, 0, 0, 300, 80, 56, 6, "right"
        ));

        for scale in [1.0, 1.25, 1.5, 2.0] {
            let diameter = (56.0_f64 * scale).round() as i32;
            let inset = (6.0_f64 * scale).round() as i32;
            let shell_width = (230.0_f64 * scale).round() as i32;
            let shell_height = (68.0_f64 * scale).round() as i32;
            let circle_left = shell_width - inset - diameter;
            let center_x = circle_left + diameter / 2;
            let center_y = inset + diameter / 2;
            assert!(is_pointer_inside_orb(
                center_x,
                center_y,
                0,
                0,
                shell_width,
                shell_height,
                diameter,
                inset,
                "right"
            ));
            assert!(!is_pointer_inside_orb(
                circle_left + 1,
                inset + 1,
                0,
                0,
                shell_width,
                shell_height,
                diameter,
                inset,
                "right"
            ));
        }
        assert!(!is_pointer_inside_orb(20, 20, 0, 0, 345, 102, 0, 9, "left"));
    }

    #[test]
    fn configured_floating_sizes_are_bounded_and_step_aligned() {
        assert_eq!(validate_floating_orb_size(MIN_FLOATING_ORB_SIZE), Ok(48));
        assert_eq!(validate_floating_orb_size(MAX_FLOATING_ORB_SIZE), Ok(88));
        assert!(validate_floating_orb_size(47).is_err());
        assert!(validate_floating_orb_size(90).is_err());
        assert_eq!(
            validate_floating_card_scale(MIN_FLOATING_CARD_SCALE),
            Ok(80)
        );
        assert_eq!(
            validate_floating_card_scale(MAX_FLOATING_CARD_SCALE),
            Ok(140)
        );
        assert!(validate_floating_card_scale(82).is_err());
        assert!(validate_floating_card_scale(145).is_err());
    }

    #[test]
    fn strips_the_native_window_frame_without_changing_other_orb_styles() {
        let existing_style = 0x14C8_0000;
        let existing_extended_style = 0x0004_0118;
        let borderless = strip_orb_native_frame_styles(existing_style, existing_extended_style);

        assert_eq!(borderless, (0x1408_0000, 0x0004_0018));
        assert_eq!(
            strip_orb_native_frame_styles(borderless.0, borderless.1),
            borderless,
            "expanded/collapsed and left/right updates must stay idempotent"
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn configures_webview2_default_background_as_transparent() {
        super::configure_webview2_transparent_background();
        assert_eq!(
            std::env::var("WEBVIEW2_DEFAULT_BACKGROUND_COLOR").as_deref(),
            Ok("00000000")
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn removes_the_nonclient_frame_from_a_real_win32_window() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, GetWindowLongPtrW, GWL_EXSTYLE, GWL_STYLE, WS_BORDER,
            WS_CAPTION, WS_DLGFRAME, WS_EX_CLIENTEDGE, WS_EX_DLGMODALFRAME, WS_EX_STATICEDGE,
            WS_EX_WINDOWEDGE, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
        };

        let class_name = "STATIC\0".encode_utf16().collect::<Vec<_>>();
        let title = "orb-frame-regression\0".encode_utf16().collect::<Vec<_>>();
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_DLGMODALFRAME | WS_EX_STATICEDGE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP
                    | WS_CAPTION
                    | WS_THICKFRAME
                    | WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_BORDER
                    | WS_DLGFRAME,
                0,
                0,
                68,
                68,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!hwnd.is_null(), "test window creation should succeed");

        remove_orb_native_frame(hwnd);

        let style = unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) };
        let extended_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
        assert_eq!(
            style
                & (WS_CAPTION
                    | WS_THICKFRAME
                    | WS_SYSMENU
                    | WS_MINIMIZEBOX
                    | WS_MAXIMIZEBOX
                    | WS_BORDER
                    | WS_DLGFRAME) as isize,
            0
        );
        assert_eq!(
            extended_style
                & (WS_EX_WINDOWEDGE | WS_EX_CLIENTEDGE | WS_EX_DLGMODALFRAME | WS_EX_STATICEDGE)
                    as isize,
            0
        );

        unsafe { DestroyWindow(hwnd) };
    }

    #[test]
    fn only_orbs_docked_to_the_screen_edge_count_as_edge_collapsed() {
        assert!(orb_anchor_is_near_dock_edge(-6, -6, 838, 1.0));
        assert!(orb_anchor_is_near_dock_edge(-4, -6, 838, 1.0));
        assert!(!orb_anchor_is_near_dock_edge(14, -6, 838, 1.0));
        assert!(orb_anchor_is_near_dock_edge(836, -6, 838, 1.0));
        assert!(!orb_anchor_is_near_dock_edge(818, -6, 838, 1.0));
        assert!(orb_anchor_is_near_dock_edge(-4, -6, 838, 1.25));
        assert!(!orb_anchor_is_near_dock_edge(-2, -6, 838, 1.25));
    }

    #[test]
    fn docks_the_visible_orb_tangent_to_both_display_edges() {
        let (dock_left, dock_right) = orb_anchor_dock_edges(0, 1920, 68, 6);
        assert_eq!(dock_left + 6, 0);
        assert_eq!(dock_left + 6 + 56, 56);
        assert_eq!(dock_right + 6, 1864);
        assert_eq!(dock_right + 6 + 56, 1920);
    }

    #[test]
    fn migrates_previously_docked_orb_positions_to_the_screen_edge() {
        assert_eq!(
            migrate_legacy_orb_dock_anchor(106, 106, 826, 94, 838, 1.0),
            Some(94)
        );
        assert_eq!(
            migrate_legacy_orb_dock_anchor(826, 106, 826, 94, 838, 1.0),
            Some(838)
        );
        assert_eq!(
            migrate_legacy_orb_dock_anchor(180, 106, 826, 94, 838, 1.0),
            None
        );
    }

    #[test]
    fn rejects_saved_main_window_positions_outside_the_current_monitors() {
        assert!(!position_intersects_monitor(
            SavedPosition {
                x: -32000,
                y: -32000
            },
            960,
            680,
            0,
            0,
            1920,
            1080
        ));
        assert!(position_intersects_monitor(
            SavedPosition { x: 120, y: 80 },
            960,
            680,
            0,
            0,
            1920,
            1080
        ));
    }

    #[test]
    fn pinned_window_only_keeps_the_pin_button_interactive() {
        assert!(is_floating_pin_hit(225, 18, 0, 0, 308, 174));
        assert!(!is_floating_pin_hit(100, 80, 0, 0, 308, 174));
        assert!(!is_floating_pin_hit(270, 18, 0, 0, 308, 174));
    }

    #[test]
    fn normalizes_local_proxy_addresses() {
        assert_eq!(
            normalize_proxy_address("127.0.0.1:7890").unwrap(),
            "http://127.0.0.1:7890"
        );
        assert!(normalize_proxy_address("").is_err());
        assert!(normalize_proxy_address("ftp://127.0.0.1:21").is_err());
    }

    #[test]
    fn finds_an_installed_codex_binary() {
        assert!(!find_codex_launcher(None)
            .unwrap()
            .program
            .as_os_str()
            .is_empty());
    }

    #[test]
    #[ignore = "requires a live Codex account and network access"]
    fn reads_rate_limits_from_local_codex_session() {
        assert!(read_codex_usage(
            None,
            NetworkSettings {
                mode: "system".to_owned(),
                address: String::new(),
            }
        )
        .unwrap()
        .0
        .primary
        .is_some());
    }
}
