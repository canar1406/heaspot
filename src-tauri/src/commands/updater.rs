use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

#[derive(Clone, serde::Serialize)]
pub struct UpdateProgress {
    stage: String,
    detail: String,
    version: Option<String>,
    notes: String,
    percent: Option<u8>,
    available: bool,
    installing: bool,
    done: bool,
    success: bool,
}

static UPDATE_PROGRESS: OnceLock<Mutex<Option<UpdateProgress>>> = OnceLock::new();

fn state() -> &'static Mutex<Option<UpdateProgress>> {
    UPDATE_PROGRESS.get_or_init(|| Mutex::new(None))
}

fn publish(app: &AppHandle, progress: UpdateProgress, show: bool) {
    if let Ok(mut current) = state().lock() {
        *current = Some(progress.clone());
    }
    let _ = app.emit("update://progress", progress);
    if show {
        show_update_window(app);
    }
}

pub fn show_update_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("update-progress") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub fn get_update_progress() -> Option<UpdateProgress> {
    state().lock().ok().and_then(|value| value.clone())
}

fn configuration(app: &AppHandle) -> Result<crate::commands::settings::Settings, String> {
    let app_state = app.state::<crate::AppState>();
    let conn = app_state.db.lock().map_err(|error| error.to_string())?;
    Ok(crate::commands::settings::load(&conn))
}

async fn fetch_update(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let config = configuration(app)?;
    if config.update_endpoint.is_empty() || config.update_pubkey.is_empty() {
        return Err("Chưa cấu hình endpoint/public key cập nhật".into());
    }
    let endpoint = config
        .update_endpoint
        .parse()
        .map_err(|error| format!("Endpoint cập nhật không hợp lệ: {error}"))?;
    app.updater_builder()
        .pubkey(config.update_pubkey)
        .endpoints(vec![endpoint])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle, manual: bool) -> Result<String, String> {
    let config = configuration(&app)?;
    if config.update_endpoint.is_empty() || config.update_pubkey.is_empty() {
        return if manual {
            Err("Chưa cấu hình endpoint/public key cập nhật".into())
        } else {
            Ok("Auto-update chưa có kênh phát hành".into())
        };
    }
    if manual {
        publish(
            &app,
            UpdateProgress {
                stage: "Đang kiểm tra cập nhật".into(),
                detail: "Đang kết nối kênh phát hành và xác minh metadata có chữ ký.".into(),
                version: None,
                notes: String::new(),
                percent: None,
                available: false,
                installing: false,
                done: false,
                success: false,
            },
            true,
        );
    }
    match fetch_update(&app).await? {
        Some(update) => {
            let version = update.version.to_string();
            let notes = update.body.clone().unwrap_or_default();
            publish(
                &app,
                UpdateProgress {
                    stage: format!("Có bản cập nhật {version}"),
                    detail: "Gói chỉ được cài sau khi chữ ký khớp public key đã cấu hình.".into(),
                    version: Some(version.clone()),
                    notes,
                    percent: Some(0),
                    available: true,
                    installing: false,
                    done: false,
                    success: false,
                },
                true,
            );
            Ok(format!("Có bản cập nhật {version}"))
        }
        None => {
            if manual {
                publish(
                    &app,
                    UpdateProgress {
                        stage: "HeaSpot đã là bản mới nhất".into(),
                        detail: "Không có phiên bản mới hơn trên kênh phát hành đã cấu hình."
                            .into(),
                        version: None,
                        notes: String::new(),
                        percent: Some(100),
                        available: false,
                        installing: false,
                        done: true,
                        success: true,
                    },
                    true,
                );
            }
            Ok("✓ HeaSpot đã là bản mới nhất".into())
        }
    }
}

#[tauri::command]
pub async fn install_available_update(app: AppHandle) -> Result<(), String> {
    let Some(update) = fetch_update(&app).await? else {
        return Err("Bản cập nhật không còn khả dụng".into());
    };
    let version = update.version.to_string();
    let notes = update.body.clone().unwrap_or_default();
    publish(
        &app,
        UpdateProgress {
            stage: format!("Đang tải HeaSpot {version}"),
            detail: "Có thể tiếp tục dùng launcher; cửa sổ này theo dõi tải và xác minh gói."
                .into(),
            version: Some(version.clone()),
            notes: notes.clone(),
            percent: Some(0),
            available: true,
            installing: true,
            done: false,
            success: false,
        },
        true,
    );

    let progress_app = app.clone();
    let finished_app = app.clone();
    let progress_version = version.clone();
    let progress_notes = notes.clone();
    let finished_version = version.clone();
    let finished_notes = notes.clone();
    let mut downloaded = 0u64;
    update
        .download_and_install(
            move |chunk_length, content_length| {
                downloaded = downloaded.saturating_add(chunk_length as u64);
                let percent = content_length
                    .filter(|total| *total > 0)
                    .map(|total| ((downloaded.saturating_mul(100) / total) as u8).min(99));
                publish(
                    &progress_app,
                    UpdateProgress {
                        stage: format!("Đang tải HeaSpot {progress_version}"),
                        detail: format!("Đã tải {} KB", downloaded / 1024),
                        version: Some(progress_version.clone()),
                        notes: progress_notes.clone(),
                        percent,
                        available: true,
                        installing: true,
                        done: false,
                        success: false,
                    },
                    false,
                );
            },
            move || {
                publish(
                    &finished_app,
                    UpdateProgress {
                        stage: "Đã tải xong — đang cài đặt".into(),
                        detail: "Chữ ký hợp lệ. Windows đang thay thế bản cũ và sẽ khởi động lại HeaSpot.".into(),
                        version: Some(finished_version.clone()),
                        notes: finished_notes.clone(),
                        percent: Some(100),
                        available: true,
                        installing: true,
                        done: false,
                        success: true,
                    },
                    false,
                );
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    app.restart();
}

pub fn spawn_auto_check(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(10));
        let enabled = configuration(&app)
            .map(|settings| settings.auto_update)
            .unwrap_or(false);
        if enabled {
            let _ = tauri::async_runtime::block_on(check_for_updates(app, false));
        }
    });
}
