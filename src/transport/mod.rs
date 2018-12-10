use failure::{err_msg, Error};
use serde_derive::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct AgentId {
    label: String,
    account_id: String,
    audience: String,
}

impl AgentId {
    pub(crate) fn new(label: &str, account_id: &str, audience: &str) -> Self {
        Self {
            label: label.to_owned(),
            account_id: account_id.to_owned(),
            audience: audience.to_owned(),
        }
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}.{}.{}", self.label, self.account_id, self.audience)
    }
}

impl FromStr for AgentId {
    type Err = Error;

    fn from_str(val: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = val.splitn(3, '.').collect();
        match parts[..] {
            [label, account_id, audience] => Ok(Self::new(&label, &account_id, &audience)),
            _ => Err(err_msg(format!("Invalid value for agent id: {}", val))),
        }
    }
}

pub(crate) mod compat;