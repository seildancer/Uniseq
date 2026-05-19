use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::Hasher;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};

use serde::{Deserialize, Serialize};

const UNISEQ_DIR: &str = "uniseq";
pub const SYNC_CONFIG_FILE_NAME: &str = "sync-config.json";
pub const SYNC_AUTH_FILE_NAME: &str = "sync-auth.json";
const SYNC_STATE_FILE_NAME: &str = "sync-state.json";
const PAGE_TRANSACTION_DIR_NAME: &str = ".uniseq-page-transaction";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncProviderKind {
    Uniseq,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncConfig {
    pub enabled: bool,
    pub provider: SyncProviderKind,
    pub sync_root_url: String,
    #[serde(default)]
    pub remote_workspace_id: String,
    #[serde(default)]
    pub remote_workspace_name: String,
    #[serde(default)]
    pub auth: SyncAuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncAuthConfig {
    #[serde(default)]
    pub kind: SyncAuthKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SyncAuthSecrets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bearer_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supabase_publishable_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SyncAuthKind {
    #[default]
    None,
    Bearer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncServiceDiscovery {
    pub version: u32,
    pub auth: SyncServiceAuth,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncServiceAuth {
    None,
    Bearer {
        #[serde(default)]
        login_url: Option<String>,
        #[serde(default)]
        token_url: Option<String>,
        #[serde(default)]
        instructions: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncState {
    #[serde(default)]
    pub files: BTreeMap<String, SyncFileState>,
    #[serde(default)]
    pub conflicts: BTreeMap<String, SyncConflictState>,
    pub last_synced_at: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncFileState {
    pub remote_version: Option<String>,
    pub local_hash: String,
    pub size: u64,
    pub last_synced_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncConflictState {
    pub path: String,
    pub local_hash: Option<String>,
    pub remote_version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteFileMeta {
    pub path: String,
    pub remote_version: String,
    pub size: u64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RemoteWorkspace {
    pub id: String,
    pub name: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFileBlob {
    pub meta: RemoteFileMeta,
    pub content: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushFileRequest {
    pub path: String,
    pub content: Vec<u8>,
    pub base_remote_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteFileRequest {
    pub path: String,
    pub base_remote_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PushResult {
    Accepted {
        remote_version: String,
        updated_at: Option<String>,
    },
    Conflict {
        current: RemoteFileMeta,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatusKind {
    Off,
    Ready,
    Syncing,
    Synced,
    Conflict,
    Error,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncStatus {
    pub kind: SyncStatusKind,
    pub enabled: bool,
    pub background_loop_running: bool,
    pub provider: Option<SyncProviderKind>,
    pub sync_root_url: Option<String>,
    pub remote_workspace_id: Option<String>,
    pub remote_workspace_name: Option<String>,
    pub remote_workspace_url: Option<String>,
    pub auth: Option<SyncStatusAuth>,
    pub last_synced_at: Option<u64>,
    pub last_error: Option<String>,
    pub conflicts: Vec<SyncConflictState>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncStatusAuth {
    pub kind: SyncAuthKind,
    pub has_bearer_token: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncRunSummary {
    pub pushed: usize,
    pub pulled: usize,
    pub deleted_local: usize,
    pub deleted_remote: usize,
    pub conflicts: Vec<SyncConflictState>,
    pub status: SyncStatus,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncProgressOperation {
    InitialPull,
    Sync,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncProgressPhase {
    Waiting,
    Listing,
    ScanningLocal,
    Comparing,
    Pulling,
    Pushing,
    DeletingLocal,
    DeletingRemote,
    Finalizing,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncProgress {
    pub operation: SyncProgressOperation,
    pub phase: SyncProgressPhase,
    pub current: usize,
    pub total: usize,
    pub path: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SyncConflictDetail {
    pub path: String,
    pub local_content: String,
    pub remote_content: String,
    pub remote_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncConflictResolution {
    UseLocal,
    UseRemote,
}

#[derive(Debug, Clone)]
pub struct SyncError {
    message: String,
    pub auth_expired: bool,
    pub remote_missing: bool,
}

type SyncResult<T> = Result<T, SyncError>;

pub trait SyncProvider {
    fn list(&self) -> SyncResult<Vec<RemoteFileMeta>>;
    fn pull(&self, path: &str) -> SyncResult<RemoteFileBlob>;
    fn push(&self, request: PushFileRequest) -> SyncResult<PushResult>;
    fn delete(&self, request: DeleteFileRequest) -> SyncResult<PushResult>;
}

pub struct HttpSyncProvider {
    sync_root_url: String,
    remote_workspace_id: Option<String>,
    bearer_token: Option<String>,
    client: reqwest::blocking::Client,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum PushResponse {
    Accepted {
        remote_version: String,
        updated_at: Option<String>,
    },
    Conflict {
        current: RemoteFileMeta,
    },
}

#[derive(Debug, Deserialize)]
struct PullMetaResponse {
    path: String,
    remote_version: String,
    size: Option<u64>,
    updated_at: Option<String>,
    content: Option<Vec<u8>>,
}

#[derive(Debug, Serialize)]
struct CreateWorkspaceBody<'a> {
    name: &'a str,
}

#[derive(Debug, Serialize)]
struct BaseVersionBody<'a> {
    base_remote_version: Option<&'a str>,
}

impl SyncConfig {
    pub fn new_with_auth(
        provider: SyncProviderKind,
        sync_root_url: String,
        remote_workspace_id: String,
        remote_workspace_name: String,
        auth: SyncAuthConfig,
    ) -> Self {
        Self {
            enabled: true,
            provider,
            sync_root_url: sync_root_url.trim().trim_end_matches('/').to_owned(),
            remote_workspace_id,
            remote_workspace_name,
            auth,
        }
    }
}

impl Default for SyncAuthConfig {
    fn default() -> Self {
        Self {
            kind: SyncAuthKind::None,
        }
    }
}

impl Default for SyncServiceDiscovery {
    fn default() -> Self {
        Self {
            version: 1,
            auth: SyncServiceAuth::None,
        }
    }
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            conflicts: BTreeMap::new(),
            last_synced_at: None,
            last_error: None,
        }
    }
}

impl SyncError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            auth_expired: false,
            remote_missing: false,
        }
    }

    pub fn new_auth_expired(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            auth_expired: true,
            remote_missing: false,
        }
    }

    pub fn new_remote_missing(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            auth_expired: false,
            remote_missing: true,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<std::io::Error> for SyncError {
    fn from(value: std::io::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl From<reqwest::Error> for SyncError {
    fn from(value: reqwest::Error) -> Self {
        Self::new(value.to_string())
    }
}

impl HttpSyncProvider {
    pub fn new_account(sync_root_url: impl Into<String>) -> SyncResult<Self> {
        Self::new(
            sync_root_url,
            None,
            None,
            std::time::Duration::from_secs(30),
        )
    }

    pub fn new_account_with_auth(
        sync_root_url: impl Into<String>,
        bearer_token: Option<String>,
    ) -> SyncResult<Self> {
        Self::new(
            sync_root_url,
            None,
            bearer_token,
            std::time::Duration::from_secs(30),
        )
    }

    pub fn new_workspace_with_auth(
        sync_root_url: impl Into<String>,
        remote_workspace_id: impl Into<String>,
        bearer_token: Option<String>,
    ) -> SyncResult<Self> {
        Self::new_workspace_with_auth_timeout(
            sync_root_url,
            remote_workspace_id,
            bearer_token,
            std::time::Duration::from_secs(30),
        )
    }

    pub fn new_workspace_with_auth_timeout(
        sync_root_url: impl Into<String>,
        remote_workspace_id: impl Into<String>,
        bearer_token: Option<String>,
        timeout: Duration,
    ) -> SyncResult<Self> {
        let remote_workspace_id = remote_workspace_id.into();
        if remote_workspace_id.trim().is_empty() {
            return Err(SyncError::new("remote workspace is required"));
        }
        Self::new(
            sync_root_url,
            Some(remote_workspace_id),
            bearer_token,
            timeout,
        )
    }

    fn new(
        sync_root_url: impl Into<String>,
        remote_workspace_id: Option<String>,
        bearer_token: Option<String>,
        timeout: Duration,
    ) -> SyncResult<Self> {
        let sync_root_url = sync_root_url.into().trim().trim_end_matches('/').to_owned();
        if sync_root_url.is_empty() {
            return Err(SyncError::new("sync root URL cannot be empty"));
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(timeout)
            .build()?;
        Ok(Self {
            sync_root_url,
            remote_workspace_id: remote_workspace_id
                .map(|id| id.trim().trim_matches('/').to_owned())
                .filter(|id| !id.is_empty()),
            bearer_token: normalize_bearer_token(bearer_token),
            client,
        })
    }

    pub fn discover(sync_root_url: impl Into<String>) -> SyncResult<SyncServiceDiscovery> {
        let provider = Self::new_account(sync_root_url)?;
        let response = provider.client.get(provider.discovery_url()).send()?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(SyncServiceDiscovery::default());
        }
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "sync service discovery failed with status {}",
                response.status()
            )));
        }
        let body = response.text()?;
        let trimmed = body.trim();
        if trimmed.is_empty() || trimmed == "null" {
            return Ok(SyncServiceDiscovery::default());
        }
        Ok(serde_json::from_str(trimmed)?)
    }

    pub fn list_workspaces(&self) -> SyncResult<Vec<RemoteWorkspace>> {
        let response = self
            .with_auth(self.client.get(self.workspaces_url()))
            .send()?;
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "remote workspace list failed with status {}",
                response.status()
            )));
        }
        Ok(response.json()?)
    }

    pub fn create_workspace(&self, name: &str) -> SyncResult<RemoteWorkspace> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(SyncError::new("workspace name cannot be empty"));
        }
        let response = self
            .with_auth(self.client.post(self.workspaces_url()))
            .json(&CreateWorkspaceBody { name: trimmed })
            .send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if response.status() == reqwest::StatusCode::CONFLICT {
            return Err(SyncError::new("remote workspace already exists"));
        }
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "remote workspace create failed with status {}",
                response.status()
            )));
        }
        Ok(response.json()?)
    }

    pub fn delete_workspace(&self, workspace_id: &str) -> SyncResult<()> {
        let trimmed = workspace_id.trim();
        if trimmed.is_empty() {
            return Err(SyncError::new("remote workspace is required"));
        }
        let response = self
            .with_auth(
                self.client
                    .delete(Self::workspace_url_for(&self.sync_root_url, trimmed)),
            )
            .send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(SyncError::new("remote workspace not found"));
        }
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "remote workspace delete failed with status {}",
                response.status()
            )));
        }
        Ok(())
    }

    pub fn delete_account(&self) -> SyncResult<()> {
        let response = self
            .with_auth(self.client.delete(self.account_url()))
            .send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(SyncError::new("remote account not found"));
        }
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "remote account delete failed with status {}",
                response.status()
            )));
        }
        Ok(())
    }

    pub fn workspace_url_for(sync_root_url: &str, remote_workspace_id: &str) -> String {
        format!(
            "{}/workspaces/{}",
            sync_root_url.trim().trim_end_matches('/'),
            encode_path(remote_workspace_id.trim().trim_matches('/'))
        )
    }

    fn workspaces_url(&self) -> String {
        format!("{}/workspaces", self.sync_root_url)
    }

    fn account_url(&self) -> String {
        self.sync_root_url.clone()
    }

    fn discovery_url(&self) -> String {
        format!("{}/.well-known/uniseq-sync", self.sync_root_url)
    }

    fn workspace_url(&self) -> SyncResult<String> {
        let workspace_id = self
            .remote_workspace_id
            .as_deref()
            .ok_or_else(|| SyncError::new("remote workspace is required"))?;
        Ok(Self::workspace_url_for(&self.sync_root_url, workspace_id))
    }

    fn files_url(&self) -> SyncResult<String> {
        Ok(format!("{}/files", self.workspace_url()?))
    }

    fn file_url(&self, path: &str) -> SyncResult<String> {
        Ok(format!(
            "{}/files/{}",
            self.workspace_url()?,
            encode_path(path)
        ))
    }

    fn with_auth(
        &self,
        builder: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        match self.bearer_token.as_deref() {
            Some(token) => builder.bearer_auth(token),
            None => builder,
        }
    }
}

