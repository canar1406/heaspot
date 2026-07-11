//! Windows Services (`!`): tìm service, xem trạng thái.
//! Start/Stop/Restart nằm ở context menu (system::service_action, cần admin).

use serde::Serialize;

#[derive(Serialize)]
pub struct ServiceInfo {
    pub name: String,
    pub display: String,
    pub status: String,
}

#[tauri::command]
pub async fn list_services(query: String) -> Result<Vec<ServiceInfo>, String> {
    let q = query.trim().to_string();

    let script = r#"
try {
  $f = $env:WINSPOT_SVC_FILTER
  $list = Get-Service | Where-Object { $f -eq "" -or $_.Name -like "*$f*" -or $_.DisplayName -like "*$f*" } |
    Select-Object -First 20 | ForEach-Object {
      [pscustomobject]@{ name = $_.Name; display = $_.DisplayName; status = [string]$_.Status }
    }
  ConvertTo-Json -InputObject @($list) -Compress
} catch { '[]' }
"#;

    let stdout = tauri::async_runtime::spawn_blocking(move || {
        crate::commands::run_hidden_ps(script, &[("WINSPOT_SVC_FILTER", &q)])
    })
    .await
    .map_err(|e| e.to_string())??;

    let value: serde_json::Value =
        serde_json::from_str(stdout.trim()).unwrap_or(serde_json::json!([]));
    let items = match value {
        serde_json::Value::Array(a) => a,
        obj @ serde_json::Value::Object(_) => vec![obj],
        _ => vec![],
    };
    Ok(items
        .into_iter()
        .filter_map(|v| {
            let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
            let s = ServiceInfo {
                name: get("name"),
                display: get("display"),
                status: get("status"),
            };
            (!s.name.is_empty()).then_some(s)
        })
        .collect())
}
