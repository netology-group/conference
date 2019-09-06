use serde_derive::{Deserialize, Serialize};
use svc_agent::mqtt::{IncomingRequest, OutgoingResponse, Publishable, ResponseStatus};
use svc_error::Error as SvcError;
use uuid::Uuid;

use crate::db::{janus_backend, room, rtc, ConnectionPool};

////////////////////////////////////////////////////////////////////////////////

const MAX_LIMIT: i64 = 25;

////////////////////////////////////////////////////////////////////////////////

pub(crate) type CreateRequest = IncomingRequest<CreateRequestData>;

#[derive(Debug, Deserialize)]
pub(crate) struct CreateRequestData {
    room_id: Uuid,
}

pub(crate) type ReadRequest = IncomingRequest<ReadRequestData>;

#[derive(Debug, Deserialize)]
pub(crate) struct ReadRequestData {
    id: Uuid,
}

pub(crate) type ListRequest = IncomingRequest<ListRequestData>;

#[derive(Debug, Deserialize)]
pub(crate) struct ListRequestData {
    room_id: Uuid,
    offset: Option<i64>,
    limit: Option<i64>,
}

pub(crate) type ConnectRequest = IncomingRequest<ConnectRequestData>;

#[derive(Debug, Deserialize)]
pub(crate) struct ConnectRequestData {
    id: Uuid,
}

#[derive(Debug, Serialize)]
pub(crate) struct ConnectResponseData {
    handle_id: super::rtc_signal::HandleId,
}

impl ConnectResponseData {
    pub(crate) fn new(handle_id: super::rtc_signal::HandleId) -> Self {
        Self { handle_id }
    }
}

pub(crate) type ConnectResponse = OutgoingResponse<ConnectResponseData>;

////////////////////////////////////////////////////////////////////////////////

pub(crate) struct State {
    authz: svc_authz::ClientMap,
    db: ConnectionPool,
}

impl State {
    pub(crate) fn new(authz: svc_authz::ClientMap, db: ConnectionPool) -> Self {
        Self { authz, db }
    }
}

impl State {
    pub(crate) async fn create(
        &self,
        inreq: CreateRequest,
    ) -> Result<Vec<Box<dyn Publishable>>, SvcError> {
        let room_id = inreq.payload().room_id;

        // Authorization: room's owner has to allow the action
        {
            let conn = self.db.get()?;
            let room = room::FindQuery::new()
                .time(room::upto_now())
                .id(room_id)
                .execute(&conn)?
                .ok_or_else(|| {
                    SvcError::builder()
                        .status(ResponseStatus::NOT_FOUND)
                        .detail(&format!("the room = '{}' is not found", &room_id))
                        .build()
                })?;

            let room_id = room.id().to_string();
            self.authz.authorize(
                room.audience(),
                inreq.properties(),
                vec!["rooms", &room_id, "rtcs"],
                "create",
            )?;
        };

        // Creating a Real-Time Connection
        let object = {
            let conn = self.db.get()?;
            rtc::InsertQuery::new(room_id).execute(&conn)?
        };

        let message = inreq.to_response(object, ResponseStatus::OK);
        Ok(vec![Box::new(message) as Box<dyn Publishable>])
    }

    pub(crate) async fn connect(
        &self,
        inreq: ConnectRequest,
    ) -> Result<Vec<Box<dyn Publishable>>, SvcError> {
        let id = inreq.payload().id;

        // Authorization
        {
            let conn = self.db.get()?;
            let room = room::FindQuery::new()
                .time(room::upto_now())
                .rtc_id(id)
                .execute(&conn)?
                .ok_or_else(|| {
                    SvcError::builder()
                        .status(ResponseStatus::NOT_FOUND)
                        .detail(&format!("a room for the rtc = '{}' is not found", &id))
                        .build()
                })?;

            if room.backend() != &room::RoomBackend::Janus {
                return Err(SvcError::builder()
                    .status(ResponseStatus::NOT_IMPLEMENTED)
                    .detail(&format!(
                        "'rtc.connect' is not implemented for the backend = '{}'.",
                        room.backend()
                    ))
                    .build());
            }

            let rtc_id = id.to_string();
            let room_id = room.id().to_string();
            self.authz.authorize(
                room.audience(),
                inreq.properties(),
                vec!["rooms", &room_id, "rtcs", &rtc_id],
                "read",
            )?;
        };

        // TODO: implement resource management
        // Picking up first available backend
        let backends = {
            let conn = self.db.get()?;
            janus_backend::ListQuery::new().limit(1).execute(&conn)?
        };
        let backend = backends.first().ok_or_else(|| {
            SvcError::builder()
                .status(ResponseStatus::UNPROCESSABLE_ENTITY)
                .detail("no available backends")
                .build()
        })?;

        // Building a Create Janus Gateway Handle request
        let backreq = crate::app::janus::create_rtc_handle_request(
            inreq.properties().clone(),
            Uuid::new_v4(),
            id,
            backend.session_id(),
            backend.id(),
        )
        .map_err(|_| {
            SvcError::builder()
                .status(ResponseStatus::UNPROCESSABLE_ENTITY)
                .detail("error creating a backend request")
                .build()
        })?;

        Ok(vec![Box::new(backreq) as Box<dyn Publishable>])
    }

