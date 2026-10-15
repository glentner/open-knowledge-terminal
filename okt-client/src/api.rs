//! Talking to the server: `api()` and `post()`, returning the body text so the caller can parse
//! it into the payload type it expects.

use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, Response};

use crate::dom::win;

#[derive(Debug, Clone)]
pub enum ApiError {
    /// fetch itself failed (no server): a `TypeError` from `fetch`
    Network(String),
    /// the server answered with an error (its `error` message, or the status text)
    Http(String),
}

impl ApiError {
    pub fn message(&self) -> String {
        match self {
            ApiError::Network(m) | ApiError::Http(m) => m.clone(),
        }
    }
}

async fn fetch(req: &Request) -> Result<String, ApiError> {
    let resp = JsFuture::from(win().fetch_with_request(req))
        .await
        .map_err(|e| ApiError::Network(crate::js::err_message(&e)))?;
    let resp: Response = resp.unchecked_into();
    let text = match resp.text() {
        Ok(p) => JsFuture::from(p)
            .await
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default(),
        Err(_) => String::new(),
    };
    if !resp.ok() {
        let mut msg = resp.status_text();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
            if let Some(e) = v.get("error").and_then(|e| e.as_str()) {
                if !e.is_empty() {
                    msg = e.to_string();
                }
            }
        }
        return Err(ApiError::Http(msg));
    }
    Ok(text)
}

/// GET `path`, the body as text.
pub async fn get_text(path: &str) -> Result<String, ApiError> {
    let req =
        Request::new_with_str(path).map_err(|e| ApiError::Network(crate::js::err_message(&e)))?;
    fetch(&req).await
}

/// GET `path` as JSON of type `T`.
pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let text = get_text(path).await?;
    serde_json::from_str(&text).map_err(|e| ApiError::Http(format!("bad response: {e}")))
}

/// POST JSON to `path` (with the header the server requires of the page).
pub async fn post(path: &str, body: &serde_json::Value) -> Result<serde_json::Value, ApiError> {
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(&body.to_string()));
    let req = Request::new_with_str_and_init(path, &init)
        .map_err(|e| ApiError::Network(crate::js::err_message(&e)))?;
    let _ = req.headers().set("Content-Type", "application/json");
    let _ = req.headers().set("X-OKT", "1");
    let text = fetch(&req).await?;
    serde_json::from_str(&text).map_err(|e| ApiError::Http(format!("bad response: {e}")))
}

/// `fetch('/api/hello', { cache: 'no-store' })` succeeded with 2xx?
pub async fn hello_ok() -> bool {
    let init = RequestInit::new();
    init.set_cache(web_sys::RequestCache::NoStore);
    let Ok(req) = Request::new_with_str_and_init("/api/hello", &init) else {
        return false;
    };
    match JsFuture::from(win().fetch_with_request(&req)).await {
        Ok(r) => r.unchecked_into::<Response>().ok(),
        Err(_) => false,
    }
}

pub fn spawn(f: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(f);
}
