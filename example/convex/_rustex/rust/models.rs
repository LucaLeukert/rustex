#![allow(unused_imports)]
use crate::ids::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct MessagesDoc {
    pub _id: MessagesId,
    pub _creation_time: f64,
    pub author: String,
    pub body: String,
}
