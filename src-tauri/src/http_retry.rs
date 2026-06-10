use reqwest::header::HeaderMap;
use reqwest::StatusCode;

pub const MAX_GITLAB_RETRIES: u32 = 5;

pub fn gitlab_retry_delay_seconds(retry_no: u32) -> u64 {
    u64::from(retry_no) * 5
}

pub fn should_retry_gitlab_status(status: StatusCode) -> bool {
    matches!(
        status,
        StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    )
}

pub fn retry_after_seconds(headers: &HeaderMap, retry_no: u32) -> u64 {
    headers
        .get("Retry-After")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| gitlab_retry_delay_seconds(retry_no))
}
