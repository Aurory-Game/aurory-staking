<h1 align="center">
  <br>
   <img width="100" src="https://aurory.io/content/img/header/logo.png" alt="aurory logo"/>
  <br>
</h1>

# Reward Pool

Program for single token staking and receiving rewards. Ala xSushi.
Features:

- Stake / unstake
- Updatable lock up date
- Possibility to display the staked amount + reward separately.

## Design Overview

*TODO*

## Note

- **This code is unaudited. Use at your own risk.**
- In this implementation the xTokens are stored in an ATA.

## Developing

[Anchor](https://github.com/project-serum/anchor) is used for developoment, and it's
recommended workflow is used here. To get started, see the [guide](https://project-serum.github.io/anchor/getting-started/introduction.html).

### Build

```
anchor build --verifiable
```

The `--verifiable` flag should be used before deploying so that your build artifacts
can be deterministically generated with docker.

### Test

When testing locally, be sure to build with feature "local-testing" to enable the testing IDs. You can do this by editing `programs/step-staking/Cargo.toml` and uncommenting the default feature set line.

```
anchor test
```

Tested with:

- anchor-cli 0.18.2
- solana-cli 1.7.17

### Verify

To verify the program deployed on Solana matches your local source code, change directory
into the program you want to verify, e.g., `cd program`, and run

```bash
anchor verify <program-id | write-buffer>
```

### Deploy

To deploy the program, configure your CLI to the desired network/wallet and run

```bash
solana program deploy --programid <keypair> target/verifiable/step_staking.so
```

I would not suggest using anchor deploy at this time; it wouldn't/couldn't really add much value. Be sure to use `--programid <keypair>` to deploy to the correct address.

Note: By default, programs are deployed to accounts that are twice the size of the original deployment. Doing so leaves room for program growth in future redeployments. For this program, I beleive that's proper - I wouldn't want to limit it, nor do I see growth beyond double.

### Initial Migration

After deployment for the first time, you must point your `anchor.toml` file to the network you've deployed to and run

```bash
anchor migrate
```

This will call the `initialize` method to create the token vault. No specific calling key is needed - it can be called by anyone, and is a once only operation for PDA vault creation. Subsequent runs will fail.

Any problems in this process can be triaged in the `migrations/deploy.js` file, which is what `anchor migrate` executes.
