use anyhow::anyhow;
use async_std::{stream, task};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use slog::o;
use std::result::Result as StdResult;
use svc_agent::mqtt::{
    IncomingRequestProperties, OutgoingEvent, OutgoingEventProperties, OutgoingMessage,
    ResponseStatus, ShortTermTimingProperties,
};

use crate::{
    app::{context::Context, endpoint::prelude::*, metrics::HistogramExt},
    db,
};

////////////////////////////////////////////////////////////////////////////////

const MAX_LIMIT: i64 = 25;

#[derive(Debug, Deserialize)]
pub struct ListRequest {
    room_id: db::room::Id,
    rtc_id: Option<db::rtc::Id>,
    #[serde(default)]
    #[serde(with = "crate::serde::ts_seconds_option_bound_tuple")]
    time: Option<db::room::Time>,
    offset: Option<i64>,
    limit: Option<i64>,
}

pub struct ListHandler;

#[async_trait]
impl RequestHandler for ListHandler {
    type Payload = ListRequest;
    const ERROR_TITLE: &'static str = "Failed to list rtc streams";

    async fn handle<C: Context>(
        context: &mut C,
        payload: Self::Payload,
        reqp: &IncomingRequestProperties,
    ) -> Result {
        if let Some(rtc_id) = payload.rtc_id {
            context.add_logger_tags(o!("rtc_id" => rtc_id.to_string()));
        }
        let conn = context.get_conn().await?;
        let room = task::spawn_blocking({
            let room_id = payload.room_id;
            move || helpers::find_room_by_id(room_id, helpers::RoomTimeRequirement::Open, &conn)
        })
        .await?;
        helpers::add_room_logger_tags(context, &room);

        if room.rtc_sharing_policy() == db::rtc::SharingPolicy::None {
            let err = anyhow!(
                "'rtc_stream.list' is not implemented for rtc_sharing_policy = '{}'",
                room.rtc_sharing_policy()
            );

            return Err(err).error(AppErrorKind::NotImplemented)?;
        }

        let room_id = room.id().to_string();
        let object = vec!["rooms", &room_id];

        let authz_time = context
            .authz()
            .authorize(room.audience(), reqp, object, "read")
            .await?;
        context.metrics().observe_auth(authz_time);

        let conn = context.get_conn().await?;
        let rtc_streams = task::spawn_blocking(move || {
            let mut query = db::janus_rtc_stream::ListQuery::new().room_id(payload.room_id);

            if let Some(rtc_id) = payload.rtc_id {
                query = query.rtc_id(rtc_id);
            }

            if let Some(time) = payload.time {
                query = query.time(time);
            }

            if let Some(offset) = payload.offset {
                query = query.offset(offset);
            }

            query = query.limit(std::cmp::min(payload.limit.unwrap_or(MAX_LIMIT), MAX_LIMIT));

            query.execute(&conn)
        })
        .await?;
        context
            .metrics()
            .request_duration
            .rtc_stream_list
            .observe_timestamp(context.start_timestamp());

        Ok(Box::new(stream::once(helpers::build_response(
            ResponseStatus::OK,
            rtc_streams,
            reqp,
            context.start_timestamp(),
            Some(authz_time),
        ))))
    }
}

////////////////////////////////////////////////////////////////////////////////

pub type ObjectUpdateEvent = OutgoingMessage<db::janus_rtc_stream::Object>;

