use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use field_photo_dispatch::{
    infrai_client::InfraiClient,
    work_order_photos::{self, CompletionRequest, UploadRequest, WorkflowError},
};
use serde_json::json;
use std::{
    env,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone)]
struct AppState {
    infrai: Arc<InfraiClient>,
    photo_bucket: Arc<str>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let infrai = InfraiClient::from_env()?;
    let started_at = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let photo_bucket: Arc<str> =
        format!("field-service-photos-{started_at}-{}", std::process::id()).into();
    infrai.create_bucket(&photo_bucket).await?;
    let app = Router::new()
        .route("/work-orders/:work_order_id/photos/upload", post(upload))
        .route(
            "/work-orders/:work_order_id/photos/:photo_id/complete",
            post(complete),
        )
        .with_state(AppState {
            infrai: Arc::new(infrai),
            photo_bucket: Arc::clone(&photo_bucket),
        });
    let address = env::var("BIND_ADDRESS").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    println!("field photo dispatch using bucket {photo_bucket} and listening on http://{address}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn upload(
    State(state): State<AppState>,
    Path(work_order_id): Path<String>,
    Json(input): Json<UploadRequest>,
) -> Result<Json<work_order_photos::UploadTicket>, ApiError> {
    Ok(Json(
        work_order_photos::start_upload(&state.infrai, &state.photo_bucket, &work_order_id, input)
            .await?,
    ))
}

async fn complete(
    State(state): State<AppState>,
    Path((work_order_id, photo_id)): Path<(String, String)>,
    Json(input): Json<CompletionRequest>,
) -> Result<Json<work_order_photos::CompletedPhoto>, ApiError> {
    Ok(Json(
        work_order_photos::finish_upload(
            &state.infrai,
            &state.photo_bucket,
            &work_order_id,
            &photo_id,
            input,
        )
        .await?,
    ))
}

struct ApiError(WorkflowError);
impl From<WorkflowError> for ApiError {
    fn from(value: WorkflowError) -> Self {
        Self(value)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            WorkflowError::InvalidInput(_) => StatusCode::BAD_REQUEST,
            WorkflowError::Infrai(error) => error.caller_status(),
        };
        (status, Json(json!({ "error": self.0.to_string() }))).into_response()
    }
}
