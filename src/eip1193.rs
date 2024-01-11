use serde::Serialize;
use serde_json::value::to_raw_value;
use serde_wasm_bindgen::{from_value, Serializer};
use thiserror::Error;
use hex::FromHexError;
use std::{pin::Pin, fmt::Debug};
use futures::{Future, future::try_join_all};
use async_trait::async_trait;
use serde_json::json;

use wasm_bindgen::{closure::Closure, prelude::*, JsValue};
use wasm_bindgen_futures::spawn_local;

use alloy_json_rpc::{ResponsePayload, RequestPacket, Response, SerializedRequest, ResponsePacket, ErrorPayload};
use alloy_transport::{TransportError, TransportErrorKind};

use crate::{
//    helpers::{serialize, log}, 
    WebClient, 
};

#[wasm_bindgen]
#[derive(Clone)]
pub struct Eip1193Request {
    method: String,
    params: JsValue,
}

impl Debug for Eip1193Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Request: {} params {:#?}", self.method, js_sys::JSON::stringify(&self.params))
    }
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
// old note
// All attributes this library needs is thread unsafe.
// But wasm itself is a single threaded... something.
// To avoid problems with Send and Sync, all these parameters are
// fetched whenever it is needed
#[derive(Debug, Clone)]
pub struct Eip1193 {}

#[derive(Error, Debug)]
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

// to persist connected wallet
#[wasm_bindgen(inline_js = "export function get_accounts_js() {
    return window.ethereum.request({
        'method': 'eth_accounts',
        'params': []
    })
}" )]
extern "C" {
    #[wasm_bindgen(catch)]
    async fn get_accounts_js() -> Result<JsValue, JsValue>;
}

#[wasm_bindgen]
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
    pub fn is_available() -> bool {
        Ethereum::default().is_ok()
    }
    
    pub fn new() -> Self {
        Eip1193 {}
    }
    
    pub fn on(self, event: &str, callback: Box<dyn FnMut(JsValue)>) -> Result<(), Eip1193Error> {
        let ethereum = Ethereum::default()?;
        let closure = Closure::wrap(callback);
        ethereum.on(event, &closure);
        closure.forget();
        Ok(())
    }
    
}

// function in ethers-web that idk why is it.. never make it do anything.. anyway, still using it
pub fn parse_params<T: Serialize + Send + Sync >(params: T) -> js_sys::Array {
    let t_params = serde_wasm_bindgen::to_value(&params).unwrap();
//    log(format!("pre params {:#?}", t_params).as_str());
    let typename_object = JsValue::from_str("type");
    if !t_params.is_null() & !t_params.is_undefined() {
        js_sys::Array::from(&t_params).map(&mut |val, _, _| {
            if let Some(trans) = js_sys::Object::try_from(&val) {   // does not detect object..
//                log(format!("Its object {:#?}", trans).as_str());
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
                        return t_copy.into()
                    }
                }
            }
            val
        })
    } else {
        js_sys::Array::new()
    }
}

#[async_trait(?Send)]
impl WebClient for Eip1193 {
    fn send(
        &self,
        req: SerializedRequest,
    ) -> Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send + 'static>> {
        let (tx, rx) = futures::channel::oneshot::channel::<Result<alloy_json_rpc::Response, TransportErrorKind>>();
        // launch a thread to send the request
        spawn_local(async move {
            let method = req.method().to_string();
            let ethereum = Ethereum::default().unwrap();
            let ser = Serializer::json_compatible();    // needs serializing_map_into_object

            let payload = match req.params() {
                Some(p) => {
                    let p = serde_json::to_value(p).expect("Cannot parse value");                    
                    Eip1193Request { 
                        method, 
                        params: p.serialize(&ser).unwrap()
                    }
                },
                None => Eip1193Request{method, params: parse_params(json!(null)).into()}
            };
            let resu = ethereum.request(payload).await;
            //log(format!("is {:#?}", resu).as_str());
            let res = Response {
                id: req.id().clone(),
                payload: match resu {
                    Ok(s) => {
                        let json_string: serde_json::Value = from_value(s).unwrap();
                        let raw_value = to_raw_value(&json_string).expect("Could not serialize RawValue");
                        ResponsePayload::Success(raw_value.to_owned())
                    },
                    Err(e) => {
                        crate::helpers::log(format!("{:#?}", e).as_str());
                        let err: ErrorPayload = serde_wasm_bindgen::from_value(e).expect("Error format is wrong");
                        ResponsePayload::Failure(err)
                    }
                }
            };
            tx.send(Ok(res)).unwrap()
        });
        // and wait for the response
        let r: Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send>> 
            = Box::pin(async move {
            let d = rx.await.map_err(|_| TransportErrorKind::backend_gone())?;
            d.map_err(|_| TransportErrorKind::backend_gone())
        });
        r  
    }

    fn send_packet(&self, req: RequestPacket) -> Pin<Box<dyn Future<Output = Result<ResponsePacket, TransportError>> + Send>> {
        match req {
            RequestPacket::Single(req) => {
                let fut = self.send(req);
                Box::pin(async move {
                    match fut.await {
                        Ok(d) => Ok(ResponsePacket::Single(d.into())),
                        Err(e) => Err(e)
                    }
                })
            }
            RequestPacket::Batch(reqs) => {
                let futs = try_join_all(
                    reqs.into_iter().map(|req| self.send(req))
                );
                Box::pin(async move {
                    match futs.await {
                        Ok(d) => Ok(ResponsePacket::Batch(d.into())),
                        Err(e) => Err(e)
                    }
                })
            }
        }
    } 
}