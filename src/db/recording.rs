use std::fmt;
use std::ops::Bound;

use chrono::{DateTime, Utc};
use diesel::{pg::PgConnection, result::Error};
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use super::rtc::Object as Rtc;
use crate::schema::recording;

////////////////////////////////////////////////////////////////////////////////

pub(crate) type AllColumns = (
    recording::rtc_id,
    recording::started_at,
    recording::segments,
    recording::status,
    recording::janus_dumps_uris,
);

pub(crate) const ALL_COLUMNS: AllColumns = (
    recording::rtc_id,
    recording::started_at,
    recording::segments,
    recording::status,
    recording::janus_dumps_uris,
);

////////////////////////////////////////////////////////////////////////////////

pub(crate) type Segment = (Bound<i64>, Bound<i64>);

#[derive(Clone, Copy, Debug, DbEnum, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
#[PgType = "recording_status"]
#[DieselType = "Recording_status"]
pub(crate) enum Status {
    #[serde(rename = "in_progress")]
    InProgress,
    Ready,
    Missing,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let serialized = serde_json::to_string(self).map_err(|_| fmt::Error)?;
        write!(f, "{}", serialized)
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Serialize, Identifiable, Associations, Queryable)]
#[belongs_to(Rtc, foreign_key = "rtc_id")]
#[primary_key(rtc_id)]
#[table_name = "recording"]
pub(crate) struct Object {
    rtc_id: Uuid,
    #[serde(with = "crate::serde::ts_seconds_option")]
    started_at: Option<DateTime<Utc>>,
    segments: Option<Vec<Segment>>,
    status: Status,
    janus_dumps_uris: Option<Vec<String>>,
}

impl Object {
    pub(crate) fn rtc_id(&self) -> Uuid {
        self.rtc_id
    }

    pub(crate) fn started_at(&self) -> &Option<DateTime<Utc>> {
        &self.started_at
    }

    pub(crate) fn segments(&self) -> &Option<Vec<Segment>> {
        &self.segments
    }

    pub(crate) fn status(&self) -> &Status {
        &self.status
    }

    /// Get a reference to the object's janus dumps uris.
    pub(crate) fn janus_dumps_uris(&self) -> Option<&Vec<String>> {
        self.janus_dumps_uris.as_ref()
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub(crate) struct FindQuery {
    rtc_id: Uuid,
}

impl FindQuery {
    pub(crate) fn new(rtc_id: Uuid) -> Self {
        Self { rtc_id }
    }

    pub(crate) fn execute(self, conn: &PgConnection) -> Result<Option<Object>, Error> {
        use diesel::prelude::*;

        recording::table
            .filter(recording::rtc_id.eq(self.rtc_id))
            .get_result(conn)
            .optional()
    }
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Insertable)]
#[table_name = "recording"]
pub(crate) struct InsertQuery {
    rtc_id: Uuid,
}

impl InsertQuery {
    pub(crate) fn new(rtc_id: Uuid) -> Self {
        Self { rtc_id }
    }

    pub(crate) fn execute(self, conn: &PgConnection) -> Result<Object, Error> {
        use crate::schema::recording::dsl::recording;
        use diesel::RunQueryDsl;

        diesel::insert_into(recording).values(self).get_result(conn)
    }
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Identifiable, AsChangeset)]
#[table_name = "recording"]
#[primary_key(rtc_id)]
pub(crate) struct UpdateQuery {
    rtc_id: Uuid,
    status: Option<Status>,
    started_at: Option<DateTime<Utc>>,
    segments: Option<Vec<Segment>>,
    janus_dumps_uris: Option<Vec<String>>,
}

impl UpdateQuery {
    pub(crate) fn new(rtc_id: Uuid) -> Self {
        Self {
            rtc_id,
            status: None,
            started_at: None,
            segments: None,
            janus_dumps_uris: None,
        }
    }

    pub(crate) fn status(self, status: Status) -> Self {
        Self {
            status: Some(status),
            ..self
        }
    }

    pub(crate) fn janus_dumps_uris(self, dumps_uris: Option<Vec<String>>) -> Self {
        Self {
            janus_dumps_uris: dumps_uris,
            ..self
        }
    }

    pub(crate) fn started_at(self, started_at: DateTime<Utc>) -> Self {
        Self {
            started_at: Some(started_at),
            ..self
        }
    }

    pub(crate) fn segments(self, segments: Vec<Segment>) -> Self {
        Self {
            segments: Some(segments),
            ..self
        }
    }

    pub(crate) fn execute(&self, conn: &PgConnection) -> Result<Object, Error> {
        use diesel::prelude::*;

        diesel::update(self).set(self).get_result(conn)
    }
}
