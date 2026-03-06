import React, { useEffect, useMemo, useRef, useState } from 'react';
import { Chat } from '#caller-utils';
import { useConnectModal, useAddRecentTransaction } from '@rainbow-me/rainbowkit';
import { useAccount, useSendTransaction, useSwitchChain, useWriteContract } from 'wagmi';
import { erc20Abi, parseEther, parseUnits, type Address as EvmAddress } from 'viem';
import { useChatStore } from '../../store/chat';
import { getChatDisplayName } from '../../utils/chatDisplay';
import './SendCryptoModal.css';

const BASE_CHAIN_ID = 8453;

type SendToken = {
  symbol: string;
  label: string;
  kind: 'native' | 'erc20';
  decimals: number;
  address?: EvmAddress;
};

const BASE_SEND_TOKENS: readonly SendToken[] = [
  { symbol: 'ETH', label: 'Base ETH', kind: 'native', decimals: 18 },
  {
    symbol: 'USDC',
    label: 'USDC',
    kind: 'erc20',
    decimals: 6,
    address: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
  },
  {
    symbol: 'DAI',
    label: 'DAI',
    kind: 'erc20',
    decimals: 18,
    address: '0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb',
  },
  {
    symbol: 'HYPR',
    label: 'HYPR',
    kind: 'erc20',
    decimals: 18,
    address: '0x0000000000f6a4A20f4d340A1c3F3040aC5A255c',
  },
];

interface SendCryptoModalProps {
  chat: Chat.Chat;
  recipientAddress: string;
  onClose: () => void;
}

const truncateAddress = (address: string): string =>
  `${address.slice(0, 6)}...${address.slice(-4)}`;

const SendCryptoModal: React.FC<SendCryptoModalProps> = ({ chat, recipientAddress, onClose }) => {
  const { recordPayment } = useChatStore();
  const { openConnectModal } = useConnectModal();
  const { address, isConnected, chainId } = useAccount();
  const { switchChainAsync, isPending: isSwitching } = useSwitchChain();
  const { sendTransactionAsync, isPending: isSendingNativeTx } = useSendTransaction();
  const { writeContractAsync, isPending: isSendingTokenTx } = useWriteContract();
  const addRecentTransaction = useAddRecentTransaction();

  const [amount, setAmount] = useState('');
  const [selectedToken, setSelectedToken] = useState(BASE_SEND_TOKENS[0].symbol);
  const [error, setError] = useState<string | null>(null);
  const [isSavingEvent, setIsSavingEvent] = useState(false);
  const mountedRef = useRef(true);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const canSwitchToBase = isConnected && chainId !== BASE_CHAIN_ID;
  const tokenConfig = useMemo(
    () => BASE_SEND_TOKENS.find((token) => token.symbol === selectedToken) ?? BASE_SEND_TOKENS[0],
    [selectedToken],
  );
  const parsedAmount = useMemo(() => {
    const numeric = Number(amount);
    if (!amount || Number.isNaN(numeric) || numeric <= 0) return null;
    return numeric;
  }, [amount]);

  const isBusy = isSwitching || isSendingNativeTx || isSendingTokenTx || isSavingEvent;

  const handleSwitchChain = async () => {
    setError(null);
    try {
      await switchChainAsync({ chainId: BASE_CHAIN_ID });
    } catch (err) {
      if (mountedRef.current) {
        setError('Unable to switch to Base. Please switch in your wallet.');
      }
    }
  };

  const handleSend = async () => {
    if (!parsedAmount) {
      setError('Enter a valid amount.');
      return;
    }
    if (!isConnected || !address) {
      setError('Connect your wallet first.');
      return;
    }
    if (chainId !== BASE_CHAIN_ID) {
      setError('Switch to Base before sending.');
      return;
    }
    if (tokenConfig.kind === 'erc20' && !tokenConfig.address) {
      setError(`Token ${tokenConfig.symbol} is not configured for transfer.`);
      return;
    }

    setError(null);

    try {
      const txHash =
        tokenConfig.kind === 'native'
          ? await sendTransactionAsync({
              to: recipientAddress as EvmAddress,
              value: parseEther(amount),
              chainId: BASE_CHAIN_ID,
            })
          : await writeContractAsync({
              address: tokenConfig.address as EvmAddress,
              abi: erc20Abi,
              functionName: 'transfer',
              args: [recipientAddress as EvmAddress, parseUnits(amount, tokenConfig.decimals)],
              chainId: BASE_CHAIN_ID,
            });

      addRecentTransaction({
        hash: txHash,
        description: `Sent ${amount} ${tokenConfig.symbol}`,
      });

      if (mountedRef.current) {
        setIsSavingEvent(true);
      }
      await recordPayment({
        chat_id: chat.id,
        tx_hash: txHash,
        amount,
        coin_name: tokenConfig.symbol,
        from_address: address,
        to_address: recipientAddress,
      });
      onClose();
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : 'Failed to send transaction.');
      }
    } finally {
      if (mountedRef.current) {
        setIsSavingEvent(false);
      }
    }
  };

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="send-crypto-modal" onClick={(e) => e.stopPropagation()}>
        <div className="settings-header">
          <h3>Send Crypto</h3>
          <button className="close-button" onClick={onClose}>
            ×
          </button>
        </div>

        <div className="send-crypto-body">
          <div className="send-crypto-row">
            <span>Recipient</span>
            <strong>{getChatDisplayName(chat)}</strong>
          </div>
          <div className="send-crypto-address" title={recipientAddress}>
            {truncateAddress(recipientAddress)}
          </div>

          <label className="send-crypto-label" htmlFor="send-crypto-token">
            Coin
          </label>
          <select
            id="send-crypto-token"
            className="send-crypto-select"
            value={tokenConfig.symbol}
            onChange={(e) => setSelectedToken(e.target.value)}
            disabled={isBusy}
          >
            {BASE_SEND_TOKENS.map((token) => (
              <option key={token.symbol} value={token.symbol}>
                {token.label}
              </option>
            ))}
          </select>

          <label className="send-crypto-label" htmlFor="send-crypto-amount">
            Amount ({tokenConfig.symbol})
          </label>
          <input
            id="send-crypto-amount"
            className="send-crypto-input"
            type="number"
            inputMode="decimal"
            min="0"
            step="any"
            placeholder="0.00"
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
            disabled={isBusy}
          />

          {error && <div className="send-crypto-error">{error}</div>}
        </div>

        <div className="send-crypto-actions">
          {!isConnected ? (
            <button className="send-crypto-button" onClick={() => openConnectModal?.()}>
              Connect Wallet
            </button>
          ) : canSwitchToBase ? (
            <button className="send-crypto-button" onClick={handleSwitchChain} disabled={isBusy}>
              {isSwitching ? 'Switching...' : 'Switch To Base'}
            </button>
          ) : (
            <button
              className="send-crypto-button"
              onClick={handleSend}
              disabled={!parsedAmount || isBusy}
            >
              {isSendingNativeTx || isSendingTokenTx
                ? 'Confirm In Wallet...'
                : isSavingEvent
                  ? 'Saving Event...'
                  : `Send ${parsedAmount ?? ''} ${tokenConfig.symbol}`}
            </button>
          )}
        </div>
      </div>
    </div>
  );
};

export default SendCryptoModal;