impl SyncProvider for HttpSyncProvider {
    fn list(&self) -> SyncResult<Vec<RemoteFileMeta>> {
        let response = self.with_auth(self.client.get(self.files_url()?)).send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(SyncError::new_remote_missing("remote workspace not found"));
        }
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "remote list failed with status {}",
                response.status()
            )));
        }
        Ok(response.json()?)
    }

    fn pull(&self, path: &str) -> SyncResult<RemoteFileBlob> {
        let response = self
            .with_auth(self.client.get(self.file_url(path)?))
            .send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if !response.status().is_success() {
            return Err(SyncError::new(format!(
                "remote pull failed for '{path}' with status {}",
                response.status()
            )));
        }

        let headers = response.headers().clone();
        let content_type = headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");

        if content_type.contains("application/json") {
            let payload: PullMetaResponse = response.json()?;
            let content = payload.content.unwrap_or_default();
            let size = payload.size.unwrap_or(content.len() as u64);
            return Ok(RemoteFileBlob {
                meta: RemoteFileMeta {
                    path: payload.path,
                    remote_version: payload.remote_version,
                    size,
                    updated_at: payload.updated_at,
                },
                content,
            });
        }

        let remote_version = header_string(&headers, "x-uniseq-remote-version")
            .or_else(|| header_string(&headers, "etag"))
            .ok_or_else(|| SyncError::new("remote pull response missing remote version"))?;
        let updated_at = header_string(&headers, "x-uniseq-updated-at");
        let content = response.bytes()?.to_vec();
        Ok(RemoteFileBlob {
            meta: RemoteFileMeta {
                path: path.to_owned(),
                remote_version,
                size: content.len() as u64,
                updated_at,
            },
            content,
        })
    }

    fn push(&self, request: PushFileRequest) -> SyncResult<PushResult> {
        let mut builder = self.with_auth(self.client.put(self.file_url(&request.path)?));
        if let Some(base) = request.base_remote_version.as_deref() {
            builder = builder.header("x-uniseq-base-remote-version", base);
        }
        let response = builder.body(request.content).send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if !response.status().is_success() && response.status() != reqwest::StatusCode::CONFLICT {
            return Err(SyncError::new(format!(
                "remote push failed for '{}' with status {}",
                request.path,
                response.status()
            )));
        }
        push_response_from_response(response)
    }

    fn delete(&self, request: DeleteFileRequest) -> SyncResult<PushResult> {
        let mut builder = self.with_auth(self.client.delete(self.file_url(&request.path)?));
        if let Some(base) = request.base_remote_version.as_deref() {
            builder = builder.header("x-uniseq-base-remote-version", base);
        }
        let response = builder
            .json(&BaseVersionBody {
                base_remote_version: request.base_remote_version.as_deref(),
            })
            .send()?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(SyncError::new_auth_expired("sync token expired"));
        }
        if !response.status().is_success() && response.status() != reqwest::StatusCode::CONFLICT {
            return Err(SyncError::new(format!(
                "remote delete failed for '{}' with status {}",
                request.path,
                response.status()
            )));
        }
        push_response_from_response(response)
    }
}

