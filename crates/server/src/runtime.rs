//! Owns one local server, shared-engine discovery, and graceful shutdown.
use antiquota_accounts::FileAccounts;
use antiquota_agy::{AgyRunner, isolated_fake};
use antiquota_app::{AppError, Service, SystemClock};
use antiquota_store::{EngineInfo, FileStore, data_root, lock_file, token, write_json_atomic};
use std::{path::PathBuf, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::Notify,
};
pub(crate) struct HttpState {
    pub service: Arc<Service>,
    pub token: String,
    pub port: u16,
    pub agy: PathBuf,
    pub ui_dir: PathBuf,
    pub shutdown: Arc<Notify>,
    pub closing: tokio::sync::watch::Sender<bool>,
}
#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error(transparent)]
    App(#[from] AppError),
    #[error(transparent)]
    Port(#[from] antiquota_core::PortError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(String),
}
pub async fn serve(port: u16, open_browser: bool) -> Result<(), ServerError> {
    let root = data_root()?;
    let Some(_lease) = lock_file(&root.join("state/engine.lock"))? else {
        let info = discover().await?;
        println!("antiquota already runs on {} (pid {})", info.url, info.pid);
        if open_browser {
            open_url(&info.url);
        }
        return Ok(());
    };
    let store = Arc::new(FileStore::open(root.clone())?);
    let home = antiquota_store::user_home()?;
    let executable = antiquota_accounts::find_agy(&home).unwrap_or_else(|_| PathBuf::from("agy"));
    let runner = Arc::new(AgyRunner {
        executable: executable.clone(),
        hook_program: std::env::current_exe()?,
        isolated_fake: isolated_fake(&executable, &root),
    });
    let accounts = Arc::new(FileAccounts::with_runtime(
        root.clone(),
        home,
        executable.clone(),
    ));
    // Tool calls and mid-turn messages of this engine's turns reach it through its own hook.
    accounts.seed_stack(false)?;
    accounts.install_hook(&std::env::current_exe()?)?;
    let service = Service::compose(antiquota_app::Ports {
        store,
        settings: antiquota_store::read_settings(&root)?,
        accounts: accounts.clone(),
        runner,
        clock: Arc::new(SystemClock),
        quota: Some(accounts),
        shell: Some(Arc::new(antiquota_shell::Shell)),
    })?;
    let listener = match TcpListener::bind(("127.0.0.1", port)).await {
        Ok(x) => x,
        Err(_) if port != 0 => TcpListener::bind(("127.0.0.1", 0)).await?,
        Err(e) => return Err(e.into()),
    };
    let port = listener.local_addr()?.port();
    let shutdown = Arc::new(Notify::new());
    let (closing, _) = tokio::sync::watch::channel(false);
    let state = Arc::new(HttpState {
        service: service.clone(),
        token: token(&root)?,
        port,
        agy: executable,
        ui_dir: std::env::var_os("ANTIQUOTA_UI_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui/dist")),
        shutdown: shutdown.clone(),
        closing: closing.clone(),
    });
    let router = axum::Router::new()
        .fallback(crate::http::dispatch)
        .with_state(state);
    let wait = shutdown.clone();
    let stopping = service.clone();
    let server = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                tokio::select! {_=wait.notified()=>{},_=signals()=>{}};
                stopping.begin_shutdown();
                let _ = stopping.accounts.cancel_logins().await;
                closing.send_replace(true);
            })
            .await
    });
    let info = EngineInfo {
        pid: std::process::id(),
        port,
        url: format!("http://127.0.0.1:{port}/"),
        version: env!("CARGO_PKG_VERSION").into(),
        started: antiquota_core::Clock::now(&SystemClock),
        by: std::env::var("ANTIQUOTA_LAUNCHED_BY").unwrap_or_else(|_| "cli".into()),
    };
    let ready = tokio::time::timeout(std::time::Duration::from_secs(3), async {
        loop {
            if health(port).await.is_ok() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    if ready.is_err() {
        shutdown.notify_one();
        return Err(ServerError::Invalid("engine did not become healthy".into()));
    }
    write_json_atomic(&root.join("state/engine.json"), &info)?;
    service.account_service.start_account_polling();
    println!("antiquota on {} (Ctrl-C to stop)", info.url);
    if open_browser {
        open_url(&info.url);
    }
    let result = server
        .await
        .map_err(|e| ServerError::Invalid(e.to_string()))?;
    service.shutdown().await;
    let _ = std::fs::remove_file(root.join("state/engine.json"));
    result?;
    Ok(())
}
pub async fn discover() -> Result<EngineInfo, ServerError> {
    let root = data_root()?;
    let text = std::fs::read_to_string(root.join("state/engine.json"))?;
    let info: EngineInfo = serde_json::from_str(&text)
        .map_err(|_| ServerError::Invalid("engine discovery is malformed".into()))?;
    let actual = health(info.port).await?;
    if actual.pid != info.pid || actual.app != "antiquota" {
        return Err(ServerError::Invalid("engine discovery is stale".into()));
    }
    Ok(info)
}
async fn health(port: u16) -> Result<crate::api::HealthResponse, ServerError> {
    tokio::time::timeout(std::time::Duration::from_secs(1),async {
        let mut socket=TcpStream::connect(("127.0.0.1",port)).await?;
        socket.write_all(format!("GET /api/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n").as_bytes()).await?;
        let mut bytes=vec![];socket.take(65536).read_to_end(&mut bytes).await?;
        let offset=bytes.windows(4).position(|x|x==b"\r\n\r\n").ok_or_else(||ServerError::Invalid("engine health response is malformed".into()))?+4;
        serde_json::from_slice(&bytes[offset..]).map_err(|_|ServerError::Invalid("engine health response is malformed".into()))
    }).await.map_err(|_|ServerError::Invalid("engine health timed out".into()))?
}
async fn signals() {
    #[cfg(unix)]
    {
        let mut terminate =
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(s) => s,
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                    return;
                }
            };
        tokio::select! {_=terminate.recv()=>{},_=tokio::signal::ctrl_c()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
pub fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(error) = result {
        tracing::warn!(%error,"browser could not open");
    }
}
