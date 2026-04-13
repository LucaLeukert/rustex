use serde::{Deserialize, Serialize};
use crate::ids::*;
use crate::models::*;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddArgs {
    pub author: String,
    pub body: String,
}

pub type AddResponse = ();

pub type CollectResponse = ();