pub fn update_event(
    room_id: db::room::Id,
    object: db::janus_rtc_stream::Object,
    start_timestamp: DateTime<Utc>,
) -> StdResult<ObjectUpdateEvent, AppError> {
    let uri = format!("rooms/{}/events", room_id);
    let timing = ShortTermTimingProperties::until_now(start_timestamp);
    let props = OutgoingEventProperties::new("rtc_stream.update", timing);
    Ok(OutgoingEvent::broadcast(object, props, &uri))
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod test {
    mod list {
        use std::ops::Bound;

        use chrono::SubsecRound;
        use diesel::prelude::*;

        use crate::{
            db::{janus_rtc_stream::Object as JanusRtcStream, rtc::Object as Rtc},
            test_helpers::{prelude::*, test_deps::LocalDeps},
        };

        use super::super::*;

        #[async_std::test]
        async fn list_rtc_streams() {
            let local_deps = LocalDeps::new();
            let postgres = local_deps.run_postgres();
            let db = TestDb::with_local_postgres(&postgres);
            let mut authz = TestAuthz::new();

            let (rtc_stream, rtc) = db
                .connection_pool()
                .get()
                .map(|conn| {
                    // Insert janus rtc streams.
                    let rtc_stream = factory::JanusRtcStream::new(USR_AUDIENCE).insert(&conn);

                    let rtc_stream = crate::db::janus_rtc_stream::start(rtc_stream.id(), &conn)
                        .expect("Failed to start rtc stream")
                        .expect("Missing rtc stream");

                    let other_rtc_stream = factory::JanusRtcStream::new(USR_AUDIENCE).insert(&conn);

                    crate::db::janus_rtc_stream::start(other_rtc_stream.id(), &conn)
                        .expect("Failed to start rtc stream");

                    // Find rtc.
                    let rtc: Rtc = crate::schema::rtc::table
                        .find(rtc_stream.rtc_id())
                        .get_result(&conn)
                        .expect("Rtc not found");

                    (rtc_stream, rtc)
                })
                .expect("Failed to create rtc streams");

            // Allow user to list rtcs in the room.
            let agent = TestAgent::new("web", "user123", USR_AUDIENCE);
            let room_id = rtc.room_id().to_string();
            let object = vec!["rooms", &room_id];
            authz.allow(agent.account_id(), object, "read");

            // Make rtc_stream.list request.
            let mut context = TestContext::new(db, authz);

            let payload = ListRequest {
                room_id: rtc.room_id(),
                rtc_id: Some(rtc.id()),
                time: None,
                offset: None,
                limit: None,
            };

            let messages = handle_request::<ListHandler>(&mut context, &agent, payload)
                .await
                .expect("Rtc streams listing failed");

            // Assert response.
            let (streams, respp, _) = find_response::<Vec<JanusRtcStream>>(messages.as_slice());
            assert_eq!(respp.status(), ResponseStatus::OK);
            assert_eq!(streams.len(), 1);

            let expected_time = match rtc_stream.time().expect("Missing time") {
                (Bound::Included(val), upper) => (Bound::Included(val.trunc_subsecs(0)), upper),
                _ => panic!("Bad rtc stream time"),
            };

            assert_eq!(streams[0].id(), rtc_stream.id());
            assert_eq!(streams[0].handle_id(), rtc_stream.handle_id());
            assert_eq!(streams[0].backend_id(), rtc_stream.backend_id());
            assert_eq!(streams[0].label(), rtc_stream.label());
            assert_eq!(streams[0].sent_by(), rtc_stream.sent_by());
            assert_eq!(streams[0].time(), Some(expected_time));
            assert_eq!(
                streams[0].created_at(),
                rtc_stream.created_at().trunc_subsecs(0)
            );
        }

        #[async_std::test]
        async fn list_rtc_streams_not_authorized() {
            let local_deps = LocalDeps::new();
            let postgres = local_deps.run_postgres();
            let db = TestDb::with_local_postgres(&postgres);
            let agent = TestAgent::new("web", "user123", USR_AUDIENCE);

            let room = {
                let conn = db
                    .connection_pool()
                    .get()
                    .expect("Failed to get DB connection");

                shared_helpers::insert_room(&conn)
            };

            let mut context = TestContext::new(db, TestAuthz::new());

            let payload = ListRequest {
                room_id: room.id(),
                rtc_id: None,
                time: None,
                offset: None,
                limit: None,
            };

            let err = handle_request::<ListHandler>(&mut context, &agent, payload)
                .await
                .expect_err("Unexpected success on rtc listing");

            assert_eq!(err.status(), ResponseStatus::FORBIDDEN);
            assert_eq!(err.kind(), "access_denied");
        }

        #[async_std::test]
        async fn list_rtc_streams_missing_room() {
            let local_deps = LocalDeps::new();
            let postgres = local_deps.run_postgres();
            let db = TestDb::with_local_postgres(&postgres);

            let agent = TestAgent::new("web", "user123", USR_AUDIENCE);
            let mut context = TestContext::new(db, TestAuthz::new());

            let payload = ListRequest {
                room_id: db::room::Id::random(),
                rtc_id: None,
                time: None,
                offset: None,
                limit: None,
            };

            let err = handle_request::<ListHandler>(&mut context, &agent, payload)
                .await
                .expect_err("Unexpected success on rtc listing");

            assert_eq!(err.status(), ResponseStatus::NOT_FOUND);
            assert_eq!(err.kind(), "room_not_found");
        }
    }
}
