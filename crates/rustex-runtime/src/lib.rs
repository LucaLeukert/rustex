use std::collections::BTreeMap;

use convex::{ConvexClient, FunctionResult, Value};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

pub trait FunctionSpec {
    type Args: Serialize;
    type Output: DeserializeOwned;

    const PATH: &'static str;
}

pub trait QuerySpec: FunctionSpec {}
pub trait MutationSpec: FunctionSpec {}
pub trait ActionSpec: FunctionSpec {}

pub struct TypedConvexClient {
    inner: ConvexClient,
}

impl TypedConvexClient {
    pub async fn new(deployment_url: &str) -> anyhow::Result<Self> {
        Ok(Self {
            inner: ConvexClient::new(deployment_url).await?,
        })
    }

    pub fn from_inner(inner: ConvexClient) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &ConvexClient {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut ConvexClient {
        &mut self.inner
    }

    pub fn into_inner(self) -> ConvexClient {
        self.inner
    }

    pub async fn query<F>(
        &mut self,
        _function: F,
        args: &F::Args,
    ) -> Result<F::Output, RuntimeError>
    where
        F: QuerySpec,
    {
        let result = self.inner.query(F::PATH, encode_args(args)?).await?;
        decode_result(result)
    }

    pub async fn mutation<F>(
        &mut self,
        _function: F,
        args: &F::Args,
    ) -> Result<F::Output, RuntimeError>
    where
        F: MutationSpec,
    {
        let result = self.inner.mutation(F::PATH, encode_args(args)?).await?;
        decode_result(result)
    }

    pub async fn action<F>(
        &mut self,
        _function: F,
        args: &F::Args,
    ) -> Result<F::Output, RuntimeError>
    where
        F: ActionSpec,
    {
        let result = self.inner.action(F::PATH, encode_args(args)?).await?;
        decode_result(result)
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Transport(#[from] anyhow::Error),
    #[error("Convex function returned an error message: {0}")]
    FunctionMessage(String),
    #[error("Convex function raised an application error: {message}")]
    ConvexError {
        message: String,
        data: serde_json::Value,
    },
    #[error("arguments must serialize to an object or null")]
    InvalidArgsShape,
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}

pub fn encode_args<T: Serialize>(args: &T) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let json = serde_json::to_value(args)?;
    match json {
        serde_json::Value::Null => Ok(BTreeMap::new()),
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| Ok((key, Value::try_from(value)?)))
            .collect(),
        _ => Err(RuntimeError::InvalidArgsShape),
    }
}

pub fn decode_result<T: DeserializeOwned>(result: FunctionResult) -> Result<T, RuntimeError> {
    match result {
        FunctionResult::Value(value) => {
            let json: serde_json::Value = value.into();
            Ok(serde_json::from_value(json)?)
        }
        FunctionResult::ErrorMessage(message) => Err(RuntimeError::FunctionMessage(message)),
        FunctionResult::ConvexError(error) => Err(RuntimeError::ConvexError {
            message: error.message,
            data: error.data.into(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{RuntimeError, decode_result, encode_args};
    use convex::{FunctionResult, Value};
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Debug, Serialize)]
    struct AddArgs {
        author: String,
        done: bool,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct AddResponse {
        id: String,
    }

    #[test]
    fn encode_args_serializes_structs_to_convex_values() {
        let args = AddArgs {
            author: "alice".into(),
            done: true,
        };

        let encoded = encode_args(&args).expect("args should encode");
        assert!(matches!(encoded.get("author"), Some(Value::String(value)) if value == "alice"));
        assert!(matches!(encoded.get("done"), Some(Value::Boolean(true))));
    }

    #[test]
    fn encode_args_allows_null_as_empty_object() {
        let encoded = encode_args(&()).expect("unit should encode");
        assert!(encoded.is_empty());
    }

    #[test]
    fn decode_result_deserializes_typed_payloads() {
        let mut object = BTreeMap::new();
        object.insert("id".into(), Value::String("abc".into()));

        let decoded: AddResponse =
            decode_result(FunctionResult::Value(Value::Object(object))).expect("decode");
        assert_eq!(decoded, AddResponse { id: "abc".into() });
    }

    #[test]
    fn decode_result_surfaces_function_errors() {
        let error = decode_result::<serde_json::Value>(FunctionResult::ErrorMessage("boom".into()))
            .expect_err("error expected");

        assert!(matches!(error, RuntimeError::FunctionMessage(message) if message == "boom"));
    }
}
