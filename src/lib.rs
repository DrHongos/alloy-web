pub mod eip1193;    // change to DotEthereum (window.ethereum is not eip1193 convention)
pub mod helpers;
pub mod errors;
pub mod builder;

//pub mod walletconnect;
/* 
Refactor and clean this up
- how to use and test wallet-connect?   -> Re-implement

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
    borrow::Cow,
};
use unsafe_send_sync::UnsafeSendSync;
use wasm_bindgen::prelude::*;
use async_trait::async_trait;
use futures::Future;

use alloy_primitives::{
    Address, U256, U64,
    BlockHash, Bytes, TxHash
};
use alloy_json_rpc::{SerializedRequest, Response, RequestPacket, ResponsePacket};
use alloy_transport::{TransportError, TransportErrorKind, TransportFut, TransportConnect, Transport, BoxTransport, TransportResult}; 
use alloy_rpc_client::{/* RpcCall,  */RpcClient, ClientBuilder};
use alloy_chains::Chain;
use alloy_rpc_types::{
    BlockId, BlockNumberOrTag, 
    Block, FeeHistory, Filter, Log, RpcBlockHash, SyncStatus,
    Transaction, TransactionReceipt, TransactionRequest,
};
use eip1193::Eip1193;
use crate::errors::EthereumError;

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
    fn new(
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

    pub fn disconnect(&mut self) {
        //if let WebProvider::WalletConnect(wc) = &self.wallet {
        //    wc.disconnect();
        //}
        self.wallet = WebProvider::None;
        //self.accounts = None;
        //self.chain_id = None;

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

// copied from alloy idea..
// como actualizar los campos from & chain desde eventos? testear!
#[derive(Clone)]
pub struct Provider<T: Transport = BoxTransport> {
    inner: Arc<RpcClient<T>>,
    pub from: Vec<Address>,
    pub chain: Chain,
}
impl Provider<BrowserTransport> {
    pub async fn new(eth: BrowserTransport) -> Self {
        let client = ClientBuilder::default()
            .connect(eth).await.expect("cannot create client");        
        let aclient = Arc::new(client);
        //let bclient = aclient.clone();
        let accounts: Vec<Address> = aclient.clone().prepare("eth_requestAccounts", Cow::<()>::Owned(())).await.expect("Could not get accounts");
        let chain_id_r: U64 = aclient.clone().prepare("eth_chainId", Cow::<()>::Owned(())).await.expect("Could not get chain id");
        let chain_id: u64 = chain_id_r.try_into().unwrap_or(1);
        Self {
            inner: aclient,
            from: accounts,
            chain: Chain::from_id(chain_id)
                //.to_u64().expect("Error on chain id")),
        }
    }

    // mostly copied fns from alloy_provider
    pub async fn get_transaction_count(
        &self,
        address: Address,
    ) -> TransportResult<U256> {
        self.inner
            .prepare(
                "eth_getTransactionCount",
                Cow::<(Address, &'static str)>::Owned((address, "latest")),
            )
            .await
    }

    // Gets a block by its [BlockHash], with full transactions or only hashes.
    pub async fn get_block_by_hash(
        &self,
        hash: BlockHash,
        full: bool,
    ) -> TransportResult<Option<Block>> {
        self.inner
            .prepare("eth_getBlockByHash", Cow::<(BlockHash, bool)>::Owned((hash, full)))
            .await
    }

    // Gets a block by [BlockNumberOrTag], with full transactions or only hashes.
    pub async fn get_block_by_number<B: Into<BlockNumberOrTag> + Send + Sync>(
        &self,
        number: B,
        full: bool,
    ) -> TransportResult<Option<Block>> {
        self.inner
            .prepare(
                "eth_getBlockByNumber",
                Cow::<(BlockNumberOrTag, bool)>::Owned((number.into(), full)),
            )
            .await
    }    

    pub async fn get_code_at<B: Into<BlockId> + Send + Sync>(
        &self,
        address: Address,
        tag: B,
    ) -> TransportResult<Bytes> {
        self.inner
            .prepare("eth_getCode", Cow::<(Address, BlockId)>::Owned((address, tag.into())))
            .await
    }

    /// Gets a [Transaction] by its [TxHash].
    pub async fn get_transaction_by_hash(&self, hash: TxHash) -> TransportResult<Transaction> {
        self.inner
            .prepare(
                "eth_getTransactionByHash",
                // Force alloy-rs/alloy to encode this an array of strings,
                // even if we only need to send one hash.
                Cow::<Vec<TxHash>>::Owned(vec![hash]),
            )
            .await
    }

    pub async fn get_logs(&self, filter: Filter) -> TransportResult<Vec<Log>> {
        self.inner.prepare("eth_getLogs", Cow::<Vec<Filter>>::Owned(vec![filter])).await
    }
    
    /// Gets the current gas price.
    pub async fn get_gas_price(&self) -> TransportResult<U256> {
        self.inner.prepare("eth_gasPrice", Cow::<()>::Owned(())).await
    }    

    // copy pasted from Provider implementation (file:///home/drhongos/Documents/yew_ethers/alloy/target/doc/src/alloy_providers/provider.rs.html)
    // add the rest of functions of provider:
    pub async fn get_chain_id(&self) -> TransportResult<U64> {
        self.inner.prepare("eth_chainId", Cow::<()>::Owned(())).await
    }

    pub async fn get_accounts(&self) -> TransportResult<Vec<Address>> {
        self.inner.prepare("eth_accounts", Cow::<()>::Owned(())).await
    } 
    
    pub fn inner(&self) -> &RpcClient<BrowserTransport> {
        &self.inner
    }

    /// Gets the last block number available.
    pub async fn get_block_number(&self) -> TransportResult<U64> {
        self.inner.prepare("eth_blockNumber", Cow::<()>::Owned(())).await
    }

    /// Gets the balance of the account at the specified tag, which defaults to latest.
    pub async fn get_balance(
        &self,
        address: Address,
        tag: Option<BlockId>,
    ) -> TransportResult<U256> {
        self.inner
            .prepare(
                "eth_getBalance",
                Cow::<(Address, BlockId)>::Owned((
                    address,
                    tag.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest)),
                )),
            )
            .await
    }

    /// Gets a [TransactionReceipt] if it exists, by its [TxHash].
    pub async fn get_transaction_receipt(
        &self,
        hash: TxHash,
    ) -> TransportResult<Option<TransactionReceipt>> {
        self.inner.prepare("eth_getTransactionReceipt", Cow::<Vec<TxHash>>::Owned(vec![hash])).await
    }

       /// Returns a collection of historical gas information [FeeHistory] which
    /// can be used to calculate the EIP1559 fields `maxFeePerGas` and `maxPriorityFeePerGas`.
    pub async fn get_fee_history<B: Into<BlockNumberOrTag> + Send + Sync>(
        &self,
        block_count: U256,
        last_block: B,
        reward_percentiles: &[f64],
    ) -> TransportResult<FeeHistory> {
        self.inner
            .prepare(
                "eth_feeHistory",
                Cow::<(U256, BlockNumberOrTag, Vec<f64>)>::Owned((
                    block_count,
                    last_block.into(),
                    reward_percentiles.to_vec(),
                )),
            )
            .await
    }

    /// Gets the selected block [BlockNumberOrTag] receipts.
    pub async fn get_block_receipts(
        &self,
        block: BlockNumberOrTag,
    ) -> TransportResult<Vec<TransactionReceipt>> {
        self.inner.prepare("eth_getBlockReceipts", Cow::<BlockNumberOrTag>::Owned(block)).await
    }

    /// Gets an uncle block through the tag [BlockId] and index [U64].
    pub async fn get_uncle<B: Into<BlockId> + Send + Sync>(
        &self,
        tag: B,
        idx: U64,
    ) -> TransportResult<Option<Block>> {
        let tag = tag.into();
        match tag {
            BlockId::Hash(hash) => {
                self.inner
                    .prepare(
                        "eth_getUncleByBlockHashAndIndex",
                        Cow::<(RpcBlockHash, U64)>::Owned((hash, idx)),
                    )
                    .await
            }
            BlockId::Number(number) => {
                self.inner
                    .prepare(
                        "eth_getUncleByBlockNumberAndIndex",
                        Cow::<(BlockNumberOrTag, U64)>::Owned((number, idx)),
                    )
                    .await
            }
        }
    }

    /// Gets syncing info.
    pub async fn syncing(&self) -> TransportResult<SyncStatus> {
        self.inner.prepare("eth_syncing", Cow::<()>::Owned(())).await
    }

    /// Execute a smart contract call with [TransactionRequest] without publishing a transaction.
    pub async fn call(
        &self,
        tx: TransactionRequest,
        block: Option<BlockId>,
    ) -> TransportResult<Bytes> {
        self.inner
            .prepare(
                "eth_call",
                Cow::<(TransactionRequest, BlockId)>::Owned((
                    tx,
                    block.unwrap_or(BlockId::Number(BlockNumberOrTag::Latest)),
                )),
            )
            .await
    }

    /// Estimate the gas needed for a transaction.
    pub async fn estimate_gas(
        &self,
        tx: TransactionRequest,
        block: Option<BlockId>,
    ) -> TransportResult<Bytes> {
        if let Some(block_id) = block {
            let params = Cow::<(TransactionRequest, BlockId)>::Owned((tx, block_id));
            self.inner.prepare("eth_estimateGas", params).await
        } else {
            let params = Cow::<TransactionRequest>::Owned(tx);
            self.inner.prepare("eth_estimateGas", params).await
        }
    }

    /// Sends an already-signed transaction.
    pub async fn send_raw_transaction(&self, tx: Bytes) -> TransportResult<TxHash> {
        self.inner.prepare("eth_sendRawTransaction", Cow::<Bytes>::Owned(tx)).await
    }
   
}

impl Debug for Provider<BrowserTransport> {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "Ethereum browser provider with accounts {:?} connected in chain {:?}",
            self.from, self.chain,
        )
    }
}