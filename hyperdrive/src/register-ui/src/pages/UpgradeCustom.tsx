import { useState, useEffect, FormEvent, useCallback } from "react";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";
import { useAccount, useWaitForTransactionReceipt, useSendTransaction } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit";
import { tbaUpgradeAbi } from "../abis";
import { encodeFunctionData, stringToHex } from "viem";
import BackButton from "../components/BackButton";

interface UpgradeCustomNameProps extends PageProps { }

function UpgradeCustom({ }: UpgradeCustomNameProps) {
    const { address } = useAccount();
    const { openConnectModal } = useConnectModal();

    const [tbaAddress, setTbaAddress] = useState<string>("");
    const [implAddress, setImplAddress] = useState<string>("");
    const [showSuccess, setShowSuccess] = useState<boolean>(false);

    const { data: hash, sendTransaction, isPending, isError, error } = useSendTransaction({
        mutation: {
            onSuccess: (data) => {
                addRecentTransaction({ hash: data, description: `Upgrade implementation` });
            },
        },
    });

    const { isLoading: isConfirming, isSuccess: isConfirmed } =
        useWaitForTransactionReceipt({ hash });

    const addRecentTransaction = useAddRecentTransaction();

    useEffect(() => {
        document.title = "Upgrade Hyper Account";
    }, []);

    useEffect(() => {
        if (!address) {
            openConnectModal?.();
        }
    }, [address, openConnectModal]);

    const handleUpgrade = useCallback(
        async (e: FormEvent) => {
            e.preventDefault();
            e.stopPropagation();

            if (!address) {
                openConnectModal?.();
                return;
            }

            const tba = tbaAddress as `0x${string}`;
            const impl = implAddress as `0x${string}`;

            const data = encodeFunctionData({
                abi: tbaUpgradeAbi,
                functionName: "upgradeToAndCall",
                args: [impl, stringToHex("")],
            });

            try {
                sendTransaction({
                    to: tba,
                    data,
                    gas: 1000000n,
                });
            } catch (error) {
                console.error("Failed to send transaction:", error);
            }
        },
        [address, sendTransaction, openConnectModal, tbaAddress, implAddress],
    );

    const isFormValid = tbaAddress.trim() !== "" && implAddress.trim() !== "";

    // show success screen and reset form on confirmation
    useEffect(() => {
        if (isConfirmed) {
            setTbaAddress("");
            setImplAddress("");
            setShowSuccess(true);
        }
    }, [isConfirmed]);

    const handleUpgradeNew = () => {
        setShowSuccess(false);
    };

    return (
        <div className="container fade-in">
            <div className="section">
                <form className="form" onSubmit={handleUpgrade}>
                    {isPending || isConfirming ? (
                        <Loader msg={isConfirming ? "Upgrading..." : "Please confirm the transaction in your wallet"} />
                    ) : showSuccess ? (
                        <>
                            <p className="form-label">
                                <span>✅ Upgrade Successful!</span>
                            </p>
                            <p className="text-center mb-4">
                                Your Hyper Account has been successfully upgraded.
                            </p>
                            <div className="flex flex-col gap-1">
                                <button
                                    type="button"
                                    className="button"
                                    onClick={handleUpgradeNew}
                                >
                                    Upgrade New
                                </button>
                                <BackButton mode="wide" />
                            </div>
                        </>
                    ) : (
                        <>
                            <p className="form-label">
                                <span>Upgrade Hyper Account Implementation</span>
                            </p>

                            <div className="mb-4 text-sm text-gray-300 space-y-2">
                                <p>
                                    <strong>What is "Upgrade Hyper Account"?</strong>
                                </p>
                                <p>
                                    When minting a Hyper Account, you can make it upgradable. This allows the operator
                                    to update the contract implementation and add or remove various functions without
                                    changing the account address.
                                </p>
                                <p>
                                    This page allows you to upgrade an existing upgradable Hyper Account (TBA - Token Bound Account)
                                    to a new implementation contract. The new implementation can include additional features,
                                    bug fixes, or optimizations.
                                </p>
                                <p className="text-yellow-400">
                                    ⚠️ In the default implementation, only the operator can call upgradeToAndCall.
                                    This operation requires ERC-1967 support in both the account and implementation contracts.
                                </p>
                            </div>

                            <input
                                type="text"
                                name="tba"
                                placeholder="Enter TBA address to upgrade"
                                value={tbaAddress}
                                onChange={(e) => setTbaAddress(e.target.value)}
                            />
                            <input
                                type="text"
                                name="impl"
                                placeholder="Enter new implementation address"
                                value={implAddress}
                                onChange={(e) => setImplAddress(e.target.value)}
                            />

                            <div className="flex flex-col gap-1">
                                <button
                                    type="submit"
                                    className="button"
                                    disabled={!isFormValid || isPending || isConfirming}
                                >
                                    Upgrade Hyper Account
                                </button>
                                <BackButton mode="wide" />
                            </div>
                        </>
                    )}
                    {isError && !showSuccess && (
                        <p className="text-red-500 wrap-anywhere mt-2">
                            Error: {error?.message || "There was an error on upgrade"}
                        </p>
                    )}
                </form>
            </div>
        </div>
    );
}

export default UpgradeCustom;