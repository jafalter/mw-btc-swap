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

#[derive(PartialEq)]
pub enum SwapStatus {
    INITIALIZED,
    SETUP,
    EXECUTING,
    FINISHED
}