use serde::{Serialize, Deserialize};

#[derive(PartialEq)]
pub enum HttpMethod {
    GET,
    POST
}

#[derive(PartialEq)]
pub enum Currency {
    BTC,
    GRIN
}

pub fn parse_currency_from_string(cur : String) -> Currency {
    if cur.to_uppercase() == "BTC" || cur.to_uppercase() == "BITCOIN" {
        Currency::BTC
    }
    else if cur.to_uppercase() == "GRIN" {
        Currency::GRIN
    }
    else {
        panic!("Invalid Currency provided");
    }
}

#[derive(PartialEq, Serialize, Deserialize)]
pub enum SwapStatus {
    INITIALIZED,
    SETUP,
    EXECUTING,
    FINISHED
}

#[derive(PartialEq, Serialize, Deserialize)]
pub enum SwapType {
    OFFERED,
    REQUESTED
}