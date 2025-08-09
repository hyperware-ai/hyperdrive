// API utility functions for communicating with hyperwallet backend

export interface ApiResponse<T = any> {
  success: boolean
  data?: T
  error?: {
    code: string
    message: string
    details?: any
  }
  request_id?: string
  timestamp: number
}

/**
 * Call a hyperwallet API endpoint
 * Following Hyperware patterns for HTTP endpoints
 */
export async function callApi(endpoint: string, data: any): Promise<any> {
  try {
    const response = await fetch(`/${endpoint}`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify(data)
    })

    if (!response.ok) {
      throw new Error(`HTTP ${response.status}: ${response.statusText}`)
    }

    const result = await response.json()
    
    // If this is an operation response, check for errors
    if (result.success !== undefined && !result.success) {
      throw new Error(result.error?.message || 'Operation failed')
    }

    return result
  } catch (error) {
    console.error('API call failed:', error)
    throw error
  }
}

/**
 * Call a wallet operation through the hyperwallet service
 */
export async function callWalletOperation(
  operation: string,
  params: any,
  walletId?: string,
  chainId?: number
): Promise<ApiResponse> {
  const operationRequest = {
    operation,
    params,
    wallet_id: walletId,
    chain_id: chainId,
    timestamp: Math.floor(Date.now() / 1000)
  }

  return callApi('api', operationRequest)
}

/**
 * Get service status
 */
export async function getServiceStatus(): Promise<any> {
  return callApi('status', {})
}

/**
 * List accessible wallets
 */
export async function listWallets(): Promise<ApiResponse> {
  return callWalletOperation('ListWallets', {})
}

/**
 * Create a new wallet
 */
export async function createWallet(
  name: string,
  chainId: number,
  encrypt: boolean = false
): Promise<ApiResponse> {
  return callWalletOperation('CreateWallet', {
    name,
    chain_id: chainId,
    encrypt
  })
}

/**
 * Import an existing wallet
 */
export async function importWallet(
  name: string,
  privateKey: string,
  chainId: number,
  encrypt: boolean = false
): Promise<ApiResponse> {
  return callWalletOperation('ImportWallet', {
    name,
    private_key: privateKey,
    chain_id: chainId,
    encrypt
  })
}

/**
 * Get wallet information
 */
export async function getWalletInfo(walletId: string): Promise<ApiResponse> {
  return callWalletOperation('GetWalletInfo', {}, walletId)
}

/**
 * Get wallet balance
 */
export async function getBalance(walletId: string, chainId?: number): Promise<ApiResponse> {
  return callWalletOperation('GetBalance', {}, walletId, chainId)
}

/**
 * Get token balance
 */
export async function getTokenBalance(
  walletId: string,
  tokenAddress: string,
  chainId?: number
): Promise<ApiResponse> {
  return callWalletOperation('GetTokenBalance', {
    token_address: tokenAddress
  }, walletId, chainId)
}

/**
 * Send ETH
 */
export async function sendEth(
  walletId: string,
  to: string,
  amount: string,
  chainId?: number
): Promise<ApiResponse> {
  return callWalletOperation('SendEth', {
    to,
    amount
  }, walletId, chainId)
}

/**
 * Send tokens
 */
export async function sendToken(
  walletId: string,
  to: string,
  tokenAddress: string,
  amount: string,
  chainId?: number
): Promise<ApiResponse> {
  return callWalletOperation('SendToken', {
    to,
    token_address: tokenAddress,
    amount
  }, walletId, chainId)
}

/**
 * Resolve Hypermap identity
 */
export async function resolveIdentity(entryName: string, chainId?: number): Promise<ApiResponse> {
  return callWalletOperation('ResolveIdentity', {
    entry_name: entryName
  }, undefined, chainId)
}

/**
 * Create Hypermap note
 */
export async function createNote(
  entryName: string,
  noteKey: string,
  data: string,
  walletId: string,
  chainId?: number
): Promise<ApiResponse> {
  return callWalletOperation('CreateNote', {
    entry_name: entryName,
    note_key: noteKey,
    data
  }, walletId, chainId)
}

/**
 * Read Hypermap note
 */
export async function readNote(
  entryName: string,
  noteKey: string,
  chainId?: number
): Promise<ApiResponse> {
  return callWalletOperation('ReadNote', {
    entry_name: entryName,
    note_key: noteKey
  }, undefined, chainId)
}

/**
 * Execute via TBA
 */
export async function executeViaTba(
  walletId: string,
  tbaAddress: string,
  targetAddress: string,
  callData: string,
  value?: string,
  chainId?: number
): Promise<ApiResponse> {
  return callWalletOperation('ExecuteViaTba', {
    tba_address: tbaAddress,
    target_address: targetAddress,
    call_data: callData,
    value
  }, walletId, chainId)
}