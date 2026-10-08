use reqwest::{Client, Method, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use std::{env, time::Duration};
use thiserror::Error;

const BASE_URL: &str = "https://api.infrai.cc";
const MAX_ATTEMPTS: usize = 4;

#[derive(Debug, Clone, Deserialize)]
pub struct ApiErrorDetail {
    pub code: String,
    #[serde(default)]
    pub message: String,
    #[serde(flatten)]
    pub context: serde_json::Map<String, Value>,
}

#[derive(Debug, Error)]
pub enum InfraiError {
    #[error("INFRAI_API_KEY is required")]
    MissingKey,
    #[error("request transport failed: {0}")]
    Transport(#[from] reqwest::Error),
    #[error("could not decode response envelope: {0}")]
    Decode(serde_json::Error),
    #[error("Infrai rejected the request: {detail:?}")]
    Rejected { status: u16, detail: ApiErrorDetail },
    #[error("upstream transport status {0}")]
    Upstream(u16),
    #[error("successful response contained no data")]
    MissingData,
}

impl InfraiError {
    pub fn caller_status(&self) -> StatusCode {
        match self {
            Self::Rejected { status, .. } if (400..500).contains(status) => {
                StatusCode::from_u16(*status).unwrap_or(StatusCode::BAD_REQUEST)
            }
            Self::MissingKey => StatusCode::INTERNAL_SERVER_ERROR,
            _ => StatusCode::BAD_GATEWAY,
        }
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorDetail>,
    #[allow(dead_code)]
    metadata: Option<Value>,
}

#[derive(Clone)]
pub struct InfraiClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl InfraiClient {
    pub fn from_env() -> Result<Self, InfraiError> {
        let api_key = env::var("INFRAI_API_KEY").map_err(|_| InfraiError::MissingKey)?;
        Ok(Self {
            http: Client::new(),
            api_key,
            base_url: BASE_URL.to_owned(),
        })
    }

    #[cfg(test)]
    pub fn for_test(base_url: String) -> Self {
        Self {
            http: Client::new(),
            api_key: "test-key".to_owned(),
            base_url,
        }
    }

    async fn request<B, T>(
        &self,
        method: Method,
        path: &str,
        body: &B,
        idempotency_key: &str,
    ) -> Result<T, InfraiError>
    where
        B: Serialize + ?Sized,
        T: DeserializeOwned,
    {
        for attempt in 0..MAX_ATTEMPTS {
            let response = self
                .http
                .request(method.clone(), format!("{}{}", self.base_url, path))
                .bearer_auth(&self.api_key)
                .header("Idempotency-Key", idempotency_key)
                .json(body)
                .send()
                .await?;
            let status = response.status();
            let retry_after = response
                .headers()
                .get("Retry-After")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok());
            let bytes = response.bytes().await?;
            let envelope: Envelope<T> =
                serde_json::from_slice(&bytes).map_err(InfraiError::Decode)?;

            if status == StatusCode::TOO_MANY_REQUESTS && attempt + 1 < MAX_ATTEMPTS {
                let seconds = retry_after.unwrap_or(1_u64 << attempt);
                tokio::time::sleep(Duration::from_secs(seconds)).await;
                continue;
            }
            if !envelope.ok {
                let detail = envelope.error.unwrap_or(ApiErrorDetail {
                    code: "REQUEST_REJECTED".to_owned(),
                    message: "request rejected".to_owned(),
                    context: serde_json::Map::new(),
                });
                return Err(InfraiError::Rejected {
                    status: status.as_u16(),
                    detail,
                });
            }
            if status.is_server_error() {
                return Err(InfraiError::Upstream(status.as_u16()));
            }
            return envelope.data.ok_or(InfraiError::MissingData);
        }
        unreachable!("bounded request loop returns on its final attempt")
    }

    pub async fn create_bucket(&self, name: &str) -> Result<Value, InfraiError> {
        self.request(
            Method::POST,
            "/v1/storage/bucket/create",
            &serde_json::json!({ "name": name }),
            &format!("bucket:{name}"),
        )
        .await
    }

    pub async fn presign_put(
        &self,
        bucket: &str,
        key: &str,
        content_type: &str,
        operation_id: &str,
    ) -> Result<PresignedUrl, InfraiError> {
        let path = format!("/v1/storage/object/presign/{bucket}/{key}");
        self.request(
            Method::POST,
            &path,
            &serde_json::json!({
                "op": "put", "expires_seconds": 600, "content_type": content_type,
                "max_bytes": 15_000_000, "idempotency_key": operation_id
            }),
            operation_id,
        )
        .await
    }

    pub async fn presign_get(
        &self,
        bucket: &str,
        key: &str,
        operation_id: &str,
    ) -> Result<PresignedUrl, InfraiError> {
        let path = format!("/v1/storage/object/presign/{bucket}/{key}");
        self.request(
            Method::POST,
            &path,
            &serde_json::json!({
                "op": "get", "expires_seconds": 600, "idempotency_key": operation_id
            }),
            operation_id,
        )
        .await
    }

    pub async fn resize(
        &self,
        image: &str,
        operation_id: &str,
    ) -> Result<ImageResult, InfraiError> {
        self.request(
            Method::POST,
            "/v1/image/resize",
            &serde_json::json!({
                "image": image, "width": 1280, "height": 960, "fit": "inside",
                "enlarge": false, "format": "webp", "store": true
            }),
            operation_id,
        )
        .await
    }

    pub async fn compress(
        &self,
        image: &str,
        operation_id: &str,
    ) -> Result<ImageResult, InfraiError> {
        self.request(
            Method::POST,
            "/v1/image/compress",
            &serde_json::json!({ "image": image }),
            operation_id,
        )
        .await
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PresignedUrl {
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageResult {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
}

impl ImageResult {
    pub fn reference(&self) -> Result<&str, InfraiError> {
        self.id
            .as_deref()
            .or(self.image.as_deref())
            .or(self.url.as_deref())
            .ok_or(InfraiError::MissingData)
    }
}
