# mw-btc-swap
Implementation for my masters thesis

## requirements for building from source

* g++
* pkg-config
* rust + cargo
* libssl-dev
* libclang-dev

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

`./mw-btc-swap init --from-currency BTC --to-currency GRIN --from-amount 1600 --to-amount 1500000 --timeout 600`

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

`./mw-btc-swap import btc --swapid 8715159615153475876 --sk cPg1qrQrVDc6fvwSHWkGg64gVZHxekXQ7hU2AizkKWCpPxXvJm5J  --txid 3f11e68ec0798b3f550c99b232353f51ba9a2442c731580e521777c79c1829da --vout 1 --value 1826996 --pub_script 76a914137aabb97216f7bdf4d5f4a53fc9504b0dcc396488ac`

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

After having received the public swap file from Bob, Alice can call the accept command as follows:

`./mw-btc-swap accept --swapid 8715159615153475876`

She will then import her grin as following:

`./mw-btc-swap import grin --swapid 8715159615153475876 --commitment 09257c975816e6ba6e9a66d1956a202b80d2cd25889a6bef2db0542d51fad6df8e --blinding_factor afa38b309656a60024064b045ce30209c7fd5d406aa2e9216b74287f7425da41 --value 2000000000`

## setup

The setup command will run the setup phase of the protocol. It takes a single mandatory argument:

--swapid <integer> the id of the atomic swap

`./mw-btc-swap accept --swapid 8715159615153475876`

