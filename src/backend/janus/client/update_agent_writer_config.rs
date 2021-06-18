use serde::Serialize;
use uuid::Uuid;

use super::{HandleId, SessionId};

#[derive(Serialize, Debug)]
pub struct UpdateWriterConfigRequest {
    pub session_id: SessionId,
    pub handle_id: HandleId,
    pub body: UpdateWriterConfigRequestBody,
}

#[derive(Debug, Serialize)]
pub struct UpdateWriterConfigRequestBody {
    method: &'static str,
    configs: Vec<UpdateWriterConfigRequestBodyConfigItem>,
}

impl UpdateWriterConfigRequestBody {
    pub(crate) fn new(configs: Vec<UpdateWriterConfigRequestBodyConfigItem>) -> Self {
        Self {
            method: "writer_config.update",
            configs,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct UpdateWriterConfigRequestBodyConfigItem {
    pub stream_id: Uuid,
    pub send_video: bool,
    pub send_audio: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_remb: Option<u32>,
}
// pub type ReadStreamResponse = EventResponse;