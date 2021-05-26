use std::ops::Bound;

use chrono::{Duration, SubsecRound, Utc};
use diesel::pg::PgConnection;
use rand::Rng;
use svc_agent::AgentId;
use uuid::Uuid;

use crate::db::agent::{Object as Agent, Status as AgentStatus};
use crate::db::agent_connection::Object as AgentConnection;
use crate::db::janus_backend::Object as JanusBackend;
use crate::db::recording::Object as Recording;
use crate::db::room::Object as Room;
use crate::db::rtc::{Object as Rtc, SharingPolicy as RtcSharingPolicy};
use crate::diesel::Identifiable;

use super::{agent::TestAgent, factory, SVC_AUDIENCE, USR_AUDIENCE};

///////////////////////////////////////////////////////////////////////////////

pub(crate) fn insert_room(conn: &PgConnection) -> Room {
    let now = Utc::now().trunc_subsecs(0);

    factory::Room::new()
        .audience(USR_AUDIENCE)
        .time((
            Bound::Included(now),
            Bound::Excluded(now + Duration::hours(1)),
        ))
        .rtc_sharing_policy(RtcSharingPolicy::Shared)
        .insert(conn)
}

pub(crate) fn insert_room_with_backend_id(conn: &PgConnection, backend_id: &AgentId) -> Room {
    let now = Utc::now().trunc_subsecs(0);

    factory::Room::new()
        .audience(USR_AUDIENCE)
        .time((
            Bound::Included(now),
            Bound::Excluded(now + Duration::hours(1)),
        ))
        .rtc_sharing_policy(RtcSharingPolicy::Shared)
        .backend_id(backend_id)
        .insert(conn)
}

pub(crate) fn insert_closed_room(conn: &PgConnection) -> Room {
    let now = Utc::now().trunc_subsecs(0);

    factory::Room::new()
        .audience(USR_AUDIENCE)
        .time((
            Bound::Included(now - Duration::hours(10)),
            Bound::Excluded(now - Duration::hours(8)),
        ))
        .rtc_sharing_policy(RtcSharingPolicy::Shared)
        .insert(conn)
}

pub(crate) fn insert_closed_room_with_backend_id(
    conn: &PgConnection,
    backend_id: &AgentId,
) -> Room {
    let now = Utc::now().trunc_subsecs(0);

    factory::Room::new()
        .audience(USR_AUDIENCE)
        .time((
            Bound::Included(now - Duration::hours(10)),
            Bound::Excluded(now - Duration::hours(8)),
        ))
        .rtc_sharing_policy(RtcSharingPolicy::Shared)
        .backend_id(backend_id)
        .insert(conn)
}

pub(crate) fn insert_room_with_owned(conn: &PgConnection) -> Room {
    let now = Utc::now().trunc_subsecs(0);

    factory::Room::new()
        .audience(USR_AUDIENCE)
        .time((Bound::Included(now), Bound::Unbounded))
        .rtc_sharing_policy(RtcSharingPolicy::Owned)
        .insert(conn)
}

pub(crate) fn insert_agent(conn: &PgConnection, agent_id: &AgentId, room_id: Uuid) -> Agent {
    factory::Agent::new()
        .agent_id(agent_id)
        .room_id(room_id)
        .status(AgentStatus::Ready)
        .insert(conn)
}

pub(crate) fn insert_connected_agent(
    conn: &PgConnection,
    agent_id: &AgentId,
    backend_id: &AgentId,
    room_id: Uuid,
    rtc_id: Uuid,
) -> (Agent, AgentConnection) {
    let mut rng = rand::thread_rng();
    let agent = insert_agent(conn, agent_id, room_id);

    let janus_backend_handle =
        factory::JanusBackendHandle::new(backend_id, &[rng.gen()]).insert(&conn);

    let agent_connection = factory::AgentConnection::new(
        *agent.id(),
        rtc_id,
        janus_backend_handle.handle_id(),
        *janus_backend_handle.id(),
    )
    .insert(conn);

    (agent, agent_connection)
}

pub(crate) fn insert_janus_backend(conn: &PgConnection) -> JanusBackend {
    let mut rng = rand::thread_rng();

    let label_suffix: String = rng
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(5)
        .collect();

    let label = format!("janus-gateway-{}", label_suffix);
    let agent = TestAgent::new("alpha", &label, SVC_AUDIENCE);

    let backend = factory::JanusBackend::new(agent.agent_id().to_owned(), rng.gen(), rng.gen())
        .capacity(10)
        .balancer_capacity(10)
        .insert(conn);

    let handle_ids = (0..10).map(|_| rng.gen()).collect::<Vec<_>>();
    factory::JanusBackendHandle::new(backend.id(), &handle_ids).insert(&conn);
    backend
}

pub(crate) fn insert_rtc(conn: &PgConnection) -> Rtc {
    let room = insert_room(conn);
    factory::Rtc::new(room.id()).insert(conn)
}

pub(crate) fn insert_rtc_with_room(conn: &PgConnection, room: &Room) -> Rtc {
    factory::Rtc::new(room.id()).insert(conn)
}

pub(crate) fn insert_recording(conn: &PgConnection, rtc: &Rtc) -> Recording {
    factory::Recording::new().rtc(rtc).insert(conn)
}