#[derive(Serialize)]
struct RefreshBody<'a> {
    refresh_token: &'a str,
}

#[derive(Deserialize)]
struct SupabaseTokenResponse {
    access_token: Option<String>,
    refresh_token: Option<String>,
}

pub fn refresh_supabase_auth(
    sync_root_url: &str,
    secrets: &SyncAuthSecrets,
) -> SyncResult<SyncAuthSecrets> {
    let refresh_token = secrets
        .refresh_token
        .as_deref()
        .ok_or_else(|| SyncError::new("session expired — please sign in again"))?;
    let publishable_key = secrets
        .supabase_publishable_key
        .as_deref()
        .ok_or_else(|| SyncError::new("session expired — please sign in again"))?;
    let supabase_url = supabase_url_from_sync_root(sync_root_url)
        .ok_or_else(|| SyncError::new("cannot determine Supabase URL from sync root"))?;

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let response = client
        .post(format!(
            "{}/auth/v1/token?grant_type=refresh_token",
            supabase_url
        ))
        .header("apikey", publishable_key)
        .header("Content-Type", "application/json")
        .json(&RefreshBody { refresh_token })
        .send()?;
    if !response.status().is_success() {
        return Err(SyncError::new(format!(
            "token refresh failed with status {} — please sign in again",
            response.status()
        )));
    }
    let data: SupabaseTokenResponse = response.json()?;
    let new_access = data
        .access_token
        .ok_or_else(|| SyncError::new("token refresh response missing access_token"))?;
    Ok(SyncAuthSecrets {
        bearer_token: Some(new_access),
        refresh_token: data.refresh_token.or_else(|| secrets.refresh_token.clone()),
        supabase_publishable_key: secrets.supabase_publishable_key.clone(),
    })
}

fn supabase_url_from_sync_root(sync_root_url: &str) -> Option<String> {
    let trimmed = sync_root_url.trim();
    let after_scheme = trimmed.find("://").map(|i| i + 3)?;
    let host_end = trimmed[after_scheme..]
        .find('/')
        .map(|i| after_scheme + i)
        .unwrap_or(trimmed.len());
    Some(trimmed[..host_end].to_owned())
}

