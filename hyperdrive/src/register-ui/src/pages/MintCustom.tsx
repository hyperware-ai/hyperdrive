import { useState, useEffect, FormEvent, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";

import DirectNodeCheckbox from "../components/DirectCheckbox";
import SpecifyRoutersCheckbox from "../components/SpecifyRoutersCheckbox";

import { useAccount, useWaitForTransactionReceipt, useSendTransaction } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit"
import { tbaMintAbi, generateNetworkingKeys, HYPER_ACCOUNT_IMPL } from "../abis";
import { encodePacked, encodeFunctionData, stringToHex } from "viem";
import BackButton from "../components/BackButton";
interface MintCustomNameProps extends PageProps { }

function MintCustom({
    direct,
    setDirect,
    hnsName,
    setHnsName,
    setNetworkingKey,
    setIpAddress,
    setWsPort,
    setTcpPort,
    setRouters,
}: MintCustomNameProps) {
    let { address } = useAccount();
    let navigate = useNavigate();
    let { openConnectModal } = useConnectModal();

    const { data: hash, sendTransaction, isPending, isError, error } = useSendTransaction({
        mutation: {
            onSuccess: (data) => {
                addRecentTransaction({ hash: data, description: `Mint ${hnsName}` });
            }
        }
    });
    const { isLoading: isConfirming, isSuccess: isConfirmed } =
        useWaitForTransactionReceipt({
            hash,
        });
    const addRecentTransaction = useAddRecentTransaction();

    const [triggerNameCheck, setTriggerNameCheck] = useState<boolean>(false)
    const [specifyRouters, setSpecifyRouters] = useState(false)
    const [customRouters, setCustomRouters] = useState('')

    // Modified setDirect function to handle mutual exclusivity
    const handleSetDirect = (value: boolean) => {
        setDirect(value);
        if (value) {
            setSpecifyRouters(false);
            setCustomRouters(''); // Clear custom routers when switching to direct
        }
    };

    // Modified setSpecifyRouters function to handle mutual exclusivity
    const handleSetSpecifyRouters = (value: boolean) => {
        setSpecifyRouters(value);
        if (value) {
            setDirect(false);
        } else {
            setCustomRouters(''); // Clear custom routers when unchecking
        }
    };

    useEffect(() => {
        document.title = "Mint"
    }, [])

    useEffect(() => setTriggerNameCheck(!triggerNameCheck), [address])

    useEffect(() => {
        if (!address) {
            openConnectModal?.();
        }
    }, [address, openConnectModal]);

    let handleMint = useCallback(async (e: FormEvent) => {
        e.preventDefault()
        e.stopPropagation()

        const formData = new FormData(e.target as HTMLFormElement)

        if (!address) {
            openConnectModal?.()
            return
        }

        // Process custom routers if specified
        let routersToUse: string[] = [];
        if (specifyRouters && customRouters.trim()) {
            routersToUse = customRouters
                .split('\n')
                .map(router => router.trim())
                .filter(router => router.length > 0);

            // Update the routers in your app state
            setRouters(routersToUse);
            console.log("Custom routers:", routersToUse);
        }

        const initCall = await generateNetworkingKeys({
            direct,
            our_address: address,
            label: hnsName,
            setNetworkingKey,
            setIpAddress,
            setWsPort,
            setTcpPort,
            setRouters: routersToUse.length > 0 ? () => setRouters(routersToUse) : setRouters,
            reset: false,
            customRouters: routersToUse.length > 0 ? routersToUse : undefined,
        });

        setHnsName(formData.get('full-hns-name') as string)

        const name = formData.get('name') as string

        console.log("full hns name", formData.get('full-hns-name'))
        console.log("name", name)

        const data = encodeFunctionData({
            abi: tbaMintAbi,
            functionName: 'mint',
            args: [
                address,
                encodePacked(["bytes"], [stringToHex(name)]),
                initCall,
                HYPER_ACCOUNT_IMPL,
            ],
        })

        // use data to write to contract -- do NOT use writeContract
        // writeContract will NOT generate the correct selector for some reason
        // probably THEIR bug.. no abi works
        try {
            sendTransaction({
                to: formData.get('tba') as `0x${string}`,
                data: data,
                gas: 1000000n,
            })
        } catch (error) {
            console.error('Failed to send transaction:', error)
        }
    }, [direct, specifyRouters, customRouters, address, sendTransaction, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, openConnectModal])

    useEffect(() => {
        if (isConfirmed) {
            navigate("/set-password");
        }
    }, [isConfirmed, address, navigate]);

    return (
        <div className="container fade-in">
            <div className="section">
                {
                    <form className="form" onSubmit={handleMint}>
                        {isPending || isConfirming ? (
                            <Loader msg={isConfirming ? 'Minting name...' : 'Please confirm the transaction in your wallet'} />
                        ) : (
                            <>
                                <p className="form-label">
                                    <span>
                                        Register a name on a different top-level zone -- this will likely fail if that zone's requirements are not met
                                    </span>
                                </p>
                                <input type="text" name="name" placeholder="Enter hypermap name" />
                                <input type="text" name="full-hns-name" placeholder="Enter full HNS name" />
                                <input type="text" name="tba" placeholder="Enter TBA to mint under" />
                                <details className="advanced-options">
                                    <summary>Advanced Options</summary>
                                    <div className="flex flex-col gap-3">
                                        <DirectNodeCheckbox direct={direct} setDirect={handleSetDirect} />
                                        <SpecifyRoutersCheckbox specifyRouters={specifyRouters} setSpecifyRouters={handleSetSpecifyRouters} />
                                        {specifyRouters && (
                                            <div className="flex flex-col gap-2 ml-6">
                                                <label htmlFor="custom-routers" className="text-sm font-medium">
                                                    Router Names: <span className="text-red-500">*</span>
                                                </label>
                                                <textarea
                                                    id="custom-routers-mint"
                                                    value={customRouters}
                                                    onChange={(e) => setCustomRouters(e.target.value)}
                                                    placeholder="Enter router names, one per line&#10;e.g.:&#10;router1.os&#10;router2.os&#10;myrouter.os"
                                                    className={`input resize-vertical min-h-[80px] ${
                                                        specifyRouters && customRouters.split('\n').map(r => r.trim()).filter(r => r.length > 0).length === 0
                                                            ? 'border-red-500 focus:border-red-500'
                                                            : ''
                                                    }`}
                                                    rows={4}
                                                />
                                                <span className={`text-xs ${
                                                    customRouters.split('\n').map(r => r.trim()).filter(r => r.length > 0).length === 0
                                                        ? 'text-red-500'
                                                        : 'text-gray-500'
                                                }`}>
                                                    {customRouters.split('\n').map(r => r.trim()).filter(r => r.length > 0).length === 0
                                                        ? 'At least one router name is required'
                                                        : 'Enter one router name per line. These routers will be used for your indirect node.'
                                                    }
                                                </span>
                                            </div>
                                        )}
                                    </div>
                                </details>
                                <div className="flex flex-col gap-1">
                                    <button
                                        type="submit"
                                        className="button"
                                        disabled={
                                            isPending ||
                                            isConfirming ||
                                            !hnsName ||
                                            (specifyRouters && customRouters.split('\n').map(r => r.trim()).filter(r => r.length > 0).length === 0)
                                        }>
                                        Mint custom name
                                    </button>

                                    <BackButton mode="wide" />
                                </div>
                            </>
                        )}
                        {isError && (
                            <p className="text-red-500 wrap-anywhere mt-2">
                                Error: {error?.message || 'There was an error minting your name, please try again.'}
                            </p>
                        )}
                    </form>
                }
            </div>
        </div>
    );
}

export default MintCustom;
