use serde::{de::DeserializeOwned, Serialize};
use serde_json::value::RawValue;
use thiserror::Error;
use wasm_bindgen::{closure::Closure, prelude::*, JsValue};
use hex::FromHexError;
use alloy_primitives::Address;
use crate::{
    helpers::{serialize, log}, 
        WebClient, 
    //    EthereumError
};
use std::pin::Pin;

use alloy_json_rpc::{ResponsePayload, RequestPacket, RpcError, Response, SerializedRequest};
use alloy_json_rpc::ResponsePacket;
use futures::{Future, future::{try_join_all, TryJoinAll}};
use async_trait::async_trait;

/* use alloy_rpc_types::TransactionKind;
use serde_json::value::RawValue;
use wasm_bindgen_futures::JsFuture; */

// test
/*
use wasm_bindgen_futures::spawn_local;
*/
use alloy_transport::{TransportError, TransportErrorKind, TransportFut};
/* 
use std::sync::{Arc, Mutex};
use alloy_primitives::U256;
use futures::future::try_join_all;
use serde_json::value::RawValue;
use tokio::sync::{broadcast, mpsc::{self, UnboundedSender}, oneshot};
 */
#[wasm_bindgen]
#[derive(Debug, Clone)]
pub struct Eip1193Request {
    method: String,
    params: JsValue,
}

#[wasm_bindgen]
impl Eip1193Request {
    pub fn new(method: String, params: JsValue) -> Eip1193Request {
        Eip1193Request { method, params }
    }

    #[wasm_bindgen(getter)]
    pub fn method(&self) -> String {
        self.method.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn params(&self) -> JsValue {
        self.params.clone()
    }
}

#[derive(/* Debug,  */Clone)]
// All attributes this library needs is thread unsafe.
// But wasm itself is a single threaded... something.
// To avoid problems with Send and Sync, all these parameters are
// fetched whenever it is needed
pub struct Eip1193 {/* client: UnsafeSendSync<Arc<Ethereum>> */}

// what if i can't.. so i add
// client: UnsafeSendSync<Arc<RefCell<Ethereum>>>

#[derive(Error, Debug)]
/// Error thrown when sending an HTTP request
pub enum Eip1193Error {
    /// Thrown if the request failed
    #[error("JsValue error")]
    JsValueError(String),

    /// Thrown if no window.ethereum is found in DOM
    #[error("No ethereum found")]
    JsNoEthereum,

    #[error("Cannot parse ethereum response")]
    JsParseError,

    #[error("Not implemented yet")]
    Unimplemented,
/* 
    #[error(transparent)]
    /// Thrown if the response could not be parsed
    JsonRpcError(#[from] JsonRpcError),
 */
    #[error(transparent)]
    /// Serde JSON Error
    SerdeJson(#[from] serde_json::Error),
/* 
    #[error(transparent)]
    ConversionError(#[from] ConversionError),

    #[error(transparent)]
    SignatureError(#[from] SignatureError),

    */
    #[error(transparent)]
    HexError(#[from] FromHexError),
}

#[wasm_bindgen(inline_js = "export function get_provider_js() {return window.ethereum}")]
extern "C" {
    #[wasm_bindgen(catch)]
    fn get_provider_js() -> Result<Option<Ethereum>, JsValue>;
}

#[wasm_bindgen]
#[async_trait(?Send)]               // this has no effect (?)
extern "C" {
    #[derive(Clone, Debug)]
    /// An EIP-1193 provider object. Available by convention at `window.ethereum`
    pub type Ethereum;
    
    #[wasm_bindgen(catch, method)]
    async fn request(_: &Ethereum, args: Eip1193Request) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(method)]
    fn on(_: &Ethereum, eventName: &str, listener: &Closure<dyn FnMut(JsValue)>);

    #[wasm_bindgen(method, js_name = "removeListener")]
    fn removeListener(_: &Ethereum, eventName: &str, listener: &Closure<dyn FnMut(JsValue)>);
}
impl Ethereum {
    pub fn default() -> Result<Self, Eip1193Error> {
        if let Ok(Some(eth)) = get_provider_js() {
            return Ok(eth);
        } else {
            return Err(Eip1193Error::JsNoEthereum);
        }
    }
}

impl From<JsValue> for Eip1193Error {
    fn from(src: JsValue) -> Self {
        Eip1193Error::JsValueError(format!("{:?}", src))
    }
}



impl Eip1193 {
    /// Sends the request via `window.ethereum` in Js
    pub async fn request<T: Serialize + Send + Sync, R: DeserializeOwned + Send>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, Eip1193Error> {
        let t_params = serde_wasm_bindgen::to_value(&params).unwrap();
        let typename_object = JsValue::from_str("type");
        //log(format!("params is {:#?}", t_params).as_str());
        let parsed_params = if !t_params.is_null() & !t_params.is_undefined() {
            js_sys::Array::from(&t_params).map(&mut |val, _, _| {   // error undefined is not iterable
                if let Some(trans) = js_sys::Object::try_from(&val) {
                    if let Ok(obj_type) = js_sys::Reflect::get(trans, &typename_object) {
                        if let Some(type_string) = obj_type.as_string() {
                            let t_copy = trans.clone();
                            _ = match type_string.as_str() {
                                "0x01" => js_sys::Reflect::set(
                                    &t_copy,
                                    &typename_object,
                                    &JsValue::from_str("0x1"),
                                ),
                                "0x02" => js_sys::Reflect::set(
                                    &t_copy,
                                    &typename_object,
                                    &JsValue::from_str("0x2"),
                                ),
                                "0x03" => js_sys::Reflect::set(
                                    &t_copy,
                                    &typename_object,
                                    &JsValue::from_str("0x3"),
                                ),
                                _ => Ok(true),
                            };
                            return t_copy.into();
                        }
                    }
                }

                val
            })
        } else {
            js_sys::Array::new()
        };
        
        let ethereum = Ethereum::default()?;
        let payload = Eip1193Request::new(method.to_string(), parsed_params.into());
        let req = ethereum.request(payload);
        match req.await {
            Ok(r) => Ok(serde_wasm_bindgen::from_value(r).unwrap()),
            Err(e) => Err(e.into()),
        }
    }
    
