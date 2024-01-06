use async_trait::async_trait;
use alloy_json_rpc::{SerializedRequest, Response, RequestPacket, ResponsePacket};
use alloy_transport::{TransportError, TransportErrorKind}; 
use std::pin::Pin;
use futures::Future;
use super::{BrowserTransport, WebProvider};

// need this to be a trait (in order to implement async)
#[async_trait(?Send)]
pub trait WebClient {
    fn send(&self, req: SerializedRequest) -> Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send + 'static>>;
    fn send_packet(&self, req: RequestPacket) -> Pin<Box<dyn Future<Output = Result<ResponsePacket, TransportError>> + Send>> ;
}

#[async_trait(?Send)]
impl WebClient for BrowserTransport {
    fn send(
        &self,
        req: SerializedRequest,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Response, alloy_json_rpc::RpcError<TransportErrorKind>>> + Send + 'static>> {
        match &self.wallet {
            WebProvider::None => panic!("Unavailable"),//Err(EthereumError::NotConnected),
            WebProvider::Injected(provider) => provider.send(req),
//            WebProvider::WalletConnect(_provider) => panic!("Unavailable")//Err(EthereumError::Unavailable)//Ok(provider.request(req).await?),
        }
    }
    fn send_packet(&self, req: RequestPacket) -> Pin<Box<dyn Future<Output = Result<ResponsePacket, TransportError>> + Send>> {
        match &self.wallet {
            WebProvider::None => panic!("Unavailable"),
            WebProvider::Injected(provider) => provider.send_packet(req),
//            WebProvider::WalletConnect(_provider) => panic!("Unavailable")
        }
    }
}  