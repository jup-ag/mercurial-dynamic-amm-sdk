import { Wallet } from '@project-serum/anchor';
import { bs58 } from '@project-serum/anchor/dist/cjs/utils/bytes';
import { TOKEN_PROGRAM_ID, Token } from '@solana/spl-token';
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey } from '@solana/web3.js';

export const airDropSol = async (connection: Connection, publicKey: PublicKey, amount = 1) => {
  try {
    const airdropSignature = await connection.requestAirdrop(publicKey, amount * LAMPORTS_PER_SOL);
    const latestBlockHash = await connection.getLatestBlockhash();
    await connection.confirmTransaction(
      {
        blockhash: latestBlockHash.blockhash,
        lastValidBlockHeight: latestBlockHash.lastValidBlockHeight,
        signature: airdropSignature,
      },
      connection.commitment,
    );
  } catch (error) {
    console.error(error);
    throw error;
  }
};

export const airDropSolIfBalanceNotEnough = async (connection: Connection, publicKey: PublicKey, balance = 1) => {
  const walletBalance = await connection.getBalance(publicKey);
  if (walletBalance < balance * LAMPORTS_PER_SOL) {
    await airDropSol(connection, publicKey);
  }
};

export const getOrCreateATA = async (connection: Connection, mint: PublicKey, owner: PublicKey, payer: Keypair) => {
  const token = new Token(connection, mint, TOKEN_PROGRAM_ID, payer);
  const ata = await token.getOrCreateAssociatedAccountInfo(owner);

  return ata.address;
};

export const mockWallet = new Wallet(Keypair.generate());

// export const MAINNET = {
//   connection: new Connection(process.env.MAINNET_RPC_ENDPOINT as string),
//   cluster: 'mainnet-beta',
// };

// export const DEVNET = {
//   connection: new Connection('https://api.devnet.solana.com/', {
//     commitment: 'confirmed',
//   }),
//   cluster: 'devnet',
// };
