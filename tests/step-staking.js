const anchor = require('@project-serum/anchor');
const { TOKEN_PROGRAM_ID, Token } = require('@solana/spl-token');
const utils = require('./utils');
const assert = require('assert');
const fs = require('fs');

let program = anchor.workspace.StepStaking;

//Read the provider from the configured environmnet.
//represents an outside actor
//owns mints out of any other actors control, provides initial $$ to others
const envProvider = anchor.Provider.env();

//we allow this convenience var to change between default env and mock user(s)
//initially we are the outside actor
let provider = envProvider;
//convenience method to set in anchor AND above convenience var
//setting in anchor allows the rpc and accounts namespaces access
//to a different wallet from env
function setProvider(p) {
  provider = p;
  anchor.setProvider(p);
  program = new anchor.Program(program.idl, program.programId, p);
}
setProvider(provider);

describe('step-staking', () => {
  //hardcoded in program, read from test keys directory for testing
  let mintKey;
  let mintObject;
  let mintPubkey;

  //the program's vault for stored collateral against xToken minting
  let vaultPubkey;
  let vaultBump;

  //the program's account for stored initializer key and lock end date
  let stakingPubkey;
  let stakingBump;
  let lockEndDate = new anchor.BN(Date.now() / 1000 + 1000);
  let newLockEndDate = new anchor.BN(Date.now() / 1000);

  const admin = provider.wallet.publicKey;
  const staker1 = new anchor.Wallet(anchor.web3.Keypair.generate());

  //the user's staking account for stored deposit amount
  let userStakingPubkey;
  let userStakingBump;
  let walletTokenAccount;

  let userStakingBump2;
  let userBStakingPub2;
  let walletTokenAccount2;

  it('Is initialized!', async () => {
    //setup logging event listeners
    program.addEventListener('PriceChange', (e, s) => {
      console.log('Price Change In Slot ', s);
      console.log('From', e.oldStepPerXstepE9.toString());
      console.log('From', e.oldStepPerXstep.toString());
      console.log('To', e.newStepPerXstepE9.toString());
      console.log('To', e.newStepPerXstep.toString());
    });

    //this already exists in ecosystem
    //test step token hardcoded in program, mint authority is wallet for testing
    let rawdata = fs.readFileSync(
      'tests/keys/step-teST1ieLrLdr4MJPZ7i8mgSCLQ7rTrPRjNnyFdHFaz9.json'
    );
    let keyData = JSON.parse(rawdata);
    mintKey = anchor.web3.Keypair.fromSecretKey(new Uint8Array(keyData));
    mintObject = await utils.createMint(
      mintKey,
      provider,
      admin,
      null,
      9,
      TOKEN_PROGRAM_ID
    );
    mintPubkey = mintObject.publicKey;

    [vaultPubkey, vaultBump] = await anchor.web3.PublicKey.findProgramAddress(
      [mintPubkey.toBuffer()],
      program.programId
    );

    [stakingPubkey, stakingBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from(anchor.utils.bytes.utf8.encode('staking'))],
        program.programId
      );

    await program.rpc.initialize(vaultBump, stakingBump, lockEndDate, {
      accounts: {
        tokenMint: mintPubkey,
        tokenVault: vaultPubkey,
        stakingAccount: stakingPubkey,
        initializer: admin,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      },
    });
  });



  it('Mint test tokens', async () => {
    walletTokenAccount = await mintObject.createAssociatedTokenAccount(
      admin
    );
    await utils.mintToAccount(
      provider,
      mintPubkey,
      walletTokenAccount,
      95_000_000_000
    );

    walletTokenAccount2 = await mintObject.createAssociatedTokenAccount(
      staker1.publicKey
    );
    await utils.mintToAccount(
      provider,
      mintPubkey,
      walletTokenAccount2,
      100_000_000_000
    );

    await provider.connection.requestAirdrop(staker1.payer.publicKey, 10e9)
  });

  it('Swap token for xToken, admin', async () => {
    [userStakingPubkey, userStakingBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [admin.toBuffer()],
        program.programId
      );

    await program.rpc.stake(
      vaultBump,
      stakingBump,
      userStakingBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          tokenFrom: walletTokenAccount,
          tokenFromAuthority: admin,
          tokenVault: vaultPubkey,
          stakingAccount: stakingPubkey,
          userStakingAccount: userStakingPubkey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
      }
    );

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey
    );
    let amount = new anchor.BN(5_000_000_000);

    assert.strictEqual(parseInt(userStakingAccount.amount), amount.toNumber());
    assert.strictEqual(
      await getTokenBalance(walletTokenAccount),
      90_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), amount.toNumber());
    assert.strictEqual(
      parseInt(userStakingAccount.xTokenAmount),
      amount.toNumber()
    );
    assert.strictEqual(await getTokenBalance(vaultPubkey), 5_000_000_000);
  });

  it('Swap token for xToken, non-admin', async () => {
    [userStakingPubkey2, userStakingBump2] =
      await anchor.web3.PublicKey.findProgramAddress(
        [staker1.publicKey.toBuffer()],
        program.programId
      );
    
    const adminWallet = program.provider.wallet;
    program.provider.wallet = staker1;
    await program.rpc.stake(
      vaultBump,
      stakingBump,
      userStakingBump2,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          tokenFrom: walletTokenAccount2,
          tokenFromAuthority: staker1.payer.publicKey,
          tokenVault: vaultPubkey,
          stakingAccount: stakingPubkey,
          userStakingAccount: userStakingPubkey2,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        }
      }
    );
    program.provider.wallet = adminWallet;
    

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey2
    );
    let amount = new anchor.BN(5_000_000_000);

    assert.strictEqual(parseInt(userStakingAccount.amount), amount.toNumber());
    assert.strictEqual(
      await getTokenBalance(walletTokenAccount2),
      95_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), amount.toNumber());
    assert.strictEqual(
      parseInt(userStakingAccount.xTokenAmount),
      amount.toNumber()
    );
    assert.strictEqual(await getTokenBalance(vaultPubkey), 10_000_000_000);
  });

  it('Redeem xToken for token wrong auth', async () => {
    await assert.rejects(
      async () => {
        await program.rpc.unstake(
          vaultBump,
          stakingBump,
          userStakingBump2,
          new anchor.BN(5_000_000_000),
          {
            accounts: {
              tokenMint: mintPubkey,
              xTokenFromAuthority: admin,
              tokenVault: vaultPubkey,
              stakingAccount: stakingPubkey,
              userStakingAccount: userStakingPubkey2,
              tokenTo: walletTokenAccount,
              tokenProgram: TOKEN_PROGRAM_ID,
            },
          }
        );
      },
      { code: 146, msg: 'A seeds constraint was violated' }
    );
  });

  it('Redeem xToken for token for user by admin', async () => {
    await program.rpc.unstakeAdmin(
      vaultBump,
      stakingBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          admin: admin,
          tokenVault: vaultPubkey,
          stakingAccount: stakingPubkey,
          userStakingAccount: userStakingPubkey2,
          tokenTo: walletTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      }
    );

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey2
    );

    assert.strictEqual(
      await getTokenBalance(walletTokenAccount),
      95_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), 0);
    assert.strictEqual(parseInt(userStakingAccount.xTokenAmount), 0);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 5_000_000_000);
  });

  it('Airdrop some tokens to the pool', async () => {
    await utils.mintToAccount(provider, mintPubkey, vaultPubkey, 1_000_000_000);

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey
    );
    let amount = new anchor.BN(5_000_000_000);

    assert.strictEqual(
      await getTokenBalance(walletTokenAccount),
      95_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), amount.toNumber());
    assert.strictEqual(
      parseInt(userStakingAccount.xTokenAmount),
      amount.toNumber()
    );
    assert.strictEqual(await getTokenBalance(vaultPubkey), 6_000_000_000);
  });

  it('Emit the price', async () => {
    var res = await program.simulate.emitPrice({
      accounts: {
        tokenMint: mintPubkey,
        tokenVault: vaultPubkey,
        stakingAccount: stakingPubkey,
      },
    });
    let price = res.events[0].data;
    console.log('Emit price: ', price.stepPerXstepE9.toString());
    console.log('Emit price: ', price.stepPerXstep.toString());
    assert.strictEqual(price.stepPerXstep.toString(), '1.2');
  });

  it('Emit the reward', async () => {
    var res = await program.simulate.emitReward({
      accounts: {
        tokenMint: mintPubkey,
        tokenVault: vaultPubkey,
        stakingAccount: stakingPubkey,
        tokenFromAuthority: admin,
        userStakingAccount: userStakingPubkey,
      },
    });
    let reward = res.events[0].data;
    console.log('Deposit Amount: ', reward.deposit.toString());
    console.log('Reward Amount: ', reward.reward.toString());
    assert.strictEqual(parseInt(reward.deposit), 5_000_000_000);
    assert.strictEqual(parseInt(reward.reward), 1_000_000_000);
  });

  it('Redeem xToken for token before lock end date', async () => {
    await assert.rejects(
      async () => {
        await program.rpc.unstake(
          vaultBump,
          stakingBump,
          userStakingBump,
          new anchor.BN(5_000_000_000),
          {
            accounts: {
              tokenMint: mintPubkey,
              xTokenFromAuthority: admin,
              tokenVault: vaultPubkey,
              stakingAccount: stakingPubkey,
              userStakingAccount: userStakingPubkey,
              tokenTo: walletTokenAccount,
              tokenProgram: TOKEN_PROGRAM_ID,
            },
          }
        );
      },
      { code: 300, msg: 'Not exceed lock end date' }
    );
  });

  it('Update lock end date', async () => {
    await program.rpc.updateLockEndDate(stakingBump, newLockEndDate, {
      accounts: {
        initializer: admin,
        stakingAccount: stakingPubkey,
      },
    });

    let stakingAccount = await program.account.stakingAccount.fetch(
      stakingPubkey
    );
    assert.strictEqual(
      parseInt(stakingAccount.lockEndDate),
      newLockEndDate.toNumber()
    );
  });

  it('Redeem xToken for token', async () => {
    await program.rpc.unstakeAdmin(
      vaultBump,
      stakingBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          admin: admin,
          tokenVault: vaultPubkey,
          stakingAccount: stakingPubkey,
          userStakingAccount: userStakingPubkey,
          tokenTo: walletTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      }
    );

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey
    );

    assert.strictEqual(
      await getTokenBalance(walletTokenAccount),
      101_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), 0);
    assert.strictEqual(parseInt(userStakingAccount.xTokenAmount), 0);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 0);
  });

  it('Airdrop some tokens to the pool before xToken creation', async () => {
    await utils.mintToAccount(provider, mintPubkey, vaultPubkey, 5_000_000_000);

    assert.strictEqual(await getTokenBalance(vaultPubkey), 5_000_000_000);
  });

  it('Swap token for xToken on prefilled pool', async () => {
    await program.rpc.stake(
      vaultBump,
      stakingBump,
      userStakingBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          tokenFrom: walletTokenAccount,
          tokenFromAuthority: admin,
          tokenVault: vaultPubkey,
          stakingAccount: stakingPubkey,
          userStakingAccount: userStakingPubkey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        },
      }
    );

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey
    );
    let amount = new anchor.BN(5_000_000_000);

    assert.strictEqual(
      await getTokenBalance(walletTokenAccount),
      96_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), amount.toNumber());
    assert.strictEqual(
      parseInt(userStakingAccount.xTokenAmount),
      amount.toNumber()
    );
    assert.strictEqual(await getTokenBalance(vaultPubkey), 10_000_000_000);
  });

  it('Redeem xToken for token after prefilled pool', async () => {
    await program.rpc.unstake(
      vaultBump,
      stakingBump,
      userStakingBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          xTokenFromAuthority: admin,
          tokenVault: vaultPubkey,
          stakingAccount: stakingPubkey,
          userStakingAccount: userStakingPubkey,
          tokenTo: walletTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
      }
    );

    let userStakingAccount = await program.account.userStakingAccount.fetch(
      userStakingPubkey
    );

    assert.strictEqual(
      await getTokenBalance(walletTokenAccount),
      106_000_000_000
    );
    assert.strictEqual(parseInt(userStakingAccount.amount), 0);
    assert.strictEqual(parseInt(userStakingAccount.xTokenAmount), 0);
  });

  it('Freeze program for staking/unstaking', async () => {
    await program.rpc.toggleFreezeProgram(stakingBump, {
      accounts: {
        initializer: admin,
        stakingAccount: stakingPubkey,
      },
    });

    let stakingAccount = await program.account.stakingAccount.fetch(
      stakingPubkey
    );

    assert.strictEqual(stakingAccount.freezeProgram, true);

    await assert.rejects(
      async () => {
        await program.rpc.stake(
          vaultBump,
          stakingBump,
          userStakingBump,
          new anchor.BN(5_000_000_000),
          {
            accounts: {
              tokenMint: mintPubkey,
              tokenFrom: walletTokenAccount,
              tokenFromAuthority: admin,
              tokenVault: vaultPubkey,
              stakingAccount: stakingPubkey,
              userStakingAccount: userStakingPubkey,
              systemProgram: anchor.web3.SystemProgram.programId,
              tokenProgram: TOKEN_PROGRAM_ID,
              rent: anchor.web3.SYSVAR_RENT_PUBKEY,
            },
          }
        );
      },
      { code: 143 }
    );

    await assert.rejects(
      async () => {
        await program.rpc.unstake(
          vaultBump,
          stakingBump,
          userStakingBump,
          new anchor.BN(5_000_000_000),
          {
            accounts: {
              tokenMint: mintPubkey,
              xTokenFromAuthority: admin,
              tokenVault: vaultPubkey,
              stakingAccount: stakingPubkey,
              userStakingAccount: userStakingPubkey,
              tokenTo: walletTokenAccount,
              tokenProgram: TOKEN_PROGRAM_ID,
            },
          }
        );
      },
      { code: 143 }
    );
  });
});

async function getTokenBalance(pubkey) {
  return parseInt(
    (await provider.connection.getTokenAccountBalance(pubkey)).value.amount
  );
}
