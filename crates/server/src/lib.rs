//! Owns the local HTTP/SSE boundary and Rust-owned public API bindings.
mod account_http;
pub mod api;
mod http;
pub mod routes;
mod runtime;
mod stream;
pub use runtime::{ServerError, discover, open_url, serve};
fn expand_cwd(text: &str) -> Result<String, antiquota_app::AppError> {
    let home = antiquota_store::user_home()?;
    Ok(if text == "~" {
        home.to_string_lossy().into_owned()
    } else if let Some(rest) = text.strip_prefix("~/") {
        home.join(rest).to_string_lossy().into_owned()
    } else {
        text.to_owned()
    })
}
