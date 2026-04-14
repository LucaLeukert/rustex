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
    
}

#[doc(hidden)]
#[macro_export]
macro_rules! __rustex_arg_value {
    ($field:ident, $value:expr) => {
        ::core::convert::Into::into($value)
    };
    ($field:ident) => {
        ::core::convert::Into::into($field)
    };
}

#[macro_export]
macro_rules! query {
    ($client:expr, messages::collect) => {
        $client.query($crate::api::messages::collect(), &())
    };
    ($client:expr, messages::collect, {}) => {
        $client.query($crate::api::messages::collect(), &())
    };
}

#[macro_export]
macro_rules! mutation {
    ($client:expr, messages::add, { $($field:ident $( : $value:expr )?),* $(,)? }) => {
        $client.mutation($crate::api::messages::add(), &$crate::api::messages::AddArgs {
            $( $field: $crate::__rustex_arg_value!($field $(, $value)?), )*
        })
    };
}

#[macro_export]
macro_rules! action {
    ($($tt:tt)*) => {
        compile_error!("no generated functions support this operation macro in this crate")
    };
}

#[macro_export]
macro_rules! subscribe {
    ($client:expr, messages::collect) => {
        $client.subscribe($crate::api::messages::collect(), &())
    };
    ($client:expr, messages::collect, {}) => {
        $client.subscribe($crate::api::messages::collect(), &())
    };
}

