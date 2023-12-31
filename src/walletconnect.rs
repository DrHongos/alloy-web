/* 
WARNING WIP
- killed provider property 

*/

use hex::{FromHexError, decode};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{from_value, json};
use std::{
    cell::RefCell,
    fmt::{Debug, Formatter, Result as FmtResult},
    //str::FromStr,
    sync::Arc,
};
use thiserror::Error;
use unsafe_send_sync::UnsafeSendSync;
use walletconnect_client::prelude::*;
use alloy_primitives::Address;
use crate::helpers::serialize;
//use alloy_rpc_types::Signature;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Missing RPC provider")]
    MissingProvider,

    #[error(transparent)]
    SerdeJsonError(#[from] serde_json::Error),

    #[error(transparent)]
    WalletConnectError(#[from] WalletConnectError),
/* 
    #[error(transparent)]
    HttpClientError(#[from] HttpClientError),

    #[error(transparent)]
    SignatureError(#[from] SignatureError),
 */
    #[error(transparent)]
    HexError(#[from] FromHexError),
}

#[derive(Clone)]
pub struct WalletConnectProvider {
    client: UnsafeSendSync<Arc<RefCell<WalletConnect>>>,
//    provider: Option<UnsafeSendSync<RefCell<Http>>>,
}

impl Debug for WalletConnectProvider {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(
            f,
            "Wallet Connect provider" // "Wallet Connect signer {:?} chain id: {}",
                                      // self.address(),
                                      // self.chain_id()
        )
    }
}

impl WalletConnectProvider {
    pub fn new(client: WalletConnect, rpc_url: Option<String>) -> Self {
/*         let provider = match rpc_url {
            Some(url) => {
                if let Ok(p) = Http::from_str(&url) {
                    Some(UnsafeSendSync::new(RefCell::new(p)))
                } else {
                    None
                }
            }
            _ => None,
        };
        */
        Self {
            client: UnsafeSendSync::new(Arc::new(RefCell::new(client))),
//            provider,
        }
    }

    pub async fn disconnect(&self) {
        self.client.borrow_mut().disconnect();
    }

    pub fn chain_id(&self) -> u64 {
        self.client.borrow_mut().chain_id()
    }

    pub fn address(&self) -> Address {
        Address::from_slice(self.client.borrow_mut().address().as_bytes())
    }

    pub fn accounts(&self) -> Option<Vec<Address>> {
        let chain_id = self.client.borrow().chain_id();
        Some(self.client
            .borrow_mut()
            .get_accounts_for_chain_id(chain_id)
            .unwrap()
            .into_iter()
            .map(|a| Address::from_slice(a.as_bytes()))
            .collect())
    }

    /// Sends request via WalletConnectClient
    pub async fn request<T: Serialize + Send + Sync, R: DeserializeOwned + Send>(
        &self,
        method: &str,
        params: T,
    ) -> Result<R, Error> {
        let params = json!(params);

        let mut wc_client = self.client.borrow_mut();
        let chain_id = wc_client.chain_id();

        if wc_client.supports_method(method) {
            Ok(from_value(
                wc_client.request(method, Some(params), chain_id).await?,
            )?)
        } else {
            Err(Error::MissingProvider)
            /* 
            if let Some(provider) = &self.provider {
                Ok((*provider.borrow_mut()).request(method, params).await?)
            } else {
                Err(Error::MissingProvider)
            }
 */
        }
    }

    /// Builds typed data Json structure to send it to WalletConnect and sends via client's channel
    pub async fn sign_typed_data<T: Send + Sync + Serialize>(
        &self,
        data: T,
        from: &Address,
    ) -> Result<String, Error> {
        let data = serialize(&data);
        let from = serialize(&from.to_string());

        let sig: String = self.request("eth_signTypedData_v4", [from, data]).await?;
        let sig = sig.strip_prefix("0x").unwrap_or(&sig);

        let sig = decode(sig)?;
        Ok(serde_json::from_slice(sig.as_slice())?)
    }
}
