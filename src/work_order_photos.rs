use crate::infrai_client::{InfraiClient, InfraiError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CANONICAL_CAPABILITY: &str = "storage.object.presign";

#[derive(Debug, Deserialize)]
pub struct UploadRequest {
    pub technician_id: String,
    pub photo_id: String,
    pub filename: String,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct UploadTicket {
    pub work_order_id: String,
    pub photo_id: String,
    pub upload_url: String,
    pub method: &'static str,
    pub status: DispatchStatus,
}

#[derive(Debug, Deserialize)]
pub struct CompletionRequest {
    pub technician_id: String,
    pub requires_follow_up: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchStatus {
    AwaitingUpload,
    Dispatched,
    TechnicianFollowUp,
}

#[derive(Debug, Serialize)]
pub struct CompletedPhoto {
    pub work_order_id: String,
    pub photo_id: String,
    pub technician_id: String,
    pub variant: String,
    pub status: DispatchStatus,
}

#[derive(Debug, Error)]
pub enum WorkflowError {
    #[error("invalid {0}")]
    InvalidInput(&'static str),
    #[error(transparent)]
    Infrai(#[from] InfraiError),
}

fn safe_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
}

fn object_key(work_order_id: &str, photo_id: &str) -> Result<String, WorkflowError> {
    if !safe_component(work_order_id) {
        return Err(WorkflowError::InvalidInput("work_order_id"));
    }
    if !safe_component(photo_id) {
        return Err(WorkflowError::InvalidInput("photo_id"));
    }
    Ok(format!("work-{work_order_id}-photo-{photo_id}"))
}

pub fn next_dispatch_status(requires_follow_up: bool) -> DispatchStatus {
    if requires_follow_up {
        DispatchStatus::TechnicianFollowUp
    } else {
        DispatchStatus::Dispatched
    }
}

pub async fn start_upload(
    client: &InfraiClient,
    photo_bucket: &str,
    work_order_id: &str,
    input: UploadRequest,
) -> Result<UploadTicket, WorkflowError> {
    if !safe_component(&input.technician_id) {
        return Err(WorkflowError::InvalidInput("technician_id"));
    }
    if input.filename.is_empty() {
        return Err(WorkflowError::InvalidInput("filename"));
    }
    if !matches!(
        input.content_type.as_str(),
        "image/jpeg" | "image/png" | "image/webp"
    ) {
        return Err(WorkflowError::InvalidInput("content_type"));
    }
    let key = object_key(work_order_id, &input.photo_id)?;
    let operation_id = format!("upload:{work_order_id}:{}", input.photo_id);
    let signed = client
        .presign_put(photo_bucket, &key, &input.content_type, &operation_id)
        .await?;
    Ok(UploadTicket {
        work_order_id: work_order_id.to_owned(),
        photo_id: input.photo_id,
        upload_url: signed.url,
        method: "PUT",
        status: DispatchStatus::AwaitingUpload,
    })
}

pub async fn finish_upload(
    client: &InfraiClient,
    photo_bucket: &str,
    work_order_id: &str,
    photo_id: &str,
    input: CompletionRequest,
) -> Result<CompletedPhoto, WorkflowError> {
    if !safe_component(&input.technician_id) {
        return Err(WorkflowError::InvalidInput("technician_id"));
    }
    let key = object_key(work_order_id, photo_id)?;
    let source = client
        .presign_get(
            photo_bucket,
            &key,
            &format!("source:{work_order_id}:{photo_id}"),
        )
        .await?;
    let resized = client
        .resize(&source.url, &format!("resize:{work_order_id}:{photo_id}"))
        .await?;
    let compressed = client
        .compress(
            resized.reference()?,
            &format!("compress:{work_order_id}:{photo_id}"),
        )
        .await?;
    Ok(CompletedPhoto {
        work_order_id: work_order_id.to_owned(),
        photo_id: photo_id.to_owned(),
        technician_id: input.technician_id,
        variant: compressed.reference()?.to_owned(),
        status: next_dispatch_status(input.requires_follow_up),
    })
}
