use serde::{de::DeserializeOwned, Serialize, Deserialize};
use thiserror::Error;
use wasm_bindgen::{closure::Closure, prelude::*, JsValue};
use hex::{FromHexError, decode};
use alloy_primitives::{Address, aliases::U256};
use alloy_rpc_types::Signature;
use crate::helpers::{serialize, log};

#[wasm_bindgen]
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

#[derive(Debug, Clone)]
// All attributes this library needs is thread unsafe.
// But wasm itself is a single threaded... something.
// To avoid problems with Send and Sync, all these parameters are
// fetched whenever it is needed
pub struct Eip1193 {}

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
        let ethereum = Ethereum::default()?;
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

        let payload = Eip1193Request::new(method.to_string(), parsed_params.into());

        match ethereum.request(payload).await {
            Ok(r) => Ok(serde_wasm_bindgen::from_value(r).unwrap()),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn sign_typed_data<T: Send + Sync + Serialize>(
        &self,
        data: T,
        from: &Address,
    ) -> Result<Signature, Eip1193Error> {
        let data = serialize(&data);
        log(format!("data is {:#?}", data).as_str());
        // modify data to change chain_id=0xnumber to number 
        let from = serialize(&from.to_string());
/* 
        error: "expected value in line 1 column 1" in serde_json::from_slice()
    https://docs.metamask.io/wallet/reference/eth_signtypeddata_v4/
        Test:
        - Signature parsing (json_serde::from_slice -> Signature::from.. why doesn't exists?)
        
        'ed:
        - formatting data with chainId: 1 (instead of 0x1 sended)   -> Nope! original sending 0x1 works fine


*/
        let sig: String = self.request("eth_signTypedData_v4", [from, data]).await?;
        //log(format!("signed {:#?}", sig).as_str());
        let sig = sig.strip_prefix("0x").unwrap_or(&sig);
        //log(format!("sig2 {:#?}", sig).as_str());
        let sig = decode(sig)?;
        //log(format!("test {:#?}", sig.clone()).as_str());    // there's an error here
        //log(format!("sig3 {:#?}", sig).as_str());
        let slice = sig.as_slice();

        // how to create Signature from this? 
        //log(format!("slice {:#?}", slice.clone()).as_str()); 
        // BUG in here: "expected value" parsing signature directly 

//        let r = Signature::try_from(slice).expect("Could not parse Signature");        
        
        let s: PreSignature = serde_json::from_slice(slice).expect("Could not parse Signature");
        let r = Signature {
            r: s.r,
            s: s.s,
            v: U256::from(s.v), //???
            y_parity: None
        };



        Ok(r)
    }
    
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy, Hash)]
/// An ECDSA signature
pub struct PreSignature {
    /// R value
    pub r: U256,
    /// S Value
    pub s: U256,
    /// V value
    pub v: u64,
}