pub fn read_sync_config(root: &Path) -> SyncResult<Option<SyncConfig>> {
    let path = sync_config_path(root);
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(Some(serde_json::from_str(&contents)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn write_sync_config(root: &Path, config: &SyncConfig) -> SyncResult<()> {
    write_json(&sync_config_path(root), config)
}

pub fn read_sync_auth_secrets(root: &Path) -> SyncResult<SyncAuthSecrets> {
    let path = sync_auth_path(root);
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(serde_json::from_str(&contents)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(SyncAuthSecrets::default())
        }
        Err(error) => Err(error.into()),
    }
}

pub fn write_sync_auth_secrets(root: &Path, secrets: &SyncAuthSecrets) -> SyncResult<()> {
    write_json(&sync_auth_path(root), secrets)
}

pub fn read_sync_state(root: &Path) -> SyncResult<SyncState> {
    let path = sync_state_path(root);
    match fs::read_to_string(&path) {
        Ok(contents) => Ok(serde_json::from_str(&contents)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SyncState::default()),
        Err(error) => Err(error.into()),
    }
}

pub fn write_sync_state(root: &Path, state: &SyncState) -> SyncResult<()> {
    write_json(&sync_state_path(root), state)
}

pub fn sync_status(root: &Path) -> SyncResult<SyncStatus> {
    let config = read_sync_config(root)?;
    let state = read_sync_state(root)?;
    let secrets = read_sync_auth_secrets(root)?;
    Ok(status_from_config_and_state(
        config.as_ref(),
        &state,
        &secrets,
        false,
    ))
}

pub fn clear_sync_metadata(root: &Path) -> SyncResult<()> {
    remove_sync_file_if_exists(&sync_config_path(root))?;
    remove_sync_file_if_exists(&sync_auth_path(root))?;
    remove_sync_file_if_exists(&sync_state_path(root))?;
    Ok(())
}

pub fn disconnect_deleted_remote(root: &Path) -> SyncResult<bool> {
    let Some(config) = read_sync_config(root)? else {
        return Ok(false);
    };
    let secrets = read_sync_auth_secrets(root)?;
    let provider = match HttpSyncProvider::new_workspace_with_auth_timeout(
        &config.sync_root_url,
        &config.remote_workspace_id,
        secrets.bearer_token,
        Duration::from_secs(3),
    ) {
        Ok(provider) => provider,
        Err(_) => return Ok(false),
    };
    disconnect_deleted_remote_with_provider(root, &provider)
}

pub fn initial_pull_with_progress<F>(
    root: &Path,
    provider: &dyn SyncProvider,
    mut progress: F,
) -> SyncResult<SyncRunSummary>
where
    F: FnMut(SyncProgress),
{
    fs::create_dir_all(root)?;
    progress(SyncProgress {
        operation: SyncProgressOperation::InitialPull,
        phase: SyncProgressPhase::Listing,
        current: 0,
        total: 0,
        path: None,
        detail: Some("Listing remote files".to_owned()),
    });
    let remote_files = provider.list()?;
    let total = remote_files.len();
    progress(SyncProgress {
        operation: SyncProgressOperation::InitialPull,
        phase: SyncProgressPhase::Pulling,
        current: 0,
        total,
        path: None,
        detail: Some(format!("Downloading {total} remote files")),
    });
    for (index, meta) in remote_files.iter().enumerate() {
        let blob = provider.pull(&meta.path)?;
        write_workspace_file(root, &meta.path, &blob.content)?;
        progress(SyncProgress {
            operation: SyncProgressOperation::InitialPull,
            phase: SyncProgressPhase::Pulling,
            current: index + 1,
            total,
            path: Some(meta.path.clone()),
            detail: Some(format!("Downloaded {} of {total} files", index + 1)),
        });
    }
    let remote_paths = remote_files
        .iter()
        .map(|meta| meta.path.clone())
        .collect::<BTreeSet<_>>();
    for local_path in scan_local_files(root)?.into_keys() {
        if !remote_paths.contains(&local_path) {
            remove_workspace_file(root, &local_path)?;
        }
    }
    let local_files = scan_local_files(root)?;
    let now = now_unix_secs();
    let mut state = SyncState::default();
    for meta in remote_files {
        if let Some(local) = local_files.get(&meta.path) {
            state.files.insert(
                meta.path.clone(),
                SyncFileState {
                    remote_version: Some(meta.remote_version),
                    local_hash: local.hash.clone(),
                    size: local.size,
                    last_synced_at: now,
                },
            );
        }
    }
    state.last_synced_at = Some(now);
    progress(SyncProgress {
        operation: SyncProgressOperation::InitialPull,
        phase: SyncProgressPhase::Finalizing,
        current: total,
        total,
        path: None,
        detail: Some("Finalizing remote workspace".to_owned()),
    });
    write_sync_state(root, &state)?;
    Ok(SyncRunSummary {
        pushed: 0,
        pulled: state.files.len(),
        deleted_local: 0,
        deleted_remote: 0,
        conflicts: Vec::new(),
        status: status_from_config_and_state(
            read_sync_config(root)?.as_ref(),
            &state,
            &read_sync_auth_secrets(root)?,
            false,
        ),
    })
}

pub fn sync_once(root: &Path, provider: &dyn SyncProvider) -> SyncResult<SyncRunSummary> {
    sync_once_with_progress(root, provider, |_| {})
}

pub fn sync_once_with_progress<F>(
    root: &Path,
    provider: &dyn SyncProvider,
    mut progress: F,
) -> SyncResult<SyncRunSummary>
where
    F: FnMut(SyncProgress),
{
    let config = read_sync_config(root)?;
    if !config.as_ref().is_some_and(|config| config.enabled) {
        let state = read_sync_state(root)?;
        return Ok(SyncRunSummary {
            pushed: 0,
            pulled: 0,
            deleted_local: 0,
            deleted_remote: 0,
            conflicts: state.conflicts.values().cloned().collect(),
            status: status_from_config_and_state(
                config.as_ref(),
                &state,
                &read_sync_auth_secrets(root)?,
                false,
            ),
        });
    }

    let mut state = read_sync_state(root)?;
    progress(SyncProgress {
        operation: SyncProgressOperation::Sync,
        phase: SyncProgressPhase::Listing,
        current: 0,
        total: 0,
        path: None,
        detail: Some("Listing remote files".to_owned()),
    });
    let remote_files = match provider.list() {
        Ok(files) => files,
        Err(error) => {
            state.last_error = Some(error.message().to_owned());
            write_sync_state(root, &state)?;
            return Err(error);
        }
    };
    let remote_by_path = remote_files
        .into_iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    let mut local_files = scan_local_files_with_progress(root, |current, total, path| {
        let detail = if total == 0 {
            "No local files to scan".to_owned()
        } else if current == 0 {
            format!("Scanning {total} local files")
        } else {
            format!("Scanned {current} of {total} local files")
        };
        progress(SyncProgress {
            operation: SyncProgressOperation::Sync,
            phase: SyncProgressPhase::ScanningLocal,
            current,
            total,
            path: path.map(str::to_owned),
            detail: Some(detail),
        });
    })?;
    let mut all_paths = BTreeSet::new();
    all_paths.extend(local_files.keys().cloned());
    all_paths.extend(remote_by_path.keys().cloned());
    all_paths.extend(state.files.keys().cloned());
    let all_paths = all_paths.into_iter().collect::<Vec<_>>();
    let total = all_paths.len();
    progress(SyncProgress {
        operation: SyncProgressOperation::Sync,
        phase: SyncProgressPhase::Comparing,
        current: 0,
        total,
        path: None,
        detail: Some(format!("Checking {total} workspace files")),
    });

    let now = now_unix_secs();
    let mut pushed = 0;
    let mut pulled = 0;
    let mut deleted_local = 0;
    let mut deleted_remote = 0;

    for (index, path) in all_paths.into_iter().enumerate() {
        let local = local_files.remove(&path);
        let remote = remote_by_path.get(&path);
        let previous = state.files.get(&path).cloned();
        let local_changed = match (&local, &previous) {
            (Some(local), Some(previous)) => local.hash != previous.local_hash,
            (Some(_), None) => true,
            (None, Some(_)) => true,
            (None, None) => false,
        };
        let remote_changed = match (remote, &previous) {
            (Some(remote), Some(previous)) => {
                previous.remote_version.as_deref() != Some(remote.remote_version.as_str())
            }
            (Some(_), None) => true,
            (None, Some(previous)) => previous.remote_version.is_some(),
            (None, None) => false,
        };

        let operation = match (&local, remote, &previous, local_changed, remote_changed) {
            (Some(_), None, _, true, false) | (Some(_), Some(_), _, true, false) => {
                Some((SyncProgressPhase::Pushing, "Uploading"))
            }
            (Some(_), Some(_), _, false, true) | (None, Some(_), _, false, true) => {
                Some((SyncProgressPhase::Pulling, "Downloading"))
            }
            (None, Some(_), Some(_), true, false) => {
                Some((SyncProgressPhase::DeletingRemote, "Removing remote"))
            }
            (Some(_), None, Some(_), false, true) => {
                Some((SyncProgressPhase::DeletingLocal, "Removing local"))
            }
            _ => None,
        };
        if let Some((phase, verb)) = operation {
            progress(SyncProgress {
                operation: SyncProgressOperation::Sync,
                phase,
                current: index,
                total,
                path: Some(path.clone()),
                detail: Some(format!("{verb} {}", path)),
            });
        }

        match (local, remote, previous, local_changed, remote_changed) {
            (Some(local), None, previous, true, false) => {
                let result = provider.push(PushFileRequest {
                    path: path.clone(),
                    content: fs::read(root.join(path_from_remote(&path)?))?,
                    base_remote_version: previous.and_then(|state| state.remote_version),
                })?;
                match result {
                    PushResult::Accepted { remote_version, .. } => {
                        pushed += 1;
                        state.conflicts.remove(&path);
                        state.files.insert(
                            path.clone(),
                            synced_state(Some(remote_version), &local, now),
                        );
                    }
                    PushResult::Conflict { current } => {
                        insert_conflict(
                            &mut state,
                            &path,
                            Some(local.hash),
                            Some(current.remote_version),
                        );
                    }
                }
            }
            (Some(local), Some(_remote), previous, true, false) => {
                let result = provider.push(PushFileRequest {
                    path: path.clone(),
                    content: fs::read(root.join(path_from_remote(&path)?))?,
                    base_remote_version: previous.and_then(|state| state.remote_version),
                })?;
                match result {
                    PushResult::Accepted { remote_version, .. } => {
                        pushed += 1;
                        state.conflicts.remove(&path);
                        state.files.insert(
                            path.clone(),
                            synced_state(Some(remote_version), &local, now),
                        );
                    }
                    PushResult::Conflict { current } => {
                        insert_conflict(
                            &mut state,
                            &path,
                            Some(local.hash),
                            Some(current.remote_version),
                        );
                    }
                }
            }
            (Some(_local), Some(remote), _previous, false, true) => {
                let blob = provider.pull(&path)?;
                write_workspace_file(root, &path, &blob.content)?;
                let updated = local_file_for_path(root, &path)?;
                pulled += 1;
                state.conflicts.remove(&path);
                state.files.insert(
                    path.clone(),
                    synced_state(Some(remote.remote_version.clone()), &updated, now),
                );
            }
            (None, Some(remote), _previous, false, true) => {
                let blob = provider.pull(&path)?;
                write_workspace_file(root, &path, &blob.content)?;
                let updated = local_file_for_path(root, &path)?;
                pulled += 1;
                state.conflicts.remove(&path);
                state.files.insert(
                    path.clone(),
                    synced_state(Some(remote.remote_version.clone()), &updated, now),
                );
            }
            (Some(local), Some(remote), _previous, true, true) => {
                insert_conflict(
                    &mut state,
                    &path,
                    Some(local.hash),
                    Some(remote.remote_version.clone()),
                );
            }
            (None, Some(remote), Some(_previous), true, false) => {
                let result = provider.delete(DeleteFileRequest {
                    path: path.clone(),
                    base_remote_version: Some(remote.remote_version.clone()),
                })?;
                match result {
                    PushResult::Accepted { .. } => {
                        deleted_remote += 1;
                        state.conflicts.remove(&path);
                        state.files.remove(&path);
                    }
                    PushResult::Conflict { current } => {
                        insert_conflict(&mut state, &path, None, Some(current.remote_version));
                    }
                }
            }
            (Some(_local), None, Some(_previous), false, true) => {
                remove_workspace_file(root, &path)?;
                deleted_local += 1;
                state.conflicts.remove(&path);
                state.files.remove(&path);
            }
            (Some(local), Some(remote), _previous, false, false) => {
                state.files.insert(
                    path.clone(),
                    synced_state(Some(remote.remote_version.clone()), &local, now),
                );
            }
            (None, None, Some(_previous), true, true)
            | (None, None, Some(_previous), true, false) => {
                state.conflicts.remove(&path);
                state.files.remove(&path);
            }
            _ => {}
        }

        progress(SyncProgress {
            operation: SyncProgressOperation::Sync,
            phase: SyncProgressPhase::Comparing,
            current: index + 1,
            total,
            path: Some(path.clone()),
            detail: Some(format!("Processed {} of {total} files", index + 1)),
        });
    }

    state.last_synced_at = Some(now);
    state.last_error = None;
    let conflicts = state.conflicts.values().cloned().collect::<Vec<_>>();
    progress(SyncProgress {
        operation: SyncProgressOperation::Sync,
        phase: SyncProgressPhase::Finalizing,
        current: total,
        total,
        path: None,
        detail: Some("Finalizing sync".to_owned()),
    });
    write_sync_state(root, &state)?;
    Ok(SyncRunSummary {
        pushed,
        pulled,
        deleted_local,
        deleted_remote,
        conflicts,
        status: status_from_config_and_state(
            config.as_ref(),
            &state,
            &read_sync_auth_secrets(root)?,
            false,
        ),
    })
}

pub fn conflict_detail(
    root: &Path,
    provider: &dyn SyncProvider,
    path: &str,
) -> SyncResult<SyncConflictDetail> {
    let state = read_sync_state(root)?;
    let conflict = state
        .conflicts
        .get(path)
        .ok_or_else(|| SyncError::new("sync conflict not found"))?;
    let local_content = match fs::read(root.join(path_from_remote(path)?)) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    let remote = provider.pull(path)?;
    Ok(SyncConflictDetail {
        path: path.to_owned(),
        local_content,
        remote_content: String::from_utf8_lossy(&remote.content).to_string(),
        remote_version: conflict.remote_version.clone(),
    })
}

pub fn resolve_conflict(
    root: &Path,
    provider: &dyn SyncProvider,
    path: &str,
    resolution: SyncConflictResolution,
) -> SyncResult<SyncRunSummary> {
    let mut state = read_sync_state(root)?;
    let conflict = state
        .conflicts
        .get(path)
        .cloned()
        .ok_or_else(|| SyncError::new("sync conflict not found"))?;
    let now = now_unix_secs();

    match resolution {
        SyncConflictResolution::UseLocal => {
            let relative_path = path_from_remote(path)?;
            let content = fs::read(root.join(&relative_path))?;
            let local = LocalFileMeta {
                hash: hash_bytes(&content),
                size: content.len() as u64,
            };
            let result = provider.push(PushFileRequest {
                path: path.to_owned(),
                content,
                base_remote_version: conflict.remote_version,
            })?;
            match result {
                PushResult::Accepted { remote_version, .. } => {
                    state.conflicts.remove(path);
                    state.files.insert(
                        path.to_owned(),
                        synced_state(Some(remote_version), &local, now),
                    );
                }
                PushResult::Conflict { current } => {
                    insert_conflict(
                        &mut state,
                        path,
                        Some(local.hash),
                        Some(current.remote_version),
                    );
                }
            }
        }
        SyncConflictResolution::UseRemote => {
            let blob = provider.pull(path)?;
            write_workspace_file(root, path, &blob.content)?;
            let local = local_file_for_path(root, path)?;
            state.conflicts.remove(path);
            state.files.insert(
                path.to_owned(),
                synced_state(Some(blob.meta.remote_version), &local, now),
            );
        }
    }

    state.last_synced_at = Some(now);
    state.last_error = None;
    write_sync_state(root, &state)?;
    Ok(SyncRunSummary {
        pushed: usize::from(matches!(resolution, SyncConflictResolution::UseLocal)),
        pulled: usize::from(matches!(resolution, SyncConflictResolution::UseRemote)),
        deleted_local: 0,
        deleted_remote: 0,
        conflicts: state.conflicts.values().cloned().collect(),
        status: status_from_config_and_state(
            read_sync_config(root)?.as_ref(),
            &state,
            &read_sync_auth_secrets(root)?,
            false,
        ),
    })
}

pub fn sync_config_path(root: &Path) -> PathBuf {
    root.join(UNISEQ_DIR).join(SYNC_CONFIG_FILE_NAME)
}

fn sync_auth_path(root: &Path) -> PathBuf {
    root.join(UNISEQ_DIR).join(SYNC_AUTH_FILE_NAME)
}

fn sync_state_path(root: &Path) -> PathBuf {
    root.join(UNISEQ_DIR).join(SYNC_STATE_FILE_NAME)
}

fn push_response_from_response(response: reqwest::blocking::Response) -> SyncResult<PushResult> {
    if response.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(PushResult::Accepted {
            remote_version: now_unix_secs().to_string(),
            updated_at: None,
        });
    }
    let payload: PushResponse = response.json()?;
    Ok(match payload {
        PushResponse::Accepted {
            remote_version,
            updated_at,
        } => PushResult::Accepted {
            remote_version,
            updated_at,
        },
        PushResponse::Conflict { current } => PushResult::Conflict { current },
    })
}

fn remove_sync_file_if_exists(path: &Path) -> SyncResult<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn disconnect_deleted_remote_with_provider(
    root: &Path,
    provider: &dyn SyncProvider,
) -> SyncResult<bool> {
    match provider.list() {
        Ok(_) => Ok(false),
        Err(error) if error.remote_missing => {
            clear_sync_metadata(root)?;
            Ok(true)
        }
        Err(_) => Ok(false),
    }
}

fn status_from_config_and_state(
    config: Option<&SyncConfig>,
    state: &SyncState,
    secrets: &SyncAuthSecrets,
    syncing: bool,
) -> SyncStatus {
    let Some(config) = config else {
        return SyncStatus {
            kind: SyncStatusKind::Off,
            enabled: false,
            background_loop_running: false,
            provider: None,
            sync_root_url: None,
            remote_workspace_id: None,
            remote_workspace_name: None,
            remote_workspace_url: None,
            auth: None,
            last_synced_at: state.last_synced_at,
            last_error: state.last_error.clone(),
            conflicts: Vec::new(),
        };
    };

    let conflicts = state.conflicts.values().cloned().collect::<Vec<_>>();
    let kind = if !config.enabled {
        SyncStatusKind::Off
    } else if syncing {
        SyncStatusKind::Syncing
    } else if !conflicts.is_empty() {
        SyncStatusKind::Conflict
    } else if state.last_error.is_some() {
        SyncStatusKind::Error
    } else if state.last_synced_at.is_some() {
        SyncStatusKind::Synced
    } else {
        SyncStatusKind::Ready
    };

    SyncStatus {
        kind,
        enabled: config.enabled,
        background_loop_running: false,
        provider: Some(config.provider.clone()),
        sync_root_url: Some(config.sync_root_url.clone()),
        remote_workspace_id: Some(config.remote_workspace_id.clone()),
        remote_workspace_name: Some(config.remote_workspace_name.clone()),
        remote_workspace_url: Some(HttpSyncProvider::workspace_url_for(
            &config.sync_root_url,
            &config.remote_workspace_id,
        )),
        auth: Some(SyncStatusAuth {
            kind: config.auth.kind.clone(),
            has_bearer_token: secrets.bearer_token.is_some(),
        }),
        last_synced_at: state.last_synced_at,
        last_error: state.last_error.clone(),
        conflicts,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalFileMeta {
    hash: String,
    size: u64,
}

fn scan_local_files(root: &Path) -> SyncResult<BTreeMap<String, LocalFileMeta>> {
    scan_local_files_with_progress(root, |_, _, _| {})
}

fn scan_local_files_with_progress<F>(
    root: &Path,
    mut progress: F,
) -> SyncResult<BTreeMap<String, LocalFileMeta>>
where
    F: FnMut(usize, usize, Option<&str>),
{
    let entries = collect_local_file_entries(root)?;
    let total = entries.len();
    let mut files = BTreeMap::new();
    progress(0, total, None);
    for (index, entry) in entries.iter().enumerate() {
        let content = fs::read(&entry.local_path)?;
        files.insert(
            entry.remote_path.clone(),
            LocalFileMeta {
                hash: hash_bytes(&content),
                size: content.len() as u64,
            },
        );
        progress(index + 1, total, Some(entry.remote_path.as_str()));
    }
    Ok(files)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocalFileEntry {
    local_path: PathBuf,
    remote_path: String,
}

fn collect_local_file_entries(root: &Path) -> SyncResult<Vec<LocalFileEntry>> {
    let mut entries = Vec::new();
    collect_local_file_entries_inner(root, root, &mut entries)?;
    entries.sort_by(|left, right| left.remote_path.cmp(&right.remote_path));
    Ok(entries)
}

fn collect_local_file_entries_inner(
    root: &Path,
    current: &Path,
    entries: &mut Vec<LocalFileEntry>,
) -> SyncResult<()> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root).map_err(|_| {
            SyncError::new(format!("path is outside workspace: {}", path.display()))
        })?;
        if should_skip_path(relative) {
            continue;
        }
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_local_file_entries_inner(root, &path, entries)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let remote_path = remote_path_from_relative(relative)?;
        entries.push(LocalFileEntry {
            local_path: path,
            remote_path,
        });
    }
    Ok(())
}

fn local_file_for_path(root: &Path, path: &str) -> SyncResult<LocalFileMeta> {
    let content = fs::read(root.join(path_from_remote(path)?))?;
    Ok(LocalFileMeta {
        hash: hash_bytes(&content),
        size: content.len() as u64,
    })
}

fn should_skip_path(relative: &Path) -> bool {
    let mut components = relative.components();
    let first = components.next().and_then(component_name);
    if first == Some(PAGE_TRANSACTION_DIR_NAME) {
        return true;
    }
    if first == Some(UNISEQ_DIR) {
        let second = components.next().and_then(component_name);
        if matches!(
            second,
            Some(SYNC_CONFIG_FILE_NAME | SYNC_AUTH_FILE_NAME | SYNC_STATE_FILE_NAME)
        ) {
            return true;
        }
    }
    relative
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.contains(".uniseq-txn-") || name.ends_with(".tmp"))
}

