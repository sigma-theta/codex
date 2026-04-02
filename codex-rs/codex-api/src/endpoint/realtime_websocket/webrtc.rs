use crate::endpoint::realtime_websocket::methods_common::normalized_session_mode;
use crate::endpoint::realtime_websocket::methods_common::session_update_session;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionConfig;
use crate::error::ApiError;
use crate::provider::Provider;
use codex_client::build_reqwest_client_with_custom_ca;
use http::HeaderMap;
use reqwest::StatusCode;
use reqwest::header::CONTENT_TYPE;
use serde::Serialize;
use tracing::info;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealtimeWebrtcCall {
    pub answer_sdp: String,
    pub call_id: String,
}

#[derive(Debug, Serialize)]
struct RealtimeWebrtcSession {
    #[serde(flatten)]
    session: crate::endpoint::realtime_websocket::protocol::SessionUpdateSession,
    model: String,
}

pub(crate) async fn create_realtime_webrtc_call(
    provider: &Provider,
    session_config: &RealtimeSessionConfig,
    extra_headers: &HeaderMap,
    default_headers: &HeaderMap,
    offer_sdp: &str,
) -> Result<RealtimeWebrtcCall, ApiError> {
    let model = realtime_session_model(provider, session_config)?;
    let session_mode =
        normalized_session_mode(session_config.event_parser, session_config.session_mode);
    let session = RealtimeWebrtcSession {
        session: session_update_session(
            session_config.event_parser,
            session_config.instructions.clone(),
            session_mode,
        ),
        model,
    };
    let url = realtime_calls_url(provider)?;
    let headers = merge_request_headers(&provider.headers, extra_headers, default_headers);
    let client =
        build_reqwest_client_with_custom_ca(reqwest::Client::builder()).map_err(|err| {
            ApiError::Stream(format!(
                "failed to configure realtime webrtc client TLS: {err}"
            ))
        })?;

    info!("creating realtime webrtc call: {url}");
    let response = client
        .post(url.clone())
        .headers(headers)
        .multipart(
            reqwest::multipart::Form::new()
                .text(
                    "session",
                    serde_json::to_string(&session).map_err(|err| {
                        ApiError::Stream(format!(
                            "failed to encode realtime webrtc session config: {err}"
                        ))
                    })?,
                )
                .text("sdp", offer_sdp.to_string()),
        )
        .send()
        .await
        .map_err(|err| ApiError::Stream(format!("failed to create realtime webrtc call: {err}")))?;

    let status = response.status();
    let location = response
        .headers()
        .get("Location")
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);
    let body = response.text().await.map_err(|err| {
        ApiError::Stream(format!("failed to read realtime webrtc SDP answer: {err}"))
    })?;

    if status != StatusCode::OK {
        return Err(ApiError::Stream(format!(
            "failed to create realtime webrtc call: status={status}, body={body}"
        )));
    }

    let call_id = location
        .as_deref()
        .and_then(realtime_call_id_from_location)
        .ok_or_else(|| {
            ApiError::Stream(format!(
                "realtime webrtc call response did not include a valid Location header: {location:?}"
            ))
        })?;

    Ok(RealtimeWebrtcCall {
        answer_sdp: body,
        call_id,
    })
}

fn realtime_session_model(
    provider: &Provider,
    session_config: &RealtimeSessionConfig,
) -> Result<String, ApiError> {
    session_config
        .model
        .clone()
        .or_else(|| provider.query_params.as_ref()?.get("model").cloned())
        .ok_or_else(|| {
            ApiError::Stream(
                "realtime webrtc call setup requires a configured realtime model".to_string(),
            )
        })
}

fn realtime_calls_url(provider: &Provider) -> Result<String, ApiError> {
    let mut url = Url::parse(provider.base_url.as_str())
        .map_err(|err| ApiError::Stream(format!("failed to parse realtime api_url: {err}")))?;
    normalize_realtime_calls_path(&mut url);
    Ok(url.to_string())
}

fn normalize_realtime_calls_path(url: &mut Url) {
    let path = url.path().to_string();
    if path.is_empty() || path == "/" {
        url.set_path("/v1/realtime/calls");
        return;
    }

    if path.ends_with("/realtime/calls") {
        return;
    }

    if path.ends_with("/realtime/calls/") {
        url.set_path(path.trim_end_matches('/'));
        return;
    }

    if path.ends_with("/realtime") {
        url.set_path(&format!("{path}/calls"));
        return;
    }

    if path.ends_with("/realtime/") {
        url.set_path(&format!("{}/calls", path.trim_end_matches('/')));
        return;
    }

    if path.ends_with("/v1") {
        url.set_path(&format!("{path}/realtime/calls"));
        return;
    }

    if path.ends_with("/v1/") {
        url.set_path(&format!("{}realtime/calls", path));
        return;
    }

    url.set_path(&format!("{path}/v1/realtime/calls"));
}

fn realtime_call_id_from_location(location: &str) -> Option<String> {
    location
        .trim_end_matches('/')
        .split('/')
        .next_back()
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
}

fn merge_request_headers(
    provider_headers: &HeaderMap,
    extra_headers: &HeaderMap,
    default_headers: &HeaderMap,
) -> HeaderMap {
    let mut headers = provider_headers.clone();
    headers.extend(extra_headers.clone());
    for (name, value) in default_headers {
        if let http::header::Entry::Vacant(entry) = headers.entry(name) {
            entry.insert(value.clone());
        }
    }
    headers.remove(CONTENT_TYPE);
    headers
}
