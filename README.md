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

Let say we have Alice and Bob who would like to trade.
Alice owns 2 Grin (2000000000 Nanogrin) in the commitment 09257c975816e6ba6e9a66d1956a202b80d2cd25889a6bef2db0542d51fad6df8e of which she knows the opening.
Bob has 0.01826996 (1826996 sats) locked in a P2PKH address (mhHx61qiNcdFgXo722fDfMN4yRe1zH7bx8) for which he knows the unlocking information. Bob would like
to own some Grin and Alice BTC so agree to conduct a swap.
The exchange rate is that 1 Grin = 0.000011 BTC, Alice agrees to give 1.5 Grin (1500000000 Nanogrin) to Bob for which she wants 0.000016 BTC (1600 sats).
One of the two parties (in this case Bob) initiates the swap with the following command:

`./mw-btc-swap init --from-currency BTC --to-currency GRIN --from-amount 1600 --to-amount 1500000000 --timeout 600`

The program created a new swap with the id `8715159615153475876` and the files `8715159615153475876.prv.json`, `8715159615153475876.pub.json`

## import

The import command is used to import outputs/coins the participent owns into the program.
Currently it is supported to import Bitcoin and Grin keys by providing the `btc` or `grin` subcommand

### btc

To import btc outputs one has to provide the following required arguments

--swapid <integer> The id of the swap (as previously output by init)

--sk <string> The private key encoded in wif format

--txid <string> The id (hash) of the unspent transaction output

--vout <integer> The number of the output of the unspent transaction which should be spent

--value <integer> The value of the output which we are importing given in satoshis

--pub_script <string> The pub script (as hexadecimal) under which the Bitcoins are locked (currently only standard P2PKH is supported)

### grin

To import grin coins one has to provide the following required arguments

--swapid <integer> The id of the swap (as previously output by init)

--commitment <string> The commitment of the input which should be spent

--blinding_factor <string> The hexadecimal encoded blindingfactor to the coin

--value <integer> The value of the coin commitment

Now Bob needs to import the spending information of the UTXO that we wants to use for the swap. 

`./mw-btc-swap import btc --swapid 8715159615153475876 --sk cPg1qrQrVDc6fvwSHWkGg64gVZHxekXQ7hU2AizkKWCpPxXvJm5J  --txid 3f11e68ec0798b3f550c99b232353f51ba9a2442c731580e521777c79c1829da --vout 1 --value 273384791826996 --pub_script 76a914137aabb97216f7bdf4d5f4a53fc9504b0dcc396488ac`

If we look into `8715159615153475876.prv.json` we can see that now the Bitcoin UTXO has been imported. Since the value is greater then `1600` (the amount Bob wants to swap) he can now start listening for a trading counterpart using the listen command.

## listen 

The listen command starts the TCP server to listen and wait for a peer to run the Atomic Swap protocol with. 
Note that this command can only be started once enough inputs or coins have been imported such that a swap can take place.
The command takes the following required arguments:

--swapid <integer> the id of the atomic swap for which we want to start listening

The example command would then look like:

`./mw-btc-swap listen --swapid 8715159615153475876`

## accept

The accept command will take the public file provided by a peer and create the private file for it.
After running it the peer has to import the respective funds into the private swap file using the import command
The command takes the following mandatory argument:

--swapid <integer> the id of the atomic swap for which you have received the public file. (Make sure the file was placed into the correct directory)

After having received the public swap file from Bob Alice can call the accept command as follows:

`./mw-btc-swap accept --swapid 8715159615153475876`

She will then import her grin as following:

`./mw-btc-swap import grin --swapid 8715159615153475876 --commitment 09257c975816e6ba6e9a66d1956a202b80d2cd25889a6bef2db0542d51fad6df8e --blinding_factor afa38b309656a60024064b045ce30209c7fd5d406aa2e9216b74287f7425da41 --value 2000000000`

## execute

The execute command will connect to the peer's TCP server and start the Atomic Swap protocol for which messages will be exchanged via TCP
The command takes the following mandatory argument:

`./mw-btc-swap execute --swapid 8715159615153475876`

--swapid <integer> the id of the atomic swap which we want to start