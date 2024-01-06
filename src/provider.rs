use alloy_transport::{Transport, BoxTransport, TransportResult}; 
use alloy_primitives::{Address, U64, U256, BlockHash, Bytes, TxHash};
use std::sync::Arc;
use alloy_rpc_client::{RpcClient, ClientBuilder};
use super::BrowserTransport;
use alloy_chains::Chain;
use std::{
    borrow::Cow,
    fmt::{Debug, Formatter, Result as FmtResult},
};
use alloy_rpc_types::{
    BlockId, BlockNumberOrTag, 
    Block, FeeHistory, Filter, Log, RpcBlockHash, SyncStatus,
    Transaction, TransactionReceipt, TransactionRequest,
};
use wasm_bindgen::prelude::*;

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
// copied from alloy idea..
// como actualizar los campos from & chain desde eventos? testear!
#[derive(Clone)]
pub struct Provider<T: Transport = BoxTransport> {
    inner: Arc<RpcClient<T>>,
    pub from: Vec<Address>,
    pub chain: Chain,
}
impl Provider<BrowserTransport> {
    pub async fn new(eth: BrowserTransport) -> Option<Self> {
        let client = ClientBuilder::default()
            .connect(eth).await.expect("cannot create client");        
        let aclient = Arc::new(client);
        //let bclient = aclient.clone();
        //crate::helpers::log(format!("{:#?}", aclient).as_str());
        let accounts: Vec<Address> = aclient
            .clone()
            .prepare("eth_requestAccounts", Cow::<()>::Owned(()))
            .await
            .unwrap_or(Vec::new());
        if accounts.len() > 0 {
            let chain_id_r: U64 = aclient.clone().prepare("eth_chainId", Cow::<()>::Owned(())).await.expect("Could not get chain id");
            let chain_id: u64 = chain_id_r.try_into().unwrap_or(1);
            Some(Self {
                inner: aclient,
                from: accounts,
                chain: Chain::from_id(chain_id)
            })
        } else {
            None
        }
    }
    pub async fn resume() -> Result<Option<Self>, JsValue> {
        match get_accounts_js().await {
            Ok(d) => {
                let v: Vec<Address> = serde_wasm_bindgen::from_value(d).unwrap();
                //crate::helpers::log(format!("eth_accounts {:?}", v).as_str());
                if v.len() > 0 {
                    let bt = BrowserTransport::new();
                    let cl = ClientBuilder::default().connect(bt).await.expect("rpc client error");
                    //let chain_id = re connect chain
                    Ok(Some(Self {
                        inner: Arc::new(cl),
                        from: v,
                        chain: Chain::mainnet()
                    }))
                } else {
                    Ok(None)
                }
            },
            Err(e) => {
                crate::helpers::log(format!("Booo {:#?}", e).as_str());
                Err(e)
            }
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