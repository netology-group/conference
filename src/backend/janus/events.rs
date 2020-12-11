use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "janus")]
pub(crate) enum IncomingEvent {
    WebRtcUp(WebRtcUpEvent),
    Media(MediaEvent),
    Timeout(TimeoutEvent),
    HangUp(HangUpEvent),
    SlowLink(SlowLinkEvent),
    Detached(DetachedEvent),
}

// Stream started or a viewer started to receive it.
#[derive(Debug, Deserialize)]
pub(crate) struct WebRtcUpEvent {
    session_id: i64,
    sender: i64,
}

// A RTCPeerConnection closed for a DTLS alert (normal shutdown).
// With Firefox it's not being sent. There's only `DetachedEvent`.
#[derive(Debug, Deserialize)]
pub(crate) struct HangUpEvent {
    session_id: i64,
    sender: i64,
    reason: String,
}

// Audio or video bytes being received by a plugin handle.
#[derive(Debug, Deserialize)]
pub(crate) struct MediaEvent {
    session_id: i64,
    sender: i64,
    #[serde(rename = "type")]
    kind: String,
    receiving: bool,
}

impl MediaEvent {
    pub(crate) fn sender(&self) -> i64 {
        self.sender
    }

    pub(crate) fn is_video(&self) -> bool {
        self.kind == "video"
    }

    pub(crate) fn is_receiving(&self) -> bool {
        self.receiving
    }
}

// A session was torn down by the server because of timeout: 60 seconds (by default).
#[derive(Debug, Deserialize)]
pub(crate) struct TimeoutEvent {
    session_id: i64,
}

// Janus reporting problems sending media to a user
// (user sent many NACKs in the last second; uplink=true is from Janus' perspective).
#[derive(Debug, Deserialize)]
pub(crate) struct SlowLinkEvent {
    session_id: i64,
    sender: i64,
    uplink: bool,
}

// Janus handle detached.
// This is being sent in case of abnormal shutdown or after `HangUpEvent` in Chrome.
#[derive(Debug, Deserialize)]
pub(crate) struct DetachedEvent {
    session_id: i64,
    sender: i64,
}

impl DetachedEvent {
    pub(crate) fn sender(&self) -> i64 {
        self.sender
    }
}

// Janus Gateway online/offline status.
#[derive(Debug, Deserialize)]
pub(crate) struct StatusEvent {
    online: bool,
    capacity: Option<i32>,
    balancer_capacity: Option<i32>,
}

impl StatusEvent {
    pub(crate) fn online(&self) -> bool {
        self.online
    }

    pub(crate) fn capacity(&self) -> Option<i32> {
        self.capacity
    }

    pub(crate) fn balancer_capacity(&self) -> Option<i32> {
        self.balancer_capacity
    }
}
