//! Download manager for Nkore Browser.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadInfo {
    pub id: String,
    pub url: String,
    pub filename: String,
    pub save_path: String,
    pub total_bytes: u64,
    pub downloaded_bytes: u64,
    pub speed_bps: u64,
    pub eta_secs: Option<u64>,
    pub status: DownloadStatus,
    pub mime_type: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Pending,
    Downloading,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for DownloadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Downloading => write!(f, "downloading"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

pub enum DownloadCommand {
    Cancel,
}

pub struct DownloadManager {
    pub downloads: HashMap<String, DownloadInfo>,
    cancel_senders: HashMap<String, mpsc::Sender<DownloadCommand>>,
    client: Client,
    pub default_dir: PathBuf,
}

impl DownloadManager {
    pub fn new(default_dir: PathBuf) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (compatible; Nkore/0.1)")
            .build()
            .expect("Failed to create HTTP client");

        Self {
            downloads: HashMap::new(),
            cancel_senders: HashMap::new(),
            client,
            default_dir,
        }
    }

    pub fn filename_from_url(url: &str) -> String {
        url::Url::parse(url)
            .ok()
            .and_then(|u| {
                u.path_segments()
                    .and_then(|s| s.last())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| format!("download_{}", Uuid::new_v4().as_simple()))
    }

    pub fn start(&mut self, app: AppHandle, url: String, save_path: Option<String>) -> String {
        let id = Uuid::new_v4().to_string();
        let filename = Self::filename_from_url(&url);
        let path = save_path
            .map(PathBuf::from)
            .unwrap_or_else(|| self.default_dir.join(&filename));

        let info = DownloadInfo {
            id: id.clone(),
            url: url.clone(),
            filename: filename.clone(),
            save_path: path.to_string_lossy().to_string(),
            total_bytes: 0,
            downloaded_bytes: 0,
            speed_bps: 0,
            eta_secs: None,
            status: DownloadStatus::Pending,
            mime_type: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            completed_at: None,
            error: None,
        };

        self.downloads.insert(id.clone(), info);

        let (tx, rx) = mpsc::channel::<DownloadCommand>(4);
        self.cancel_senders.insert(id.clone(), tx);

        let client = self.client.clone();
        let id_clone = id.clone();

        tokio::spawn(async move {
            run_download(app, client, id_clone, url, path, rx).await;
        });

        id
    }

    pub fn cancel(&mut self, id: &str) -> bool {
        if let Some(tx) = self.cancel_senders.get(id) {
            let _ = tx.try_send(DownloadCommand::Cancel);
            if let Some(info) = self.downloads.get_mut(id) {
                info.status = DownloadStatus::Cancelled;
            }
            return true;
        }
        false
    }

    pub fn get_all(&self) -> Vec<DownloadInfo> {
        self.downloads.values().cloned().collect()
    }

    pub fn clear_finished(&mut self) {
        self.downloads.retain(|_, v| {
            matches!(v.status, DownloadStatus::Pending | DownloadStatus::Downloading)
        });
    }

    pub fn update_progress(&mut self, id: &str, downloaded: u64, total: u64, speed: u64) {
        if let Some(info) = self.downloads.get_mut(id) {
            info.downloaded_bytes = downloaded;
            info.total_bytes = total;
            info.speed_bps = speed;
            info.status = DownloadStatus::Downloading;
            info.eta_secs = if speed > 0 && total > downloaded {
                Some((total - downloaded) / speed)
            } else {
                None
            };
        }
    }

    pub fn set_status(&mut self, id: &str, status: DownloadStatus, error: Option<String>) {
        if let Some(info) = self.downloads.get_mut(id) {
            if matches!(status, DownloadStatus::Completed | DownloadStatus::Failed | DownloadStatus::Cancelled) {
                info.completed_at = Some(chrono::Utc::now().to_rfc3339());
                self.cancel_senders.remove(id);
            }
            info.status = status;
            info.error = error;
        }
    }
}

async fn run_download(
    app: AppHandle,
    client: Client,
    id: String,
    url: String,
    save_path: PathBuf,
    mut cancel_rx: mpsc::Receiver<DownloadCommand>,
) {
    if let Some(parent) = save_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            emit_status(&app, &id, "failed", Some(e.to_string()));
            return;
        }
    }

    let response = match client.get(&url).send().await {
        Ok(r) => r,
        Err(e) => {
            emit_status(&app, &id, "failed", Some(e.to_string()));
            return;
        }
    };

    if !response.status().is_success() {
        emit_status(&app, &id, "failed", Some(format!("HTTP {}", response.status())));
        return;
    }

    let total = response.content_length().unwrap_or(0);
    let mime = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());

    let _ = app.emit(
        "download://started",
        serde_json::json!({ "id": id, "totalBytes": total, "mimeType": mime }),
    );

    let mut file = match tokio::fs::File::create(&save_path).await {
        Ok(f) => f,
        Err(e) => {
            emit_status(&app, &id, "failed", Some(e.to_string()));
            return;
        }
    };

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    let mut last_emit = Instant::now();
    let mut bytes_since_last = 0u64;
    let mut speed: u64 = 0;

    loop {
        tokio::select! {
            cmd = cancel_rx.recv() => {
                match cmd {
                    Some(DownloadCommand::Cancel) => {
                        let _ = tokio::fs::remove_file(&save_path).await;
                        emit_status(&app, &id, "cancelled", None);
                        return;
                    }
                    None => {
                        // Channel closed unexpectedly; treat as failure rather than
                        // silently deleting the partial download.
                        log::warn!("Download {} command channel closed unexpectedly", id);
                        emit_status(&app, &id, "failed", Some("Download interrupted".to_string()));
                        return;
                    }
                }
            }
            chunk = stream.next() => {
                match chunk {
                    None => break,
                    Some(Err(e)) => {
                        emit_status(&app, &id, "failed", Some(e.to_string()));
                        return;
                    }
                    Some(Ok(bytes)) => {
                        use tokio::io::AsyncWriteExt;
                        if let Err(e) = file.write_all(&bytes).await {
                            emit_status(&app, &id, "failed", Some(e.to_string()));
                            return;
                        }
                        downloaded += bytes.len() as u64;
                        bytes_since_last += bytes.len() as u64;

                        let elapsed = last_emit.elapsed();
                        if elapsed >= Duration::from_millis(200) {
                            if elapsed.as_secs_f64() > 0.0 {
                                speed = (bytes_since_last as f64 / elapsed.as_secs_f64()) as u64;
                            }
                            bytes_since_last = 0;
                            last_emit = Instant::now();

                            let eta = if speed > 0 && total > downloaded {
                                Some((total - downloaded) / speed)
                            } else {
                                None
                            };

                            let _ = app.emit(
                                "download://progress",
                                serde_json::json!({
                                    "id": id,
                                    "downloaded": downloaded,
                                    "total": total,
                                    "speedBps": speed,
                                    "etaSecs": eta,
                                }),
                            );
                        }
                    }
                }
            }
        }
    }

    emit_status(&app, &id, "completed", None);
    let _ = app.emit(
        "download://completed",
        serde_json::json!({
            "id": id,
            "savePath": save_path.to_string_lossy(),
            "totalBytes": downloaded,
        }),
    );
}

fn emit_status(app: &AppHandle, id: &str, status: &str, error: Option<String>) {
    let _ = app.emit(
        "download://status",
        serde_json::json!({ "id": id, "status": status, "error": error }),
    );
}
