# mw-btc-swap
Implementation for my masters thesis

## requirements for building from source

* g++
* pkg-config
* rust + cargo
* libssl-dev

# commands

## init

Creates a new Atomic Swap Offer. Need to specifc the source and target currency and the swap amounts.
It will create a private and public slate file in your slate directory (configurable in settings.json).
The public slate file can be published and used by a peer to accept the offer.
The command takes the following required arguments:

--from-currency <BTC|GRIN> The currency you hold and want to offer

--to-currency <BTC|GRIN> The currency you want to receiver

--from-amount <amount> Offered amount in Satoshis or Nanogrin

--to-amount <amount> The amount you want to receive in Satoshis or Nanogrin

--timeout <minutes> Once the swap has started what is the amount of minutes until it should cancel and timeout.

Let say we have Alice and Bob. Bob owns 0.03566 BTC which he wants to swap for Grin coins. He looks at the current exchange rate and sees that he should be able to receive around 5000 Grin for his his Bitcoin. The command line uses the smallest units (satoshis and nanogrin) for each chain so:

0.03566 BTC = 3566000 sats
5000 Grin = 5000000000000 nanogrin

He chooses a 10 hour timeout, meaning that if the swap (once started) does not complete within this time limit both participents can redeem original inputs. 

The call on the command line looks like this:

`./mw-btc-swap init --from-amount 3566000 --from-currency BTC --to-amount 5000000000000 --to-currency GRIN --timeout 600`

The program created a new swap with the id `8715159615153475876` and the files `8715159615153475876.prv.json`, `8715159615153475876.pub.json`

## import

The import command is used to import outputs the participent owns into the program.
Currently it is supported to import Bitcoin and Grin keys by providing the `btc` or `grin` subcommand

### btc

To import btc outputs one has to provide the following required arguments

--swapid <integer> The id of the swap (as previously output by init)

--secret <string> The private key encoded as a hexadecimal string

--txid <string> The id (hash) of the unspent transaction output

--vout <integer> The number of the output of the unspent transaction which should be spent

--value <integer> The value of the output which we are importing given in satoshis

### grin

To import grin coins one has to provide the following required arguments

--swapid <integer> The id of the swap (as previously output by init)

--commitment <string> The commitment of the input which should be spent

--blinding_factor <string> The hexadecimal encoded blindingfactor to the coin

--value <integer> The value of the coin commitment

In our previous example Bob now would have to import UTXOs of minimum value `3566000` sats in order to start offering the swap.
Lets say Bob has `0.27338479 BTC` stored in a transaction with the id `ac3947090566ffa1780caa0348ea1638a2b0bc5b4ca7f37f5822fadd9f37ae58` as the second output to vout has to be `1` (counting from 0). The private key to the output is `E9873D79C6D87DC0FB6A5778633389F4453213303DA61F20BD67FC233AA33262` so the import command would look like:

`./mw-btc-swap import btc --swapid 8715159615153475876 --secret    --txid ac3947090566ffa1780caa0348ea1638a2b0bc5b4ca7f37f5822fadd9f37ae58 --vout 1 --value 27338479`

If we look into `8715159615153475876.prv.json` we can see that now the Bitcoin UTXO has been imported. Since the value is greater then `3566000` (the amount Bob wants to swap) he can now start listening for a trading counterpart using the listen command.

```json
{
   "mw":{
      "inputs":[
         
      ],
      "partial_key":0
   },
   "btc":{
      "inputs":[
         {
            "txid":"ac3947090566ffa1780caa0348ea1638a2b0bc5b4ca7f37f5822fadd9f37ae58",
            "vout":1,
            "value":27338479,
            "secret":"0xE9873D79C6D87DC0FB6A5778633389F4453213303DA61F20BD67FC233AA33262"
         }
      ],
      "witness":0
   }
}
```

## listen 

The listen command starts the TCP server to listen and wait for a peer to run the Atomic Swap protocol with. 
Note that this command can only be started once enough inputs or coins have been imported such that a swap can take place.
The command takes the following required arguments:

--swapid <integer> the id of the atomic swap for which we want to start listening

The example command would then look like:

`./mw-btc-swaps listen --swapid 8715159615153475876`

## accept

The accept command will take the public file provided by a peer and create the private file for it.
After running it the peer has to import the respective funds into the private swap file using the import command
The command takes the following mandatory argument:

--swapid <integer> the id of the atomic swap for which you have received the public file. (Make sure the file was placed into the correct directory)

## execute

The execute command will connect to the peer's TCP server and start the Atomic Swap protocol for which messages will be exchanged via TCP
The command takes the following mandatory argument:

--swapid <integer> the id of the atomic swap which we want to start