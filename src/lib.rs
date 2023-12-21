pub mod eip1193;
pub mod walletconnect;
pub mod helpers;

use eip1193::{Eip1193, Eip1193Error/* , Eip1193Request */};
use alloy_primitives::{Address, aliases::U256};
use hex::FromHexError;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fmt::{Debug, Formatter, Result as FmtResult},
    sync::Arc, str::FromStr,
};
use thiserror::Error;
use unsafe_send_sync::UnsafeSendSync;
use walletconnect_client::{
    prelude::{Event as WCEvent, Metadata},
    WalletConnect,
};

use walletconnect::WalletConnectProvider;
use wasm_bindgen::prelude::*;

use alloy_json_rpc::{SerializedRequest, Response, RequestPacket, ResponsePacket};
use async_trait::async_trait;
use futures::Future;
use alloy_transport::{TransportError, TransportErrorKind, TransportFut, TransportConnect}; 
use std::pin::Pin;


#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace=["console"])]
    pub fn log(m: &str);
}

pub struct EthereumBuilder {
    pub chain_id: u64,
    pub name: String,
    pub description: String,
    pub url: String,
    pub wc_project_id: Option<String>,
    pub icons: Vec<String>,
    pub rpc_node: Option<String>,
}

impl EthereumBuilder {
    pub fn new() -> Self {
        Self {
            chain_id: 1,
            name: "Example dApp".to_string(),
            description: "An example dApp written in Rust".to_string(),
            url: "https://github.com/quay-rs/ethers-web".to_string(),
            wc_project_id: None,
            icons: Vec::new(),
            rpc_node: None,
        }
    }

    pub fn chain_id(&mut self, chain_id: u64) -> &Self {
        self.chain_id = chain_id;
        self
    }

    pub fn name(&mut self, name: &str) -> &Self {
        self.name = name.to_string();
        self
    }

    pub fn description(&mut self, description: &str) -> &Self {
        self.description = description.to_string();
        self
    }

    pub fn url(&mut self, url: &str) -> &Self {
        self.url = url.to_string();
        self
    }

    pub fn walletconnect_id(&mut self, wc_project_id: &str) -> &Self {
        self.wc_project_id = Some(wc_project_id.to_string());
        self
    }

    pub fn rpc_node(&mut self, rpc_node: &str) -> &Self {
        self.rpc_node = Some(rpc_node.to_string());
        self
    }

    pub fn add_icon(&mut self, icon_url: &str) -> &Self {
        self.icons.push(icon_url.to_string());
        self
    }

    pub fn build(&self) -> Ethereum {
        Ethereum::new(
            self.chain_id,
            self.name.clone(),
            self.description.clone(),
            self.url.clone(),
            self.wc_project_id.clone(),
            self.icons.clone(),
            self.rpc_node.clone(),
        )
    }
}

#[derive(Clone, Debug, Copy)]
pub enum WalletType {
    Injected,
    WalletConnect,
}

#[derive(Error, Debug)]
pub enum EthereumError {
    #[error("Wallet unavaibale")]
    Unavailable,

    #[error("Not connected")]
    NotConnected,

