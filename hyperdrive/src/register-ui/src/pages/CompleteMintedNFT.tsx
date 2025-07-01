import { useState, useEffect, FormEvent, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import { useAccount, useWaitForTransactionReceipt, useSendTransaction } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit";
import { generateNetworkingKeys, MULTICALL } from "../abis";
import BackButton from "../components/BackButton";

interface CompleteMintedNFTProps extends PageProps { }

function CompleteMintedNFT({
    direct,
    setDirect,
    hnsName,
    setHnsName,
    setNetworkingKey,
    setIpAddress,
    setWsPort,
    setTcpPort,
    setRouters,
}: CompleteMintedNFTProps) {
    let { address } = useAccount();
    let navigate = useNavigate();
    let { openConnectModal } = useConnectModal();

    const { data: hash, sendTransaction, isPending, isError, error } = useSendTransaction({
        mutation: {
            onSuccess: (data) => {
                addRecentTransaction({ hash: data, description: `Complete setup for ${hnsName}` });
            }
        }
    });
    const { isLoading: isConfirming, isSuccess: isConfirmed } =
        useWaitForTransactionReceipt({
            hash,
        });
    const addRecentTransaction = useAddRecentTransaction();

    useEffect(() => {
        document.title = "Complete Minted NFT"
    }, [])

    useEffect(() => {
        if (!address) {
            openConnectModal?.();
        }
    }, [address, openConnectModal]);

    let handleComplete = useCallback(async (e: FormEvent) => {
        e.preventDefault()
        e.stopPropagation()

        const formData = new FormData(e.target as HTMLFormElement)

        if (!address) {
            openConnectModal?.()
            return
        }

        const fullHnsName = formData.get('full-hns-name') as string
        const tbaAddress = formData.get('tba') as `0x${string}`

        setHnsName(fullHnsName)

        // Generate networking keys and create multicall data
        const multicalls = await generateNetworkingKeys({
            direct,
            our_address: address,
            label: fullHnsName,
            setNetworkingKey,
            setIpAddress,
            setWsPort,
            setTcpPort,
            setRouters,
            reset: true, // Use reset=true to get the raw multicall data
        });

        // Send the multicall transaction directly from the TBA
        try {
            sendTransaction({
                to: MULTICALL,
                from: tbaAddress,
                data: multicalls,
                gas: 500000n,
            })
        } catch (error) {
            console.error('Failed to send transaction:', error)
        }
    }, [direct, address, sendTransaction, setHnsName, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, openConnectModal])

    useEffect(() => {
        if (isConfirmed) {
            navigate("/set-password");
        }
    }, [isConfirmed, navigate]);

    return (
        <div className="container fade-in">
            <div className="section">
                <form className="form" onSubmit={handleComplete}>
                    {isPending || isConfirming ? (
                        <Loader msg={isConfirming ? 'Completing setup...' : 'Please confirm the transaction in your wallet'} />
                    ) : (
                        <>
                            <p className="form-label">
                                <BackButton />
                                <span>
                                    Complete the setup for an already-minted NFT by storing its networking information onchain
                                </span>
                            </p>
                            <input
                                type="text"
                                name="full-hns-name"
                                placeholder="Enter full HNS name (e.g., myname.os)"
                                required
                            />
                            <input
                                type="text"
                                name="tba"
                                placeholder="Enter TBA address of minted NFT"
                                required
                                pattern="^0x[a-fA-F0-9]{40}$"
                            />
                            <details>
                                <summary>Advanced Options</summary>
                                <DirectNodeCheckbox {...{ direct, setDirect }} />
                            </details>
                            <div className="button-group">
                                <button type="submit" className="button">
                                    Complete NFT Setup
                                </button>
                            </div>
                        </>
                    )}
                    {isError && (
                        <p className="error-message">
                            Error: {error?.message || 'There was an error completing the setup, please try again.'}
                        </p>
                    )}
                </form>
            </div>
        </div>
    );
}

export default CompleteMintedNFT;
