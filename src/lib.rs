use futures_util::{SinkExt, Stream, StreamExt};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Error as WSError;
use tokio_tungstenite::tungstenite::Message;

// Include the generated client code
pub mod prod {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    include!(concat!(env!("OUT_DIR"), "/deribit_client_prod.rs"));
}

#[cfg(feature = "testnet")]
pub mod testnet {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;
    include!(concat!(env!("OUT_DIR"), "/deribit_client_testnet.rs"));
}

// Default to prod at crate root
pub use prod::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RPC Error {}: {}", self.code, self.message)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum JsonRpcVersion {
    #[serde(rename = "2.0")]
    V2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcRequest {
    jsonrpc: JsonRpcVersion,
    id: u64,
    method: String,
    params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcResponseBase {
    jsonrpc: JsonRpcVersion,
    id: u64,
    testnet: bool,
    #[serde(rename = "usIn")]
    us_in: u64,
    #[serde(rename = "usOut")]
    us_out: u64,
    #[serde(rename = "usDiff")]
    us_diff: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcOkResponse {
    #[serde(flatten)]
    base: RpcResponseBase,
    result: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RpcErrorResponse {
    #[serde(flatten)]
    base: RpcResponseBase,
    error: RpcError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscriptionParams {
    channel: String,
    data: Value,
    label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum SubscriptionMethod {
    #[serde(rename = "subscription")]
    Subscription,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SubscriptionNotification {
    jsonrpc: JsonRpcVersion,
    method: SubscriptionMethod,
    params: SubscriptionParams,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
enum HeartbeatType {
    #[serde(rename = "heartbeat")]
    Heartbeat,
    #[serde(rename = "test_request")]
    TestRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HeartbeatParams {
    r#type: HeartbeatType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum HeartbeatMethod {
    #[serde(rename = "heartbeat")]
    Heartbeat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Heartbeat {
    jsonrpc: JsonRpcVersion,
    method: HeartbeatMethod,
    params: HeartbeatParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum JsonRPCMessage {
    Heartbeat(Heartbeat),
    Notification(SubscriptionNotification),
    OkResponse(RpcOkResponse),
    ErrorResponse(RpcErrorResponse),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("RPC error: {0}")]
    RpcError(RpcError),
    #[error("WebSocket error: {0}")]
    WebSocketError(#[from] WSError),
    #[error("JSON decode error: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Invalid subscription channel: {0}")]
    InvalidSubscriptionChannel(String),
    #[error("Subscription messages lagged: {0}")]
    SubscriptionLagged(u64),
}

type Result<T> = std::result::Result<T, Error>;

// ApiRequest trait for all request types
pub trait ApiRequest: serde::Serialize {
    type Response: DeserializeOwned + Serialize;
    fn method_name(&self) -> &'static str;

    fn is_private(&self) -> bool {
        self.method_name().starts_with("private/")
    }

    fn to_params(&self) -> Value {
        serde_json::to_value(self).unwrap_or_default()
    }
}

// Subscription trait implemented by generated channel structs
pub trait Subscription {
    type Data: DeserializeOwned + Serialize + Send + 'static;
    fn channel_string(&self) -> String;
}

// Helper used by generated code to stringify subscription path parameters
pub(crate) fn sub_param_to_string<T: Serialize>(value: &T) -> String {
    let json = serde_json::to_value(value).unwrap_or(Value::Null);
    match json {
        Value::String(s) => s,
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        _ => json.to_string(),
    }
}

#[derive(Debug)]
pub enum Env {
    Production,
    Testnet,
}

#[derive(Debug)]
pub struct DeribitClient {
    authenticated: AtomicBool,
    id_counter: Arc<AtomicU64>,
    request_channel: mpsc::Sender<(RpcRequest, oneshot::Sender<Result<Value>>)>,
    subscription_channel: mpsc::Sender<(String, oneshot::Sender<broadcast::Receiver<Value>>)>,
}

impl DeribitClient {
    pub async fn connect(env: Env) -> Result<Self> {
        let ws_url = match env {
            Env::Production => "wss://www.deribit.com/ws/api/v2",
            Env::Testnet => "wss://test.deribit.com/ws/api/v2",
        };

        let (mut ws_stream, _) = connect_async(ws_url).await?;
        let (request_tx, mut request_rx) =
            mpsc::channel::<(RpcRequest, oneshot::Sender<Result<Value>>)>(100);
        let (subscription_tx, mut subscription_rx) =
            mpsc::channel::<(String, oneshot::Sender<broadcast::Receiver<Value>>)>(100);

        let id_counter = Arc::new(AtomicU64::new(0));
        let id_counter_clone = id_counter.clone();

        tokio::spawn(async move {
            let mut pending_requests: HashMap<u64, oneshot::Sender<Result<Value>>> = HashMap::new();
            let mut subscribers: HashMap<String, broadcast::Sender<Value>> = HashMap::new();

            loop {
                tokio::select! {
                    msg = ws_stream.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                match serde_json::from_str::<JsonRPCMessage>(&text) {
                                    Ok(JsonRPCMessage::Heartbeat(heartbeat)) => {
                                        if heartbeat.params.r#type == HeartbeatType::TestRequest {
                                            let test_request = RpcRequest {
                                                jsonrpc: JsonRpcVersion::V2,
                                                id: id_counter_clone.fetch_add(1, Ordering::Relaxed),
                                                method: "public/test".to_string(),
                                                params: Value::Null,
                                            };
                                            ws_stream
                                                .send(Message::Text(
                                                    serde_json::to_string(&test_request).unwrap().into(),
                                                ))
                                                .await
                                                .unwrap();
                                        }
                                    }
                                    Ok(JsonRPCMessage::Notification(notification)) => {
                                        if let Some(tx) = subscribers.get(&notification.params.channel)
                                            && tx.send(notification.params.data.clone()).is_err()
                                        {
                                            subscribers.remove(&notification.params.channel);
                                        }
                                    }
                                    Ok(JsonRPCMessage::OkResponse(response)) => {
                                        let result = Ok(response.result);
                                        if let Some(tx) = pending_requests.remove(&response.base.id) {
                                            let _ = tx.send(result);
                                        }
                                    }
                                    Ok(JsonRPCMessage::ErrorResponse(response)) => {
                                        let error = Err(Error::RpcError(response.error));
                                        if let Some(tx) = pending_requests.remove(&response.base.id) {
                                            let _ = tx.send(error);
                                        }
                                    }
                                    Err(e) => {
                                        panic!("Received invalid json message: {e}\nOriginal message: {text}");
                                    }
                                }
                            }
                            Some(Ok(msg)) => {
                                panic!("Received non-text message: {msg:?}");
                            }
                            Some(Err(e)) => {
                                panic!("WebSocket error: {e:?}");
                            }
                            None => {
                                panic!("WebSocket connection closed");
                            }
                        }
                    }
                    Some((request, tx)) = request_rx.recv() => {
                        pending_requests.insert(request.id, tx);
                        ws_stream
                            .send(Message::Text(
                                serde_json::to_string(&request).unwrap().into(),
                            ))
                            .await
                            .unwrap();
                    }
                    Some((channel, oneshot_tx)) = subscription_rx.recv() => {
                        if let Some(broadcast_tx) = subscribers.get(&channel) {
                            let _ = oneshot_tx.send(broadcast_tx.subscribe());
                        } else {
                            let (broadcast_tx, broadcast_rx) = broadcast::channel(100);
                            subscribers.insert(channel, broadcast_tx);
                            let _ = oneshot_tx.send(broadcast_rx);
                        }
                    }
                }
            }
        });

        Ok(Self {
            authenticated: AtomicBool::new(false),
            id_counter,
            request_channel: request_tx,
            subscription_channel: subscription_tx,
        })
    }

    fn next_id(&self) -> u64 {
        self.id_counter.fetch_add(1, Ordering::Relaxed)
    }

    pub async fn call_raw(&self, method: &str, params: Value) -> Result<Value> {
        let request = RpcRequest {
            jsonrpc: JsonRpcVersion::V2,
            id: self.next_id(),
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();

        self.request_channel
            .send((request, tx))
            .await
            .map_err(|_| WSError::ConnectionClosed)?;

        let value = rx.await.map_err(|_| WSError::ConnectionClosed)??;

        if method == "public/auth" {
            self.authenticated.store(true, Ordering::Release);
        }

        Ok(value)
    }

    pub async fn call<T: ApiRequest>(&self, req: T) -> Result<T::Response> {
        let value = self.call_raw(req.method_name(), req.to_params()).await?;
        let typed: T::Response = serde_json::from_value(value)?;
        Ok(typed)
    }

    pub async fn subscribe_raw(
        &self,
        channel: &str,
    ) -> Result<impl Stream<Item = Result<Value>> + Send + 'static + use<>> {
        let channels = vec![channel.to_string()];
        let subscribed_channels = if self.authenticated.load(Ordering::Acquire) {
            self.call(PrivateSubscribeRequest {
                channels,
                label: None,
            })
            .await?
        } else {
            self.call(PublicSubscribeRequest { channels }).await?
        };
        if let Some(channel) = subscribed_channels.first() {
            let (tx, rx) = oneshot::channel();
            self.subscription_channel
                .send((channel.clone(), tx))
                .await
                .map_err(|_| WSError::ConnectionClosed)?;
            let channel_rx = rx.await.map_err(|_| WSError::ConnectionClosed)?;
            let stream = BroadcastStream::new(channel_rx).map(|msg| match msg {
                Ok(msg) => Ok(msg),
                Err(BroadcastStreamRecvError::Lagged(lag)) => Err(Error::SubscriptionLagged(lag)),
            });
            Ok(stream)
        } else {
            Err(Error::InvalidSubscriptionChannel(channel.to_string()))
        }
    }

    // Typed subscription: accepts a generated Subscription and returns a typed broadcast receiver
    pub async fn subscribe<S: Subscription + Send + 'static>(
        &self,
        subscription: S,
    ) -> Result<impl Stream<Item = Result<S::Data>> + Send + 'static> {
        let channel = subscription.channel_string();
        let raw_stream = self.subscribe_raw(&channel).await?;
        let typed_stream = raw_stream.map(|msg| match msg {
            Ok(msg) => serde_json::from_value::<S::Data>(msg).map_err(Error::JsonError),
            Err(e) => Err(e),
        });
        Ok(typed_stream)
    }
}