    #[error("Already connected")]
    AlreadyConnected,
/* 
    #[error(transparent)]
    ConversionError(#[from] ConversionError),

    #[error(transparent)]
    ProviderError(#[from] ProviderError),

    #[error(transparent)]
    SignatureError(#[from] SignatureError),
 */
    #[error(transparent)]
    HexError(#[from] FromHexError),

    #[error(transparent)]
    Eip1193Error(#[from] Eip1193Error),

    #[error(transparent)]
    WalletConnectError(#[from] walletconnect::Error),

    #[error(transparent)]
    WalletConnectClientError(#[from] walletconnect_client::Error),
}
/* 
impl From<EthereumError> for ProviderError {
    fn from(src: EthereumError) -> Self {
        ProviderError::JsonRpcClientError(Box::new(src))
    }
}

impl RpcError for EthereumError {
    fn as_serde_error(&self) -> Option<&serde_json::Error> {
        None
    }

    fn is_serde_error(&self) -> bool {
        false
    }

    fn as_error_response(&self) -> Option<&JsonRpcError> {
        None
    }

    fn is_error_response(&self) -> bool {
        false
    }
}
 */
#[derive(/* Debug,  */Clone)]
pub enum WebProvider {
    None,
    Injected(Eip1193),
    WalletConnect(WalletConnectProvider),
}

impl PartialEq for WebProvider {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::None, Self::None)
            | (Self::Injected(_), Self::Injected(_))
            | (Self::WalletConnect(_), Self::WalletConnect(_)) => true,
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct Ethereum {
    pub metadata: Metadata,
    pub wc_project_id: Option<String>,
    pub rpc_node: Option<String>,

    accounts: Option<Vec<Address>>,
    chain_id: Option<u64>,
    wallet: WebProvider,
    listener: Option<UnsafeSendSync<Arc<dyn Fn(Event)>>>,
}

impl Debug for Ethereum {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "Ethereum with accounts: {:?}, chain_id: {:?} ",
            self.accounts, self.chain_id
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ConnectionWaiting(String),
    Connected,
    Disconnected,
    ChainIdChanged(Option<u64>),
    AccountsChanged(Option<Vec<Address>>),
}

impl Ethereum {
    fn new(
        chain_id: u64,
        name: String,
        description: String,
        url: String,
        wc_project_id: Option<String>,
        icons: Vec<String>,
        rpc_node: Option<String>,
    ) -> Self {
        Ethereum {
            metadata: Metadata::from(&name, &description, &url, icons),
            wc_project_id,
            rpc_node,
            accounts: None,
            chain_id: Some(chain_id),
            wallet: WebProvider::None,
            listener: None,
        }
    }

    pub fn is_available(&self, wallet_type: WalletType) -> bool {
        match wallet_type {
            WalletType::Injected => self.injected_available(),
            WalletType::WalletConnect => self.walletconnect_available(),
        }
    }

    pub fn available_wallets(&self) -> Vec<WalletType> {
        let mut types = Vec::new();

        if Eip1193::is_available() {
            types.push(WalletType::Injected);
        }

        if self.wc_project_id.is_some() {
            types.push(WalletType::WalletConnect);
        }

        types
    }

    pub fn injected_available(&self) -> bool {
        Eip1193::is_available()
    }

    pub fn walletconnect_available(&self) -> bool {
        self.wc_project_id.is_some()
    }

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
            WalletType::WalletConnect => self.connect_wc().await,
        }
    }

    pub fn disconnect(&mut self) {
        //if let WebProvider::WalletConnect(wc) = &self.wallet {
        //    wc.disconnect();
        //}
        self.wallet = WebProvider::None;
        self.accounts = None;
        self.chain_id = None;

        self.emit_event(Event::ChainIdChanged(None));
        self.emit_event(Event::AccountsChanged(None));
    }

    async fn connect_injected(&mut self) -> Result<(), EthereumError> {
        if !self.injected_available() {
            return Err(EthereumError::Unavailable);
        }
        let injected = Eip1193::new();
        self.wallet = WebProvider::Injected(injected.clone());

        self.accounts = Some(self.request_accounts().await?.into_iter().map(|a| Address::from_str(&a).unwrap()).collect());
        self.chain_id = Some(self.request_chain_id().await?.try_into().unwrap());

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
            let mut this = self.clone();
            _ = injected.clone().on(
                "chainChanged",
                Box::new(move |chain_id| {    
                    let chain_id_s: String = serde_wasm_bindgen::from_value(chain_id).unwrap();
                    let c = u64::from_str_radix(
                        &chain_id_s.trim_start_matches("0x"), 
                        16
                    ).unwrap();
                    this.chain_id = Some(c);
                    this.emit_event(Event::ChainIdChanged(this.chain_id));
                    
                    //let c = Chain::from_id(c.try_into().unwrap());
                    //log(format!("chain  {:#?}", c).as_str());
                }),
            );
        }
        {
            let mut this = self.clone();
            _ = injected.clone().on(
                "accountsChanged",
                Box::new(move |accounts| {
                    let accs: Vec<String> = serde_wasm_bindgen::from_value(accounts).unwrap();
                    let accs_p: Vec<Address> = accs
                        .into_iter()
                        .map(|a: String| return Address::from_str(&a).unwrap_or(Address::ZERO))
                        .collect();
                    this.accounts = Some(accs_p.clone());
                    this.emit_event(Event::AccountsChanged(Some(accs_p)));
                }),
            );
        }
        self.emit_event(Event::Connected);
        if self.chain_id.is_some() {
            self.emit_event(Event::ChainIdChanged(self.chain_id));
        }
        if self.accounts.is_some() {
            self.emit_event(Event::AccountsChanged(self.accounts.clone()));
        }

        Ok(())
    }

    pub async fn sign_typed_data<T: Send + Sync + Serialize>(
        &self,
        data: T,
        from: &Address,
    ) -> Result<String, EthereumError> {
        match &self.wallet {
            WebProvider::None => Err(EthereumError::NotConnected),
            WebProvider::Injected(provider) => Ok(provider.sign_typed_data(data, from).await?),
            WebProvider::WalletConnect(provider) => {
                Ok(format!("{:#?}",provider.sign_typed_data(data, from).await?))
            }
        }
    }

    async fn connect_wc(&mut self) -> Result<(), EthereumError> {
        if !self.walletconnect_available() {
            return Err(EthereumError::Unavailable);
        }

        let this = self.clone();
        let mut wc = WalletConnect::connect(
            self.wc_project_id.clone().unwrap().into(),
            self.chain_id.unwrap_or_else(|| 1),
            self.metadata.clone(),
            Some(Box::new(move |event| match event {
                WCEvent::Connected => this.emit_event(Event::Connected),
                WCEvent::Disconnected => this.emit_event(Event::Disconnected),
                WCEvent::ChainChanged(chain_id) => {
                    this.emit_event(Event::ChainIdChanged(Some(chain_id)))
                }
                WCEvent::AccountsChanged(accounts) => {
                    let accounts_parsed = accounts.into_iter().map(|a| return Address::from_slice(a.as_bytes())).collect();
                    this.emit_event(Event::AccountsChanged(Some(accounts_parsed)))
                }
            })),
        )?;

        let url = wc.initiate_session().await?;

        self.wallet =
            WebProvider::WalletConnect(WalletConnectProvider::new(wc, self.rpc_node.clone()));

        self.emit_event(Event::ConnectionWaiting(url));

        Ok(())
    }

    async fn request_accounts(&self) -> Result<Vec<String>, EthereumError> {
        match &self.wallet {
            WebProvider::None => Err(EthereumError::NotConnected),
            WebProvider::Injected(_) => Ok(self.clone().request("eth_requestAccounts", ()).await?),
            WebProvider::WalletConnect(wc) => match wc.accounts() {
                Some(accounts) => {
                    let accounts_parsed = accounts
                        .into_iter()
                        .map(|a| a.to_string())
                        //.map(|a| return Address::from_slice(a.as_bytes()))
                        .collect();               
                    Ok(accounts_parsed)
                },
                None => Err(EthereumError::Unavailable),
            },
        }
    }

    async fn request_chain_id(&self) -> Result<U256, EthereumError> {
        match &self.wallet {
            WebProvider::None => Err(EthereumError::NotConnected),
            WebProvider::Injected(_) => Ok(self.clone().request("eth_chainId", ()).await?),
            WebProvider::WalletConnect(wc) => Ok(wc.chain_id().try_into().unwrap()),
        }
    }

    fn emit_event(&self, event: Event) {
        if let Some(listener) = &self.listener {
            listener(event);
        }
    }
}

// originally an impl of ethers::JsonRpcClient for Ethereum
//#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
//#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Ethereum {
    //type Error = EthereumError; 
    async fn request<T: Serialize + Send + Sync, R: DeserializeOwned + Send>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, EthereumError> {
        match &self.wallet {
            WebProvider::None => Err(EthereumError::NotConnected),
            WebProvider::Injected(provider) => Ok(provider.request(method, params).await?),
            WebProvider::WalletConnect(provider) => Ok(provider.request(method, params).await?),
        }
    }
}

