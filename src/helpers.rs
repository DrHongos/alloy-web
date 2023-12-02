use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {

    #[wasm_bindgen(js_namespace=["console"])]
    pub fn log(value: &str);    
}

// copied from ethers::utils
pub fn serialize<T: serde::Serialize>(t: &T) -> serde_json::Value {
    serde_json::to_value(t).expect("Failed to serialize value")
}