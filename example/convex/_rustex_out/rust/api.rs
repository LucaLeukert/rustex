#![allow(unused_imports)]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::ids::*;
use crate::models::*;
use rustex_runtime::{ActionSpec, FunctionSpec, MutationSpec, QuerySpec};

pub mod messages {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct AddArgs {
        pub author: String,
        pub body: String,
    }

    pub type AddResponse = MessagesId;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Add;
    
    pub fn add() -> Add {
        Add
    }
    
    impl FunctionSpec for Add {
        type Args = AddArgs;
        type Output = AddResponse;
        const PATH: &'static str = "messages:add";
    }
    impl MutationSpec for Add {}
    
    pub type CollectArgs = ();

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct CollectResponseItem {
        #[serde(rename = "_id")]
        pub id: MessagesId,
        #[serde(rename = "_creationTime")]
        pub creation_time: f64,
        pub author: String,
        pub body: String,
    }

    pub type CollectResponse = Vec<CollectResponseItem>;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct Collect;
    
    pub fn collect() -> Collect {
        Collect
    }
    
    impl FunctionSpec for Collect {
        type Args = CollectArgs;
        type Output = CollectResponse;
        const PATH: &'static str = "messages:collect";
    }
    impl QuerySpec for Collect {}
    
}

