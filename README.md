# mw-btc-swap
Implementation for my masters thesis

## commands

### offer
Create an Atomic Swap Offer. Need to specifc the source and target currency and the swap amounts.
Will open a TCP server and print out a hexadecimal string which can be published and accepted by a peer.

--from-currency <BTC|GRIN> The currency you hold and want to offer

--to-currency <BTC|GRIN> The currency you want to receiver

--from-amount <amount> Offered amount in Satoshis or Nanogrin

--to-amount <amount> The amount you want to receive in Satoshis or Nanogrin

--timeout <minutes>

### accept
Accept the offer for a Atomic Swap by a peer. 
Will attempt to execute an atomic swap.
After your funds have been locked it will print a redeem token with which you can cancel the atomic swap, before it's execution finished.

-- offer <string>

### redeem
Redeem locked funds from a active swap.
Must be provided with the token printed during the accept command.

-- token <string>