    pub(crate) async fn read(
        &self,
        inreq: ReadRequest,
    ) -> Result<Vec<Box<dyn Publishable>>, SvcError> {
        let id = inreq.payload().id;

        // Authorization
        {
            let conn = self.db.get()?;
            let room = room::FindQuery::new()
                .time(room::upto_now())
                .rtc_id(id)
                .execute(&conn)?
                .ok_or_else(|| {
                    SvcError::builder()
                        .status(ResponseStatus::NOT_FOUND)
                        .detail(&format!("a room for the rtc = '{}' is not found", &id))
                        .build()
                })?;

            let rtc_id = id.to_string();
            let room_id = room.id().to_string();
            self.authz.authorize(
                room.audience(),
                inreq.properties(),
                vec!["rooms", &room_id, "rtcs", &rtc_id],
                "read",
            )?;
        };

        // Returning Real-Time connection
        let object = {
            let conn = self.db.get()?;
            rtc::FindQuery::new()
                .id(id)
                .execute(&conn)?
                .ok_or_else(|| {
                    SvcError::builder()
                        .status(ResponseStatus::NOT_FOUND)
                        .detail(&format!("the rtc = '{}' is not found", &id))
                        .build()
                })?
        };

        let message = inreq.to_response(object, ResponseStatus::OK);
        Ok(vec![Box::new(message) as Box<dyn Publishable>])
    }

    pub(crate) async fn list(
        &self,
        inreq: ListRequest,
    ) -> Result<Vec<Box<dyn Publishable>>, SvcError> {
        let room_id = inreq.payload().room_id;

        // Authorization: room's owner has to allow the action
        {
            let conn = self.db.get()?;
            let room = room::FindQuery::new()
                .time(room::upto_now())
                .id(room_id)
                .execute(&conn)?
                .ok_or_else(|| {
                    SvcError::builder()
                        .status(ResponseStatus::NOT_FOUND)
                        .detail(&format!("the room = '{}' is not found", &room_id))
                        .build()
                })?;

            let room_id = room.id().to_string();
            self.authz.authorize(
                room.audience(),
                inreq.properties(),
                vec!["rooms", &room_id, "rtcs"],
                "list",
            )?;
        };

        // Looking up for Real-Time Connections
        let objects = {
            let conn = self.db.get()?;
            rtc::ListQuery::from((
                Some(room_id),
                inreq.payload().offset,
                Some(std::cmp::min(
                    inreq.payload().limit.unwrap_or_else(|| MAX_LIMIT),
                    MAX_LIMIT,
                )),
            ))
            .execute(&conn)?
        };

        let message = inreq.to_response(objects, ResponseStatus::OK);
        Ok(vec![Box::new(message) as Box<dyn Publishable>])
    }
}

#[cfg(test)]
mod test {
    use diesel::prelude::*;
    use serde_json::{json, Value as JsonValue};

    use crate::util::from_base64;
    use crate::test_helpers::{
        build_authz, extract_payload, test_agent::TestAgent, test_db::TestDb,
        test_factory::{insert_janus_backend, insert_room, insert_rtc},
    };

    use super::*;

    const AUDIENCE: &str = "dev.svc.example.org";