fn component_name(component: Component<'_>) -> Option<&str> {
    match component {
        Component::Normal(name) => name.to_str(),
        _ => None,
    }
}

fn remote_path_from_relative(path: &Path) -> SyncResult<String> {
    let mut segments = Vec::new();
    for component in path.components() {
        let Component::Normal(segment) = component else {
            return Err(SyncError::new(
                "workspace path contains unsupported component",
            ));
        };
        let segment = segment
            .to_str()
            .ok_or_else(|| SyncError::new("workspace path is not valid UTF-8"))?;
        segments.push(segment.to_owned());
    }
    Ok(segments.join("/"))
}

fn path_from_remote(path: &str) -> SyncResult<PathBuf> {
    let mut output = PathBuf::new();
    for segment in path.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return Err(SyncError::new(format!("invalid remote path '{path}'")));
        }
        if segment.contains('\\') {
            return Err(SyncError::new(format!("invalid remote path '{path}'")));
        }
        output.push(segment);
    }
    Ok(output)
}

fn write_workspace_file(root: &Path, path: &str, content: &[u8]) -> SyncResult<()> {
    let relative = path_from_remote(path)?;
    let absolute = root.join(relative);
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(absolute, content)?;
    Ok(())
}

fn remove_workspace_file(root: &Path, path: &str) -> SyncResult<()> {
    let absolute = root.join(path_from_remote(path)?);
    match fs::remove_file(absolute) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn synced_state(remote_version: Option<String>, local: &LocalFileMeta, now: u64) -> SyncFileState {
    SyncFileState {
        remote_version,
        local_hash: local.hash.clone(),
        size: local.size,
        last_synced_at: now,
    }
}

fn insert_conflict(
    state: &mut SyncState,
    path: &str,
    local_hash: Option<String>,
    remote_version: Option<String>,
) {
    state.conflicts.insert(
        path.to_owned(),
        SyncConflictState {
            path: path.to_owned(),
            local_hash,
            remote_version,
            message: "local and remote both changed".to_owned(),
        },
    );
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> SyncResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let contents = serde_json::to_string_pretty(value)?;
    fs::write(path, contents)?;
    Ok(())
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Fnv1a64::default();
    hasher.write(bytes);
    format!("{:016x}", hasher.finish())
}

#[derive(Default)]
struct Fnv1a64(u64);

impl Hasher for Fnv1a64 {
    fn finish(&self) -> u64 {
        if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut hash = self.finish();
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.0 = hash;
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn encode_path(path: &str) -> String {
    path.split('/')
        .map(percent_encode_segment)
        .collect::<Vec<_>>()
        .join("/")
}

fn percent_encode_segment(segment: &str) -> String {
    let mut encoded = String::new();
    for byte in segment.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn header_string(headers: &reqwest::header::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn normalize_bearer_token(token: Option<String>) -> Option<String> {
    token
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

pub enum SyncLoopMessage {
    LocalWrite,
    UserActivity,
    Stop,
}

pub fn run_sync_loop(
    root: PathBuf,
    app: AppHandle,
    sync_lock: Arc<Mutex<()>>,
    rx: mpsc::Receiver<SyncLoopMessage>,
) {
    let min_interval = Duration::from_secs(5);
    let max_interval = Duration::from_secs(80);
    let mut interval = min_interval;
    let mut last_sync_at = Instant::now() - min_interval; // treat as ready on first event
    let mut activity_pending = false;

    loop {
        let timeout = (last_sync_at + interval).saturating_duration_since(Instant::now());

        match rx.recv_timeout(timeout) {
            Ok(SyncLoopMessage::LocalWrite) | Ok(SyncLoopMessage::UserActivity) => {
                activity_pending = true;
                if last_sync_at.elapsed() >= min_interval {
                    if let Ok(guard) = sync_lock.try_lock() {
                        let status = do_background_sync(&root).map(|mut status| {
                            status.background_loop_running = true;
                            status
                        });
                        drop(guard);
                        if let Some(status) = status {
                            let _ = app.emit("sync-status", &status);
                        }
                    }
                    last_sync_at = Instant::now();
                    interval = min_interval;
                    activity_pending = false;
                }
            }
            Ok(SyncLoopMessage::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Ok(guard) = sync_lock.try_lock() {
                    let status = do_background_sync(&root).map(|mut status| {
                        status.background_loop_running = true;
                        status
                    });
                    drop(guard);
                    if let Some(status) = status {
                        let _ = app.emit("sync-status", &status);
                    }
                }
                last_sync_at = Instant::now();
                interval = if activity_pending {
                    min_interval
                } else {
                    (interval * 2).min(max_interval)
                };
                activity_pending = false;
            }
        }
    }
}

fn do_background_sync(root: &Path) -> Option<SyncStatus> {
    fn finalize_background_sync(
        root: &Path,
        result: SyncResult<SyncRunSummary>,
    ) -> Option<SyncStatus> {
        match result {
            Ok(summary) => Some(summary.status),
            Err(error) if error.remote_missing => {
                clear_sync_metadata(root).ok()?;
                sync_status(root).ok()
            }
            Err(_) => sync_status(root).ok(),
        }
    }

    let config = match read_sync_config(root) {
        Ok(Some(c)) if c.enabled => c,
        _ => return None,
    };
    let secrets = read_sync_auth_secrets(root).ok()?;
    let provider = HttpSyncProvider::new_workspace_with_auth(
        &config.sync_root_url,
        &config.remote_workspace_id,
        secrets.bearer_token.clone(),
    )
    .ok()?;
    match sync_once(root, &provider) {
        Err(e) if e.auth_expired => {
            let new_secrets = refresh_supabase_auth(&config.sync_root_url, &secrets).ok()?;
            write_sync_auth_secrets(root, &new_secrets).ok()?;
            let new_provider = HttpSyncProvider::new_workspace_with_auth(
                &config.sync_root_url,
                &config.remote_workspace_id,
                new_secrets.bearer_token,
            )
            .ok()?;
            finalize_background_sync(root, sync_once(root, &new_provider))
        }
        other => finalize_background_sync(root, other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider {
        list_result: SyncResult<Vec<RemoteFileMeta>>,
    }

    impl SyncProvider for MockProvider {
        fn list(&self) -> SyncResult<Vec<RemoteFileMeta>> {
            self.list_result.clone()
        }

        fn pull(&self, _path: &str) -> SyncResult<RemoteFileBlob> {
            unreachable!("pull should not be called in these tests");
        }

        fn push(&self, _request: PushFileRequest) -> SyncResult<PushResult> {
            unreachable!("push should not be called in these tests");
        }

        fn delete(&self, _request: DeleteFileRequest) -> SyncResult<PushResult> {
            unreachable!("delete should not be called in these tests");
        }
    }

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("{name}-{}", now_unix_secs()))
    }

    #[test]
    fn remote_paths_reject_parent_segments() {
        assert!(path_from_remote("pages/A.md").is_ok());
        assert!(path_from_remote("../pages/A.md").is_err());
        assert!(path_from_remote("pages/../A.md").is_err());
        assert!(path_from_remote("pages\\A.md").is_err());
    }

    #[test]
    fn local_scan_skips_sync_state_and_transaction_files() {
        let root = test_root("uniseq-sync-test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::create_dir_all(root.join("uniseq")).unwrap();
        fs::create_dir_all(root.join(".uniseq-page-transaction")).unwrap();
        fs::write(root.join("pages").join("A.md"), "a").unwrap();
        fs::write(root.join("uniseq").join("sync-config.json"), "{}").unwrap();
        fs::write(root.join("uniseq").join("sync-auth.json"), "{}").unwrap();
        fs::write(root.join("uniseq").join("sync-state.json"), "{}").unwrap();
        fs::write(
            root.join(".uniseq-page-transaction").join("manifest.json"),
            "{}",
        )
        .unwrap();

        let files = scan_local_files(&root).unwrap();

        assert!(files.contains_key("pages/A.md"));
        assert!(!files.contains_key("uniseq/sync-config.json"));
        assert!(!files.contains_key("uniseq/sync-auth.json"));
        assert!(!files.contains_key("uniseq/sync-state.json"));
        assert!(!files.contains_key(".uniseq-page-transaction/manifest.json"));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_scan_reports_progress_while_hashing_files() {
        let root = test_root("uniseq-sync-progress");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("pages")).unwrap();
        fs::write(root.join("pages").join("A.md"), "a").unwrap();
        fs::write(root.join("pages").join("B.md"), "b").unwrap();

        let mut ticks = Vec::new();
        let files = scan_local_files_with_progress(&root, |current, total, path| {
            ticks.push((current, total, path.map(str::to_owned)));
        })
        .unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(ticks.first(), Some(&(0, 2, None)));
        assert_eq!(ticks.last(), Some(&(2, 2, Some("pages/B.md".to_owned()))));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disconnect_deleted_remote_clears_stale_sync_metadata() {
        let root = test_root("uniseq-sync-disconnect-missing");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("pages")).unwrap();
        write_sync_config(
            &root,
            &SyncConfig::new_with_auth(
                SyncProviderKind::Uniseq,
                "https://sync.example.com/demo".to_owned(),
                "workspace-1".to_owned(),
                "Workspace 1".to_owned(),
                SyncAuthConfig {
                    kind: SyncAuthKind::Bearer,
                },
            ),
        )
        .unwrap();
        write_sync_auth_secrets(
            &root,
            &SyncAuthSecrets {
                bearer_token: Some("token".to_owned()),
                refresh_token: Some("refresh".to_owned()),
                supabase_publishable_key: Some("key".to_owned()),
            },
        )
        .unwrap();
        write_sync_state(
            &root,
            &SyncState {
                files: BTreeMap::new(),
                conflicts: BTreeMap::new(),
                last_synced_at: Some(42),
                last_error: None,
            },
        )
        .unwrap();

        let disconnected = disconnect_deleted_remote_with_provider(
            &root,
            &MockProvider {
                list_result: Err(SyncError::new_remote_missing("remote workspace not found")),
            },
        )
        .unwrap();

        assert!(disconnected);
        assert!(read_sync_config(&root).unwrap().is_none());
        assert_eq!(
            read_sync_auth_secrets(&root).unwrap(),
            SyncAuthSecrets::default()
        );
        assert_eq!(read_sync_state(&root).unwrap(), SyncState::default());
        assert_eq!(sync_status(&root).unwrap().kind, SyncStatusKind::Off);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn disconnect_deleted_remote_ignores_transient_sync_errors() {
        let root = test_root("uniseq-sync-disconnect-network");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("pages")).unwrap();
        let config = SyncConfig::new_with_auth(
            SyncProviderKind::Uniseq,
            "https://sync.example.com/demo".to_owned(),
            "workspace-1".to_owned(),
            "Workspace 1".to_owned(),
            SyncAuthConfig::default(),
        );
        write_sync_config(&root, &config).unwrap();

        let disconnected = disconnect_deleted_remote_with_provider(
            &root,
            &MockProvider {
                list_result: Err(SyncError::new("network timeout")),
            },
        )
        .unwrap();

        assert!(!disconnected);
        assert_eq!(read_sync_config(&root).unwrap(), Some(config));

        let _ = fs::remove_dir_all(root);
    }
}
