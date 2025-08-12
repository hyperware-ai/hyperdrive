import { Address } from 'viem';
import { PublicClient } from 'wagmi';

export type WalletType = 'EOA' | 'SAFE' | 'UNKNOWN_CONTRACT';

/**
 * Detects the type of wallet connected
 * @param address - The wallet address to check
 * @param client - The public client from wagmi
 * @returns The type of wallet
 */
export async function getWalletType(
  address: Address,
  client: PublicClient
): Promise<WalletType> {
  try {
    // Get the bytecode at the address
    const code = await client.getBytecode({ address });

    // If no code, it's an EOA
    if (!code || code === '0x') {
      return 'EOA';
    }

    // Check if it's a Safe wallet
    if (await isSafeWallet(address, client)) {
      return 'SAFE';
    }

    // Otherwise it's some other smart contract wallet
    return 'UNKNOWN_CONTRACT';
  } catch (error) {
    console.error('Error detecting wallet type:', error);
    // Default to EOA on error
    return 'EOA';
  }
}

/**
 * Checks if the address is a Gnosis Safe wallet
 * @param address - The address to check
 * @param client - The public client from wagmi
 * @returns True if it's a Safe wallet
 */
async function isSafeWallet(
  address: Address,
  client: PublicClient
): Promise<boolean> {
  try {
    // Safe wallets implement the VERSION method
    // Try to call VERSION() on the contract
    const versionAbi = [{
      name: 'VERSION',
      type: 'function',
      stateMutability: 'view',
      inputs: [],
      outputs: [{ type: 'string' }],
    }] as const;

    const version = await client.readContract({
      address,
      abi: versionAbi,
      functionName: 'VERSION',
    });

    // Safe wallets return version strings like "1.3.0", "1.4.1" etc
    return typeof version === 'string' && version.includes('.');
  } catch {
    // If VERSION call fails, check for other Safe-specific methods
    try {
      // Try calling getOwners() which is specific to Safe
      const getOwnersAbi = [{
        name: 'getOwners',
        type: 'function',
        stateMutability: 'view',
        inputs: [],
        outputs: [{ type: 'address[]' }],
      }] as const;

      await client.readContract({
        address,
        abi: getOwnersAbi,
        functionName: 'getOwners',
      });

      return true;
    } catch {
      return false;
    }
  }
}

/**
 * Helper to check if an address is a contract
 * @param address - The address to check
 * @param client - The public client from wagmi
 * @returns True if the address has contract code
 */
export async function isContract(
  address: Address,
  client: PublicClient
): Promise<boolean> {
  try {
    const code = await client.getBytecode({ address });
    return code !== undefined && code !== '0x';
  } catch {
    return false;
  }
}
