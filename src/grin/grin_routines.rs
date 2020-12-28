use crate::grin::grin_types::MWCoin;
use grin_wallet_libwallet::Slate;
use grin_util::secp::pedersen::{Commitment};
use grin_core::core::transaction::OutputFeatures;
use grin_core::core::Input;

pub fn spend_coins(inputs : Vec<MWCoin>, fund_value : u64, timelock : u32, num_participants : usize) -> Slate {
    // Initial transaction slate
    let mut mwslate = Slate::blank(num_participants);
    
    // Add the input coins
    for coin in inputs {
        let commitment = Commitment::from_vec(hex::decode(coin.commitment).expect("Failed to decode commitment of input"));
        let input = Input {
            features : OutputFeatures::Plain,
            commit : commitment
        };
        mwslate.tx.body.inputs.push(input);
    }

    mwslate
}

#[cfg(test)]
mod test {
    use crate::grin::grin_types::MWCoin;
    use crate::grin::grin_routines::spend_coins;
    
    #[test]
    fn test_spend_coins() {
        let coin = MWCoin {
            commitment : String::from("086061571ea044365c81b5232c261866265024bd5c3506b5526d80df0c6c5845c8"),
            blinding_factor : String::from(""),
            value : 50000
        };
        let tx = spend_coins(vec!(coin), 600, 0, 2);
        let ser = serde_json::to_string(&tx).unwrap();
        println!("{}", ser);
    }
}