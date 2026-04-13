use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::ids::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MessagesDoc {
    pub _id: MessagesId,
    pub _creation_time: f64,
    pub author: String,
    pub body: String,
}

