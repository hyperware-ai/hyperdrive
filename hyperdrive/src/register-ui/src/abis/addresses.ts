// Contract addresses with proper checksums
// You may need to update these with the actual deployed addresses for your network

export const HYPERMAP_ADDRESS = "0x000000000044C6B8Cb4d8f0F889a3E47664EAeda";
export const MULTICALL_ADDRESS = "0xcA11bde05977b3631167028862bE2a173976CA11";
export const HYPER_ACCOUNT_IMPL_ADDRESS = "0x0000000000EDAd72076CBe7b9Cfa3751D5a85C97";
export const DOTOS_ADDRESS = "0x763Ae1AB24c4322b8933E58d76d8D9286f6C0162";

// Get the correct HYPERMAP address based on chain ID
export function getHypermapAddress(chainId: number): `0x${string}` {
  // Remove leading zeros from the address for compatibility
  // Original: 0x000000000044C6B8Cb4d8f0F889a3E47664EAeda
  // This is the same address but without leading zeros
  const cleanAddress = "0x44C6B8Cb4d8f0F889a3E47664EAeda";
  
  // You should update this with the actual deployed addresses for each chain
  switch (chainId) {
    case 1: // Ethereum Mainnet
    case 11155111: // Sepolia
    case 84532: // Base Sepolia
    case 8453: // Base Mainnet
    default:
      // Return the address without leading zeros
      return cleanAddress as `0x${string}`;
  }
}

// Normalize address to ensure it's properly formatted
export function normalizeAddress(address: string): `0x${string}` {
  // Remove 0x prefix if present
  let cleanAddress = address.toLowerCase().replace('0x', '');
  
  // Pad with zeros if needed (shouldn't be necessary for valid addresses)
  cleanAddress = cleanAddress.padStart(40, '0');
  
  // Return with 0x prefix
  return `0x${cleanAddress}` as `0x${string}`;
}