It will first attempt to establish a TCP connection with the second party and exchange a checksum of the public swap file. If the checksum matches they will start the protocol. First of all both parties create and exchange public keys that they want to use on the Bitcoin side. 
The holder of the BTC (in our case Bob) additionally creates a keypair (`x`, `pub_x`) where `x` is the secret witness that the grin holder (Alice) later needs for unlocking the Bitcoin.
In the next step Bob will create the lock output on the Bitcoin side, which will be redeemable by Alice if she posseses her secret key and `x` or by Bob after a certain block number. 
In our example the address is [2NCJDq4YRQ9C83fgvepMqU2D9kE4x7h36Ji](https://live.blockcypher.com/btc-testnet/address/2NCJDq4YRQ9C83fgvepMqU2D9kE4x7h36Ji/).
In the transaction [10536404873e6ae133afde600b5630d6a00f3be0b9dde01a248c6f13a00b3a4b](https://live.blockcypher.com/btc-testnet/tx/10536404873e6ae133afde600b5630d6a00f3be0b9dde01a248c6f13a00b3a4b/) 0.000016 BTC have been transferred to the lock by Bob.
Alice will verify that the funds are locked on the address (which she can compute herself) and once that is done start the protocol for creating the lock coins on the Grin side. Creating the Grin transaction requires interaction by both Alice and Bob, again they exchange the necessary messages via the already established TCP channel.
The funding transaction on Grin side was mined in block [718594](https://floonet.grinscan.net/block/718594) on the Grin testnet.
[08c2e1a98f5fd328cc67b7df5ab9fdee9cf0c1c1f166d5d08a02a578945fdf607](https://floonet.grinscan.net/output/08c2e1a98f5fd328cc67b7df5ab9fdee9cf0c1c1f166d5d08a02a578945fdf6076) is the lock commitment for which Alice and Bob each share a part of the blinding factor.
Now immediatly after, they create a second transaction which spends this coin and sends it back to Alice as a refund.
In fact Alice must not publish the funding transaction to the network before this refunding transaction has been completed, othermise her funds might be lost if Bob refused to further cooperate.
The refund transaction is saved in Alice's private slate file.
Now with the funds locked on both sides this concludes the setup phase.

The public slate file now looks like this

```json
{
  "status": "SETUP",
  "mw": {
    "amount": 100000000,
    "timelock": 10,
    "lock_time": 718559,
    "swap_type": "REQUESTED"
  },
  "btc": {
    "amount": 1600,
    "timelock": 1,
    "swap_type": "OFFERED",
    "lock_time": 1937142,
    "pub_a": "03f74b8534e9d18b7cfede70ba128ffe2f9a34a7bfe0c5fe82cc3389b0b14e0ebd",
    "pub_b": "0333220c416f2489268a6aeccec1080f293c4f2be64a3eb43041582e7ab67eddff",
    "pub_x": "02ef1471a79d9ba889feace1683bb112de137f78bfdd1a63f2db5f3189f67da68e"
  },
  "meta": {
    "server": "bob",
    "port": "80"
  }
}
```
The specific lock times have been added for both chains and the public keys for the bitcoin lock are added.
Alice private file looks like this:

```json
{
  "mw": {
    "inputs": [
      {
        "commitment": "089fd9f31b23932b8a694b238a22a80f1410bf96e48fbd62d8cda07729b2585c94",
        "blinding_factor": "42716b7b7fab51df06bac7bac71b89cb44be6ddfa3e7b02a7370afeff3a46d77",
        "value": 1289000000
      }
    ],
    "partial_key": 0,
    "shared_coin": {
      "commitment": "0855d1ec1d11aaa1726e5ca53524dc148343a254a71f33324d5d3961e2ebac6e81",
      "blinding_factor": "354f7a8fe187247d00abbbbf21809081a82c18ae066601106700cae6f7ec987b",
      "value": 100000000
    },
    "change_coin": {
      "commitment": "0942baf086436b2d0dfd986a3430fba1f8a38574a7d35b1e182d9b1dbe00ffdf23",
      "blinding_factor": "dcd7594b26ba36c18e97bfcc81b4cfc345c68110301a5ded97cf4361120889a5",
      "value": 1166000000
    },
    "refund_coin": {
      "commitment": "084ee621e95240cec68a8bf09e940d4327b6c53120535dfa9e81d524b58cac8141",
      "blinding_factor": "63cc807ecc7616f00227a0e2eff91c7d3a6381b26ca38b20be1cd4297bb3d4f2",
      "value": 87500000
    },
    "swapped_coin": null,
    "refund_tx": {
      "offset": "d40be920f677b8ab60b39db155cb2353dc0ba83c24f7e3b3ca2bac96e4374ae6",
      "body": {
        "inputs": [
          {
            "features": "Plain",
            "commit": "0855d1ec1d11aaa1726e5ca53524dc148343a254a71f33324d5d3961e2ebac6e81"
          }
        ],
        "outputs": [
          {
            "features": "Plain",
            "commit": "084ee621e95240cec68a8bf09e940d4327b6c53120535dfa9e81d524b58cac8141",
            "proof": "7166d04c397b8e2744338fbce7825cdc3bb266e1a3e563fdc568e880cb5b34a44438a539a2b3d5019c35ceeeb12219fe76693774dd6d43b651dd14fc144f0d0c0a8ec28b25087c057d8f8b23ea1a4b8edfc1268b3c8e0394e786aeead245f49a86b25935989b546b51e14097b79ee7dd80c60eccb30e1b4a62c339a726ff44df073bd502f1843cbd76af52718e810df908b5597995250604c7345521a20f66e6658eb6f82d41f155e2c759a7587a41497924d8fc5164abaa4409dc748b47214e948d7d626d846428bb530bbc5d0952783bc2e6e7443b0052f9cf48067814e9ddf4beb60a0a3c921f1f2af4c2969b4a4d5aeeeee54bb91a560c281839f4063b27dab90412908f57206a00596e422e988b9d5b92672a258d615c6c99f550327d518c827f24a1a5c77e6b36551ad2ab4d25b318a5807769176ebfc31e6683eb39c9b617612f897bd02362d46d266721def4aad107704e074ea46a6e80164f30407512430268fc6a4bc852e8fde02516f83416447f3443b725d15d91ae9a49c913daa96a52587e4ce492cd46ad6fbb1421feacbc04971f4f57b8271e05ccbe17e25b80ba316dd029d3f145e1aacbb470f694c25182dde9bd85ddedfbefe553fabf159442da86491f4f5ff4c8bd1e7628ce0ea0ddf273c7918c4fcc22f2117f61c09af7d770d2add882db0401a592770d67bef0e1f4f3afafc8704b3f62095cc61ba9a4baaf2b25823e24182a9fbd86ce3d26dd0a1583348e3d4590865c944cd984fb3f143082cacb5fa97f5d4045d25581b594decb4f45cad5f0077424f11c225b055e9df2e338d25dde5c2d6f117263eef52a7898ac8bc97326fc59c238171b4ee7eab48f57d8ee13713cf5f7ce41cfeebc3916419f49e479da4beb9fca8a42fcf252ca8cd7475fba08b82954faf72f2227c067f0e1293e5936a2ce3f06e70c4e3cc3a814"
          }
        ],
        "kernels": [
          {
            "features": {
              "HeightLocked": {
                "fee": 12500000,
                "lock_height": 718559
              }
            },
            "excess": "082696db1d8844194c4575e7d8899b5205b5a68036dd68fc4701dc1a87fee47332",
            "excess_sig": "f545007a2dfb6ef1af91f7c8cd4e26769cdff84ff2d875e911fbf0ff6ff5b476738cf8f2ae8a0880a115640b305aa50455f040c9581a1be7081160253d78cd72"
          }
        ]
      }
    }
  },
  "btc": {
    "inputs": [],
    "witness": 0,
    "sk": "cQUHaGqiKr68gxhKY1jEnq6pek2eR5cUJBfHCvjfbhUVvszuD4Y5",
    "x": null,
    "r_sk": null,
    "change": null,
    "swapped": null,
    "lock": {
      "txid": "b0c9fd94adc501f7332b86ce6e872b0ed4015eaab0564ec2f4106bfdacbce0b1",
      "vout": 0,
      "value": 1600,
      "secret": "cQUHaGqiKr68gxhKY1jEnq6pek2eR5cUJBfHCvjfbhUVvszuD4Y5",
      "pub_key": "03f74b8534e9d18b7cfede70ba128ffe2f9a34a7bfe0c5fe82cc3389b0b14e0ebd",
      "pub_script": "a914afbb72dceed72da9ecb0bbbf892860b21b9ab99087"
    },
    "refunded": null
  }
}
```
We have spending information for Alice change output on the Grin side, her share of the shared coin, as well as the refund coin spending information in case the swap is cancelled. 
It also contains the transaction doing the refunding which can be mined after block 718559 on the grin network. 
The file also contains the secret key used in the Bitcoin lock.
On Bobs side the private file looks as follows:
```json
{
  "mw": {
    "inputs": [],
    "partial_key": 0,
    "shared_coin": {
      "commitment": "0855d1ec1d11aaa1726e5ca53524dc148343a254a71f33324d5d3961e2ebac6e81",
      "blinding_factor": "f3001625755f9187c8c4369b7763d315fa2532f1b2d7e5e713673d5ca52443f3",
      "value": 100000000
    },
    "change_coin": null,
    "refund_coin": null,
    "swapped_coin": null,
    "refund_tx": null
  },
  "btc": {
    "inputs": [
      {
        "txid": "891e27ce988254fc53dd5c67524e3fed1b5864aaccae9f483c046ac1da05622a",
        "vout": 1,
        "value": 1929577,
        "secret": "cQauQu4gtfcnJW3nrbKe1Jz9y66G2H2Fme2ScMPvG6L5hKW8fjdA",
        "pub_key": "026279d52d042833268a9c22194a5fa6ea60ec1dc36106a684567603e6f211f2b6",
        "pub_script": "76a914baa9a6fbc3658f36643d0a3dc0c8322e5ed7f0b488ac"
      }
    ],
    "witness": 0,
    "sk": "cRaP4RjfGhRLytSYuEyhacVPG6UwvqMdhtkuwaNG9LRsKEtZE8Ct",
    "x": "cQPkea4bUkTi9uCLGHPPsXBskHcz8kjpYfGGvAWgy1t2t9NafbdB",
    "r_sk": "cPYyS85ng2ePmxnBzfc1rRsjK3NtvkmEZNXLHyNMKk3unnPujTjr",
    "change": {
      "txid": "b0c9fd94adc501f7332b86ce6e872b0ed4015eaab0564ec2f4106bfdacbce0b1",
      "vout": 1,
      "value": 1927477,
      "secret": "cPYyS85ng2ePmxnBzfc1rRsjK3NtvkmEZNXLHyNMKk3unnPujTjr",
      "pub_key": "038bc322a744a171ad5945e0bf9a2a317a5acce3ae9e9c315e7dabb690fe943e0d",
      "pub_script": "76a9142db771d55ec2ec6dfb5ce6ab9542d4e77377137b88ac"
    },
    "swapped": null,
    "lock": {
      "txid": "b0c9fd94adc501f7332b86ce6e872b0ed4015eaab0564ec2f4106bfdacbce0b1",
      "vout": 0,
      "value": 1600,
      "secret": "cRaP4RjfGhRLytSYuEyhacVPG6UwvqMdhtkuwaNG9LRsKEtZE8Ct",
      "pub_key": "0333220c416f2489268a6aeccec1080f293c4f2be64a3eb43041582e7ab67eddff",
      "pub_script": "a914afbb72dceed72da9ecb0bbbf892860b21b9ab99087"
    },
    "refunded": null
  }
}
```
Again it cointains Bobs share of the shared Grin output as well as the keys on the Bitcoin side.

## execute

The execute command will again connect to the peer's TCP server and execute the Atomic Swap protocol for which messages will be exchanged via TCP
The command takes the following mandatory argument:

`./mw-btc-swap execute --swapid 8715159615153475876`

--swapid <integer> the id of the atomic swap which we want to start

To be able to run this command the swap has to be first setup using the setup command, the second requirement is that there is enough time left to complete the swap. 
The program will first verify the current height of the networks and cancel if not enough time is left.
Not enough time is defined as 1 hour in blocks. (Calculated by average block time)
If enough time is left the execution will start, Alice and Bob will run the contract protocol on the Grin side to spend the shared coin to Bob while simultaniously revealing `x` to Alice by the use of adapted signatures. 
Completing this transaction Bob, can send it to the Grin network and now is in full possesion of the coins.
In our example this transaction was mined on the Grin testnet on block [718596](https://floonet.grinscan.net/block/718596), spending the locked funds to Bob's commitment {[09ef66334dc2e4c74732dafda8af3c32494eed5b23beb483d29d7ef32bf5c3ebb8](https://floonet.grinscan.net/output/09ef66334dc2e4c74732dafda8af3c32494eed5b23beb483d29d7ef32bf5c3ebb8).
After receiving `x` Alice can now create the required unlocking script on Bitcoin side. 
She will create a transaction sending the locked amount to her, signing it with both `x` and her secret key `sk_a` and broadcast the transaction to the network. 
With Alice now being in full possession of the Bitcoins, the swap is finished.
The redeem transaction on Bitcoin side is [aa2ab77482841571b6413c68de681830c61527bc6a90ef1781d6208d151fea10](https://live.blockcypher.com/btc-testnet/tx/aa2ab77482841571b6413c68de681830c61527bc6a90ef1781d6208d151fea10/), spending the locked funds to [n4pc2fJMqUzy6rivF8gKZy5eBXvDqvHvzo](https://live.blockcypher.com/btc-testnet/tx/aa2ab77482841571b6413c68de681830c61527bc6a90ef1781d6208d151fea10/) which is controlled by Alice.

## cancel

In the case that the setup phase of the Atomic Swap protocol was already finished, but the execute was not yet run, both parties have the option to cancel the swap. However, this will only work after the respective timeout has been reached on both chains!
The command takes the following mandatory argument:

--swapid <integer> the id of the atomic swap which we want to start

`./mw-btc-swap cancel --swapid 8715159615153475876`

It will again connect to Bob via TCP to initiate the cancellation of the swap. In this case Alice will simply publish the refund Grin transaction, which she already has. 
Bob will create a Bitcoin transaction spending the locked coins to himself while signing with his refund key.