//#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[async_trait(?Send)]
pub trait WebClient {
    fn send(&self, req: SerializedRequest) -> Pin<Box<dyn Future<Output = Result<Response, TransportError>> + Send + 'static>>;
    fn send_packet(&self, req: RequestPacket) -> Pin<Box<dyn Future<Output = Result<ResponsePacket, TransportError>> + Send>> ;
}
#[async_trait(?Send)]
impl WebClient for Ethereum {
    fn send(
        &self,
        req: SerializedRequest,
    ) -> std::pin::Pin<Box<dyn Future<Output = Result<Response, alloy_json_rpc::RpcError<TransportErrorKind>>> + Send + 'static>> {
        match &self.wallet {
            WebProvider::None => panic!("Unavailable"),//Err(EthereumError::NotConnected),
            WebProvider::Injected(provider) => provider.send(req),
            WebProvider::WalletConnect(_provider) => panic!("Unavailable")//Err(EthereumError::Unavailable)//Ok(provider.request(req).await?),
        }
    }
    fn send_packet(&self, req: RequestPacket) -> Pin<Box<dyn Future<Output = Result<ResponsePacket, TransportError>> + Send>> {
        match &self.wallet {
            WebProvider::None => panic!("Unavailable"),
            WebProvider::Injected(provider) => provider.send_packet(req),
            WebProvider::WalletConnect(_provider) => panic!("Unavailable")
        }
    }
}  

impl tower::Service<RequestPacket> for Ethereum {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    #[inline]
    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
/*         if self.tx.is_closed() {
            return std::task::Poll::Ready(Err(TransportErrorKind::backend_gone()));
        } */
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.send_packet(req)
    }
}

impl tower::Service<RequestPacket> for &Ethereum {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
/*         if self.tx.is_closed() {
            return std::task::Poll::Ready(Err(TransportErrorKind::backend_gone()));
        } */
        std::task::Poll::Ready(Ok(()))
    }

    #[inline]
    fn call(&mut self, req: RequestPacket) -> Self::Future {
        self.send_packet(req)
    }
}

impl TransportConnect for Ethereum {
    type Transport = Ethereum;

    fn is_local(&self) -> bool {
        false
    }
    fn get_transport<'a: 'b, 'b>(&'a self) -> alloy_transport::Pbf<'b, Self::Transport, TransportError> {
        Box::pin(async { Ok(self.to_owned()) } )
    }
}
