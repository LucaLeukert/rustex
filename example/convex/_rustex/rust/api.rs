#![allow(unused_imports)]
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use crate::ids::*;
use crate::models::*;
use rustex_runtime::{ActionSpec, FunctionSpec, MutationSpec, QuerySpec};

pub mod messages {
    use super::*;

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
    
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct FindByAuthorArgs {
        pub author: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct FindByAuthorResponseItem {
        #[serde(rename = "_id")]
        pub id: MessagesId,
        #[serde(rename = "_creationTime")]
        pub creation_time: f64,
        pub author: String,
        pub body: String,
    }

    pub type FindByAuthorResponse = Vec<FindByAuthorResponseItem>;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct FindByAuthor;
    
    pub fn find_by_author() -> FindByAuthor {
        FindByAuthor
    }
    
    impl FunctionSpec for FindByAuthor {
        type Args = FindByAuthorArgs;
        type Output = FindByAuthorResponse;
        const PATH: &'static str = "messages:findByAuthor";
    }
    impl QuerySpec for FindByAuthor {}
    
    pub type MultiReturnDemoArgs = ();

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    pub struct MultiReturnDemoResponseVariant1MessagesItem {
        #[serde(rename = "_id")]
        pub id: MessagesId,
        #[serde(rename = "_creationTime")]
        pub creation_time: f64,
        pub author: String,
        pub body: String,
    }

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
    #[serde(untagged)]
    pub enum MultiReturnDemoResponse {
        MessagesCountError {
            messages: Vec<MultiReturnDemoResponseVariant1MessagesItem>,
            count: f64,
            error: String,
        },
        Error {
            error: String,
        },
    }

    pub type MultiReturnDemoResponse2 = MultiReturnDemoResponse;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct MultiReturnDemo;
    
    pub fn multi_return_demo() -> MultiReturnDemo {
        MultiReturnDemo
    }
    
    impl FunctionSpec for MultiReturnDemo {
        type Args = MultiReturnDemoArgs;
        type Output = MultiReturnDemoResponse;
        const PATH: &'static str = "messages:multiReturnDemo";
    }
    impl QuerySpec for MultiReturnDemo {}
    
}

