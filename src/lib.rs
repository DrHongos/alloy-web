pub mod eip1193;    // change to DotEthereum (window.ethereum is not eip1193 convention)
pub mod helpers;
pub mod errors;
pub mod builder;
pub mod provider;
pub mod webclient;

//pub mod walletconnect;
/* 
Refactor and clean this up
- how to use and test wallet-connect?   -> Re-implement
- separate in files (Provider, WebClient, idk)

*/
/* 
use walletconnect_client::{
    prelude::{Event as WCEvent, Metadata},
    WalletConnect, /* metadata::Chain, */
}; 
*/
//use serde::{de::DeserializeOwned, Serialize};
//use walletconnect::WalletConnectProvider;

use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    sync::Arc, 
    str::FromStr,
    pin::Pin,
};
use unsafe_send_sync::UnsafeSendSync;
use wasm_bindgen::prelude::*;
use futures::Future;

use alloy_primitives::Address;
use alloy_json_rpc::{RequestPacket, ResponsePacket};
use alloy_transport::{TransportError, TransportErrorKind, TransportFut, TransportConnect}; 
use eip1193::Eip1193;
use crate::errors::EthereumError;
use crate::webclient::WebClient;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=["console"])]
    pub fn log(m: &str);
}

#[derive(Clone, Debug, Copy)]
pub enum WalletType {
    Injected,
//    WalletConnect,
}

#[derive(Debug, Clone)]
pub enum WebProvider {
    None,
    Injected(Eip1193),
//    WalletConnect(WalletConnectProvider),
}

impl PartialEq for WebProvider {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None)
            | (Self::Injected(_), Self::Injected(_)) => true,
//            | (Self::WalletConnect(_), Self::WalletConnect(_)) => true,
            _ => false,
        }
    }
}

// internal events to handle browser events
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ConnectionWaiting(String),
    Connected,
    Disconnected,
    ChainIdChanged(Option<u64>),
    AccountsChanged(Option<Vec<Address>>),
}

#[derive(Clone)]
pub struct BrowserTransport {
    //pub metadata: Metadata,
    //pub wc_project_id: Option<String>,
    //pub rpc_node: Option<String>,

    wallet: WebProvider,
    listener: Option<UnsafeSendSync<Arc<dyn Fn(Event)>>>,
}

impl Debug for BrowserTransport {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "Browser transport",
        )
    }
}

impl BrowserTransport {                 
    pub fn new(
        //name: String,
        //description: String,
        //url: String,
        //wc_project_id: Option<String>,
        //icons: Vec<String>,
        //rpc_node: Option<String>,
    ) -> Self {
        BrowserTransport {
            //metadata: Metadata::from(&name, &description, &url, icons),
            //wc_project_id,
            //rpc_node,
            wallet: WebProvider::None,
            listener: None,
        }
    }

    pub fn is_available(&self, wallet_type: WalletType) -> bool {
        match wallet_type {
            WalletType::Injected => self.injected_available(),
            //WalletType::WalletConnect => self.walletconnect_available(),
        }
    }

    pub fn available_wallets(&self) -> Vec<WalletType> {
        let mut types = Vec::new();

        if Eip1193::is_available() {
            types.push(WalletType::Injected);
        }
/* 
        if self.wc_project_id.is_some() {
            types.push(WalletType::WalletConnect);
        }
 */
        types
    }

    pub fn injected_available(&self) -> bool {
        Eip1193::is_available()
    }
/* 
    pub fn walletconnect_available(&self) -> bool {
        self.wc_project_id.is_some()
    }
 */
    pub async fn connect(
        &mut self,
        wallet: WalletType,
        listener: Option<Arc<dyn Fn(Event)>>,
    ) -> Result<(), EthereumError> {
        if self.wallet != WebProvider::None {
            return Err(EthereumError::AlreadyConnected);
        }
        self.listener = match listener {
            Some(listener) => Some(UnsafeSendSync::new(listener)),
            None => None,
        };
        match wallet {
            WalletType::Injected => self.connect_injected().await,
//            WalletType::WalletConnect => self.connect_wc(Some(1)).await,            // WARNING! faking chain_id
        }
    }

    pub fn set_listeners(&mut self, listener: Arc<dyn Fn(Event)>) {
        self.listener = Some(UnsafeSendSync::new(listener));
        log("listeners setted");
    }

    pub fn disconnect(&mut self) {
        //if let WebProvider::WalletConnect(wc) = &self.wallet {
        //    wc.disconnect();
        //}
        self.wallet = WebProvider::None;
        self.emit_event(Event::ChainIdChanged(None));
        self.emit_event(Event::AccountsChanged(None));
    }

    async fn connect_injected(&mut self) -> Result<(), EthereumError> {
        if !self.injected_available() {
            return Err(EthereumError::Unavailable);
        }
        let injected = Eip1193::new();
        self.wallet = WebProvider::Injected(injected.clone());
        {
            let mut this = self.clone();
            _ = injected.clone().on(
                "disconnected",
                Box::new(move |_| {
                    this.disconnect();
                    this.emit_event(Event::Disconnected);
                }),
            );
        }
        {
            let this = self.clone();
            _ = injected.clone().on(
                "chainChanged",
                Box::new(move |chain_id| {    
                    let chain_id_s: String = serde_wasm_bindgen::from_value(chain_id).unwrap();
                    let c = u64::from_str_radix(
                        &chain_id_s.trim_start_matches("0x"), 
                        16
                    ).unwrap();
                    this.emit_event(Event::ChainIdChanged(Some(c)));
                }),
            );
        }
        {
            let this = self.clone();
            _ = injected.clone().on(
                "accountsChanged",
                Box::new(move |accounts| {
                    let accs: Vec<String> = serde_wasm_bindgen::from_value(accounts).unwrap();
                    let accs_p: Vec<Address> = accs
                        .into_iter()
                        .map(|a: String| return Address::from_str(&a).unwrap_or(Address::ZERO))
                        .collect();
                    this.emit_event(Event::AccountsChanged(Some(accs_p)));
                }),
            );
        }
        self.emit_event(Event::Connected);
        Ok(())
    }

    fn emit_event(&self, event: Event) {
        if let Some(listener) = &self.listener {
            listener(event);
        }
    }
}


impl tower::Service<RequestPacket> for BrowserTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    #[inline]
    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        if self.wallet != WebProvider::Injected(Eip1193{}) {
            return std::task::Poll::Ready(Err(TransportErrorKind::backend_gone()));
        }
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.send_packet(req)
    }
}

impl tower::Service<RequestPacket> for &BrowserTransport {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {

        if self.wallet != WebProvider::Injected(Eip1193{}) {
            return std::task::Poll::Ready(Err(TransportErrorKind::backend_gone()));
        }
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.send_packet(req)
    }
}

impl TransportConnect for BrowserTransport {
    type Transport = BrowserTransport;

    fn is_local(&self) -> bool {
        false                       // should this detect local chain connection?
    }
    fn get_transport<'a: 'b, 'b>(&'a self) -> alloy_transport::Pbf<'b, Self::Transport, TransportError> {
        Box::pin(async { Ok(self.to_owned()) } )
    }
}
