use std::{collections::BTreeMap, marker::PhantomData, pin::Pin, task};

use convex::{
    ConvexClient, FunctionResult, QuerySetSubscription, QuerySubscription, SubscriberId, Value,
};
use futures_core::Stream;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::{Instrument, debug, trace};
use tracing_subscriber::{EnvFilter, fmt};

pub trait FunctionSpec {
    type Args: Serialize;
    type Output: DeserializeOwned;

    const PATH: &'static str;
}

pub trait QuerySpec: FunctionSpec {}
pub trait MutationSpec: FunctionSpec {}
pub trait ActionSpec: FunctionSpec {}

pub struct TypedSubscription<F> {
    inner: QuerySubscription,
    marker: PhantomData<fn() -> F>,
}

impl<F> TypedSubscription<F> {
    pub fn from_inner(inner: QuerySubscription) -> Self {
        Self {
            inner,
            marker: PhantomData,
        }
    }

    pub fn id(&self) -> &SubscriberId {
        self.inner.id()
    }

    pub fn inner(&self) -> &QuerySubscription {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut QuerySubscription {
        &mut self.inner
    }

    pub fn into_inner(self) -> QuerySubscription {
        self.inner
    }
}

impl<F> std::fmt::Debug for TypedSubscription<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedSubscription")
            .field("subscriber_id", self.id())
            .finish()
    }
}

impl<F> Stream for TypedSubscription<F>
where
    F: QuerySpec,
{
    type Item = Result<F::Output, RuntimeError>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut task::Context<'_>,
    ) -> task::Poll<Option<Self::Item>> {
        match Pin::new(&mut self.inner).poll_next(cx) {
            task::Poll::Ready(Some(result)) => task::Poll::Ready(Some(decode_result(result))),
            task::Poll::Ready(None) => task::Poll::Ready(None),
            task::Poll::Pending => task::Poll::Pending,
        }
    }
}

pub struct RustexClient {
    inner: ConvexClient,
}

impl Clone for RustexClient {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl RustexClient {
    #[tracing::instrument(name = "rustex_runtime.client.new", skip_all, fields(deployment_url))]
    pub async fn new(deployment_url: &str) -> anyhow::Result<Self> {
        debug!("connecting Convex client");
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
        let encoded_args = encode_args(args)?;
        let span = tracing::info_span!("rustex_runtime.query", convex.function = F::PATH);
        async move {
            debug!(argument_count = encoded_args.len(), "executing typed query");
            let result = self.inner.query(F::PATH, encoded_args).await?;
            decode_result(result)
        }
        .instrument(span)
        .await
    }

    pub async fn subscribe<F>(
        &mut self,
        _function: F,
        args: &F::Args,
    ) -> Result<TypedSubscription<F>, RuntimeError>
    where
        F: QuerySpec,
    {
        let encoded_args = encode_args(args)?;
        let span = tracing::info_span!("rustex_runtime.subscribe", convex.function = F::PATH);
        async move {
            debug!(
                argument_count = encoded_args.len(),
                "creating typed subscription"
            );
            let subscription = self.inner.subscribe(F::PATH, encoded_args).await?;
            Ok(TypedSubscription::from_inner(subscription))
        }
        .instrument(span)
        .await
    }

    pub async fn mutation<F>(
        &mut self,
        _function: F,
        args: &F::Args,
    ) -> Result<F::Output, RuntimeError>
    where
        F: MutationSpec,
    {
        let encoded_args = encode_args(args)?;
        let span = tracing::info_span!("rustex_runtime.mutation", convex.function = F::PATH);
        async move {
            debug!(
                argument_count = encoded_args.len(),
                "executing typed mutation"
            );
            let result = self.inner.mutation(F::PATH, encoded_args).await?;
            decode_result(result)
        }
        .instrument(span)
        .await
    }

    pub async fn action<F>(
        &mut self,
        _function: F,
        args: &F::Args,
    ) -> Result<F::Output, RuntimeError>
    where
        F: ActionSpec,
    {
        let encoded_args = encode_args(args)?;
        let span = tracing::info_span!("rustex_runtime.action", convex.function = F::PATH);
        async move {
            debug!(
                argument_count = encoded_args.len(),
                "executing typed action"
            );
            let result = self.inner.action(F::PATH, encoded_args).await?;
            decode_result(result)
        }
        .instrument(span)
        .await
    }

    pub fn watch_all(&self) -> QuerySetSubscription {
        self.inner.watch_all()
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

pub fn init_default_tracing(
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
}

#[tracing::instrument(name = "rustex_runtime.encode_args", skip_all)]
pub fn encode_args<T: Serialize>(args: &T) -> Result<BTreeMap<String, Value>, RuntimeError> {
    let json = serde_json::to_value(args)?;
    match json {
        serde_json::Value::Null => Ok(BTreeMap::new()),
        serde_json::Value::Object(map) => map
            .into_iter()
            .map(|(key, value)| Ok((key, Value::try_from(value)?)))
            .collect::<Result<BTreeMap<_, _>, RuntimeError>>()
            .inspect(|encoded| trace!(argument_count = encoded.len(), "encoded Convex args")),
        _ => Err(RuntimeError::InvalidArgsShape),
    }
}

#[tracing::instrument(name = "rustex_runtime.decode_result", skip_all)]
pub fn decode_result<T: DeserializeOwned>(result: FunctionResult) -> Result<T, RuntimeError> {
    match result {
        FunctionResult::Value(value) => {
            let json: serde_json::Value = value.into();
            trace!("deserializing Convex function value");
            Ok(serde_json::from_value(json)?)
        }
        FunctionResult::ErrorMessage(message) => {
            debug!("Convex function returned an error message");
            Err(RuntimeError::FunctionMessage(message))
        }
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
