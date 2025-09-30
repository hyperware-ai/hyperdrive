import { useState, useEffect, FormEvent, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import UpgradableCheckbox from "../components/UpgradableCheckbox";
import { useAccount, useWaitForTransactionReceipt, useSendTransaction, useConfig } from "wagmi";
import { readContract } from "wagmi/actions";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit";
import { tbaMintAbi, HYPER_ACCOUNT_IMPL, HYPER_ACCOUNT_UPGRADABLE_IMPL, HYPERMAP, mechAbi, hypermapAbi } from "../abis";
import { generateNetworkingKeys } from "../abis/helpers";
import { encodePacked, encodeFunctionData, stringToHex } from "viem";
import BackButton from "../components/BackButton";
import { predictTBAAddress } from "../utils/predictTBA";
import { hyperhash } from "../utils/hyperhash";

interface MintCustomNameProps extends PageProps { }

function MintCustom({
    upgradable,
    setUpgradable,
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
    const { address } = useAccount();
    const navigate = useNavigate();
    const { openConnectModal } = useConnectModal();
    const config = useConfig();
    const [validationError, setValidationError] = useState<string>("");

    const { data: hash, sendTransaction, isPending, isError, error } = useSendTransaction({
        mutation: {
            onSuccess: (data) => {
                addRecentTransaction({ hash: data, description: `Mint ${hnsName}` });
            },
        },
    });

    const { isLoading: isConfirming, isSuccess: isConfirmed } = useWaitForTransactionReceipt({
        hash,
    });
    const addRecentTransaction = useAddRecentTransaction();

    const [triggerNameCheck, setTriggerNameCheck] = useState<boolean>(false);

    useEffect(() => {
        document.title = "Mint";
    }, []);

    useEffect(() => setTriggerNameCheck(!triggerNameCheck), [address]);

    useEffect(() => {
        if (!address) {
            openConnectModal?.();
        }
    }, [address, openConnectModal]);

    useEffect(() => {
        if (isConfirmed) {
            navigate("/set-password");
        }
    }, [isConfirmed, address, navigate]);

    const handleMint = useCallback(
        async (e: FormEvent) => {
            e.preventDefault();
            e.stopPropagation();

            const formData = new FormData(e.target as HTMLFormElement);

            if (!address) {
                openConnectModal?.();
                return;
            }

            const tbaAddr = (formData.get("tba") as `0x${string}`) || HYPERMAP;
            const fullHnsName = formData.get("full-hns-name") as string;

            if (!fullHnsName || !fullHnsName.includes(".")) {
                setValidationError("Full HNS name must contain a dot, e.g., foo.bar");
                return;
            }

            // Derive name from the first part before the dot
            const name = fullHnsName.split(".")[0];
            const rootName = fullHnsName.replace(`${name}.`, "");
            try {
                const tokenData = (await readContract(config, {
                    address: tbaAddr,
                    abi: mechAbi,
                    functionName: "token",
                })) as readonly [bigint, `0x${string}`, bigint];
                const tokenId = tokenData[2];
                const rootNameHash = hyperhash(rootName);
                if (tokenId !== BigInt(rootNameHash)) {
                    setValidationError(`The name '${rootName}' is not associated with the provided TBA address`);
                    return;
                }
                // Predict the TBA address that will be created
                const predictedTBA = predictTBAAddress(HYPERMAP, fullHnsName);
                console.log("predictedTBA", predictedTBA);

                const initCall = await generateNetworkingKeys({
                    upgradable,
                    direct,
                    our_address: address,
                    label: hnsName,
                    setNetworkingKey,
                    setIpAddress,
                    setWsPort,
                    setTcpPort,
                    setRouters,
                    reset: false,
                    tbaAddress: predictedTBA.predictedAddress,
                });

                setHnsName(fullHnsName);

                const impl = upgradable ? HYPER_ACCOUNT_UPGRADABLE_IMPL : HYPER_ACCOUNT_IMPL;
                const data = encodeFunctionData({
                    abi: tbaMintAbi,
                    functionName: "mint",
                    args: [
                        address,
                        encodePacked(["bytes"], [stringToHex(name)]),
                        initCall,
                        impl,
                    ],
                });

                // Send the transaction
                sendTransaction({
                    to: tbaAddr,
                    data: data,
                    gas: 1000000n,
                });
            } catch (err) {
                console.error("Failed to read contract or send transaction:", err);
                setValidationError("Internal error, check console for details");
            }
        },
        [
            config,
            upgradable,
            direct,
            address,
            sendTransaction,
            setNetworkingKey,
            setIpAddress,
            setWsPort,
            setTcpPort,
            setRouters,
            openConnectModal,
            hnsName,
        ]
    );

    return (
        <div className="container fade-in">
            <div className="section">
                <form className="form" onSubmit={handleMint}>
                    {isPending || isConfirming ? (
                        <Loader msg={isConfirming ? "Minting name..." : "Please confirm the transaction in your wallet"} />
                    ) : (
                        <>
                            <p className="form-label">
                                <span>
                                    Register a name on a different top-level zone — this may fail if that zone's requirements are not met
                                </span>
                            </p>
                            <input type="text" name="full-hns-name" placeholder="Enter full HNS name (e.g. foo.bar)" />
                            <input type="text" name="tba" placeholder="Enter TBA to mint under (e.g. related .bar TBA)" />
                            <details>
                                <summary>Advanced Options</summary>
                                <DirectNodeCheckbox {...{ direct, setDirect }} />
                                <UpgradableCheckbox {...{ upgradable, setUpgradable }} />
                            </details>
                            <div className="flex flex-col gap-1">
                                <button
                                    type="submit"
                                    className="button"
                                    disabled={isPending || isConfirming}
                                >
                                    Mint custom name
                                </button>
                                <BackButton mode="wide" />
                            </div>
                        </>
                    )}
                    {validationError && (
                        <p className="text-red-500 font-semibold mt-2">
                            {validationError}
                        </p>
                    )}
                    {isError && (
                        <p className="text-red-500 wrap-anywhere mt-2">
                            Error: {error?.message || "There was an error minting your name, please try again."}
                        </p>
                    )}
                </form>
            </div>
        </div>
    );
}

export default MintCustom;