    fn build_state(db: &TestDb) -> State {
        State::new(build_authz(AUDIENCE), db.connection_pool().clone())
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct RtcResponse {
        id: Uuid,
        room_id: Uuid,
        created_at: i64,
    }

    #[test]
    fn create_rtc() {
        futures::executor::block_on(async {
            let db = TestDb::new();

            // Insert a room.
            let conn = db.connection_pool().get().unwrap();
            let room = insert_room(&conn, AUDIENCE);
            drop(conn);

            // Make rtc.create request.
            let state = build_state(&db);
            let agent = TestAgent::new("web", "user123", AUDIENCE);
            let payload = json!({"room_id": room.id()});

            let request: CreateRequest = agent.build_request("rtc.create", &payload).unwrap();
            let mut result = state.create(request).await.unwrap();
            let message = result.remove(0);
            
            // Assert response.
            let resp: RtcResponse = extract_payload(message).unwrap();
            assert_eq!(resp.room_id, room.id());

            // Assert room presence in the DB.
            let conn = db.connection_pool().get().unwrap();
            let query = crate::schema::rtc::table.find(resp.id);
            assert_eq!(query.execute(&conn).unwrap(), 1);
        });
    }

    #[test]
    fn read_rtc() {
        futures::executor::block_on(async {
            let db = TestDb::new();

            // Insert an rtc.
            let conn = db.connection_pool().get().unwrap();
            let rtc = insert_rtc(&conn, AUDIENCE);
            drop(conn);

            // Make rtc.read request.
            let state = build_state(&db);
            let agent = TestAgent::new("web", "user123", AUDIENCE);
            let payload = json!({"id": rtc.id()});
            let request: ReadRequest = agent.build_request("rtc.read", &payload).unwrap();
            let mut result = state.read(request).await.unwrap();
            let message = result.remove(0);

            // Assert response.
            let resp: RtcResponse = extract_payload(message).unwrap();
            assert_eq!(resp.id, rtc.id());
        });
    }

    #[test]
    fn list_rtcs() {
        futures::executor::block_on(async {
            let db = TestDb::new();

            // Insert rtcs.
            let conn = db.connection_pool().get().unwrap();
            let rtc = insert_rtc(&conn, AUDIENCE);
            let _other_rtc = insert_rtc(&conn, AUDIENCE);
            drop(conn);

            // Make rtc.list request.
            let state = build_state(&db);
            let agent = TestAgent::new("web", "user123", AUDIENCE);
            let payload = json!({"room_id": rtc.room_id()});
            let request: ListRequest = agent.build_request("rtc.list", &payload).unwrap();
            let mut result = state.list(request).await.unwrap();
            let message = result.remove(0);
            
            // Assert response.
            let resp: Vec<RtcResponse> = extract_payload(message).unwrap();
            assert_eq!(resp.len(), 1);
            assert_eq!(resp.first().unwrap().id, rtc.id());
        });
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct RtcConnectResponse {
        janus: String,
        plugin: String,
        session_id: i64,
        transaction: String,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct RtcConnectTransaction {
        rtc_id: String,
        session_id: i64,
        reqp: RtcConnectTransactionReqp
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct RtcConnectTransactionReqp {
        method: String,
        agent_label: String,
        account_label: String,
        audience: String,
    }

    #[test]
    fn connect_to_rtc() {
        futures::executor::block_on(async {
            let db = TestDb::new();

            // Insert an rtc and janus backend.
            let conn = db.connection_pool().get().unwrap();
            let rtc = insert_rtc(&conn, AUDIENCE);
            let backend = insert_janus_backend(&conn, AUDIENCE);
            drop(conn);

            // Make rtc.connect request.
            let state = build_state(&db);
            let agent = TestAgent::new("web", "user123", AUDIENCE);
            let payload = json!({"id": rtc.id()});
            let request: ConnectRequest = agent.build_request("rtc.connect", &payload).unwrap();
            let mut result = state.connect(request).await.unwrap();
            let message = result.remove(0);

            // Assert outgoing request to Janus.
            let resp: RtcConnectResponse = extract_payload(message).unwrap();
            assert_eq!(resp.janus, "attach");
            assert_eq!(resp.plugin, "janus.plugin.conference");
            assert_eq!(resp.session_id, backend.session_id());

            // `transaction` field is base64 encoded JSON. Decode and assert.
            let txn_wrap: JsonValue = from_base64(&resp.transaction).unwrap();
            let txn_value = txn_wrap.get("CreateRtcHandle").unwrap().to_owned();
            let txn: RtcConnectTransaction = serde_json::from_value(txn_value).unwrap();

            assert_eq!(txn, RtcConnectTransaction {
                rtc_id: rtc.id().to_string(),
                session_id: backend.session_id(),
                reqp: RtcConnectTransactionReqp {
                    method: "rtc.connect".to_string(),
                    agent_label: "web".to_string(),
                    account_label: "user123".to_string(),
                    audience: AUDIENCE.to_string(),
                }
            })
        });
    }
}