    pub async fn sign_typed_data<T: Send + Sync + Serialize>(
        &self,
        data: T,
        from: &Address,
    ) -> Result<String, Eip1193Error> {
        let data = serialize(&data);
        let from = serialize(&from.to_string());

        let sig: String = self.request("eth_signTypedData_v4", [from, data]).await?;
        Ok(sig)
    }
    pub fn is_available() -> bool {
        Ethereum::default().is_ok()
    }
    
    pub fn new() -> Self {
        Eip1193 {
/*             client: UnsafeSendSync::new(Arc::new(Ethereum::default().expect("No window.ethereum"))) */
        }
    }
    
    pub fn on(self, event: &str, callback: Box<dyn FnMut(JsValue)>) -> Result<(), Eip1193Error> {
        let ethereum = Ethereum::default()?;
        let closure = Closure::wrap(callback);
        ethereum.on(event, &closure);
        closure.forget();
        Ok(())
    }
    
}

#[async_trait(?Send)]
impl WebClient for Eip1193 {
    // this is working but needs to capture all responses in Box<RawValue> or something closer to alloy_json_rpc::ResponsePacket
    // later should also move req from SerializedRequest to RequestPacket
    async fn send(
        &self,
        req: SerializedRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send/*  + 'static */>> {
        log(format!("Received {:#?}", req).as_str());
        // create a one-shot channel to "fake" the js Promise
        let (tx, rx) = futures::channel::oneshot::channel::<Result<alloy_json_rpc::Response, TransportErrorKind>>();
        Box::pin(async move {
            wasm_bindgen_futures::spawn_local(async move {
                let method = req.method().to_string();
                let ethereum = Ethereum::default().unwrap();
                let params = serde_wasm_bindgen::to_value(req.params().expect("No params")).unwrap();
                let payload = Eip1193Request::new(method, params);
                let resu = ethereum.request(payload).await;//.unwrap();
                //log(format!("is {:#?}", resu).as_str());
                let res = alloy_json_rpc::Response {
                    id: req.id().clone(),
                    payload: match resu {
                        Ok(s) => {
                            // https://docs.rs/wasm-bindgen/latest/wasm_bindgen/struct.JsValue.html
                            // need to parse between JsValue & RawValue or something like it
    /* 
                            this needs to be open to all kind of request responses...
    
    */                            
                            // requestAccounts -> Vec<String>
                            // chainId -> String
                            // ...
                            let b: String = serde_wasm_bindgen::from_value(s).expect("Error parsing JsValue");
    
    
                            let r = serde_json::value::to_raw_value(&b).expect("Error parsing RawValue");
                            ResponsePayload::Success(r.to_owned())
                        },
                        Err(e) => {
                            let b: String = serde_wasm_bindgen::from_value(e).expect("Error parsing JsValue");
                            let r: &RawValue = serde_json::from_slice(b.as_bytes()).expect("Error parsing RawValue");
                            let f = alloy_json_rpc::ErrorPayload { code: 666, message: "idk".to_string(), data: Some(r.to_owned()) };
                            ResponsePayload::Failure(f)
                        }
                    }
                };
                //log(format!("parsed {:#?}", res).as_str());
                tx.send(Ok(res)).unwrap()
            });
            // and wait for the response
            let d = rx.await.map_err(|_| TransportErrorKind::backend_gone())?;
            d.map_err(|_| TransportErrorKind::backend_gone())
        })        
    }

    async fn send_packet(&self, req: RequestPacket) -> Pin<Box<dyn Future<Output = Result<ResponsePacket, TransportError>> + Send>> {
        match req {
            RequestPacket::Single(req) => {
                let fut: Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send>> = self.send(req).await;
                Box::pin(async move {
                    match fut.await {
                        Ok(d) => Ok(ResponsePacket::Single(d.into())),
                        Err(e) => Err(e)
                    }
                })
            }
            RequestPacket::Batch(reqs) => {
                panic!("unavailable")
                //let futs = try_join_all(
                //    reqs.into_iter().map(|req| self.send(req))
                //);
                //Box::pin(futs.into())
            }
        }
    } 

}

/* 
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl tower::Service<RequestPacket> for Eip1193 {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    #[inline]
    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        /* if self.tx.is_closed() {
            return std::task::Poll::Ready(Err(TransportErrorKind::backend_gone()));
        } */
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        if let RequestPacket::Single(b) = req {
            let req = self.nrequest(b);
            Box::pin(req)       // needs to be ResponsePacket
            
        } else { panic!("Shiiiiiiiiiiiit")}
    }
}
 */

 /* 
impl tower::Service<RequestPacket> for &Frontend {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.tx.is_closed() {
            return std::task::Poll::Ready(Err(TransportErrorKind::backend_gone()));
        }
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.send_packet(req)
    }
}
 */