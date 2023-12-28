use thiserror::Error;
use hex::FromHexError;
use crate::eip1193::Eip1193Error;
//use crate::walletconnect;

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
/* 
    #[error(transparent)]
    WalletConnectError(#[from] walletconnect::Error),
    #[error(transparent)]
    WalletConnectClientError(#[from] walletconnect_client::Error),
*/
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