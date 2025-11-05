import { useState, useEffect, FormEvent, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import UpgradableCheckbox from "../components/UpgradableCheckbox";
import { useAccount, useWaitForTransactionReceipt, useSendTransaction, useConfig } from "wagmi";
import { readContract } from "wagmi/actions";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit";
import { tbaMintAbi, HYPER_ACCOUNT_IMPL, HYPER_ACCOUNT_UPGRADABLE_IMPL, HYPERMAP, mechAbi } from "../abis";
import { generateNetworkingKeys } from "../abis/helpers";
import SpecifyRoutersCheckbox from "../components/SpecifyRoutersCheckbox";
import { encodePacked, encodeFunctionData, stringToHex } from "viem";
import BackButton from "../components/BackButton";
import { predictTBAAddress } from "../utils/predictTBA";
import { hyperhash } from "../utils/hyperhash";

interface MintCustomNameProps extends PageProps { }

// Regex for valid router names (domain format)
const ROUTER_NAME_REGEX = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$/;

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

    const [triggerNameCheck, setTriggerNameCheck] = useState<boolean>(false)
    const [specifyRouters, setSpecifyRouters] = useState(false)
    const [customRouters, setCustomRouters] = useState('')
    const [routerValidationErrors, setRouterValidationErrors] = useState<string[]>([])

    // Modified setDirect function - no longer clears custom routers
    const handleSetDirect = (value: boolean) => {
        setDirect(value);
        if (value) {
            setSpecifyRouters(false);
        }
    };

    // Modified setSpecifyRouters function - no longer clears custom routers
    const handleSetSpecifyRouters = (value: boolean) => {
        setSpecifyRouters(value);
        if (value) {
            setDirect(false);
        }
    };

    // Validate custom routers against the regex
    const validateRouters = (routersText: string): string[] => {
        if (!routersText.trim()) return [];

        const routers = routersText
            .split('\n')
            .map(router => router.trim())
            .filter(router => router.length > 0);

        const errors: string[] = [];
        routers.forEach((router, index) => {
            if (!ROUTER_NAME_REGEX.test(router)) {
                errors.push(`Line ${index + 1}: "${router}" is not a valid router name`);
            }
        });

        return errors;
    };

    // Handle custom routers change with validation
    const handleCustomRoutersChange = (value: string) => {
        setCustomRouters(value);
        if (specifyRouters && value.trim()) {
            const errors = validateRouters(value);
            setRouterValidationErrors(errors);
        } else {
            setRouterValidationErrors([]);
        }
    };

    // Add a validation function for custom routers
    const getValidCustomRouters = () => {
        if (!specifyRouters) return [];
        return customRouters
            .split('\n')
            .map(router => router.trim())
            .filter(router => router.length > 0 && ROUTER_NAME_REGEX.test(router));
    };

    const isCustomRoutersValid = () => {
        if (!specifyRouters) return true; // Not required if checkbox is unchecked
        const validRouters = getValidCustomRouters();
        return validRouters.length > 0 && routerValidationErrors.length === 0;
    };

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

    let handleMint = useCallback(async (e: FormEvent) => {
        e.preventDefault()
        e.stopPropagation()

        const formData = new FormData(e.target as HTMLFormElement)

        if (!address) {
            openConnectModal?.()
            return
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

        // Process custom routers only if the checkbox is checked
        let routersToUse: string[] = [];
        if (specifyRouters && customRouters.trim()) {
            routersToUse = getValidCustomRouters();
            setRouters(routersToUse);
            console.log("Custom routers:", routersToUse);
        } else {
            // Clear routers in app state if not specifying custom routers
            setRouters([]);
        }

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
                setRouters: routersToUse.length > 0 ? () => setRouters(routersToUse) : setRouters,
                reset: false,
                customRouters: routersToUse.length > 0 ? routersToUse : undefined,
                tbaAddress: predictedTBA.predictedAddress,
            });

            setHnsName(formData.get('full-hns-name') as string)

            console.log("full hns name", formData.get('full-hns-name'))
            console.log("name", name)

            const impl = upgradable ? HYPER_ACCOUNT_UPGRADABLE_IMPL : HYPER_ACCOUNT_IMPL;
            const data = encodeFunctionData({
                abi: tbaMintAbi,
                functionName: 'mint',
                args: [
                    address,
                    encodePacked(["bytes"], [stringToHex(name)]),
                    initCall,
                    impl,
                ],
            })

            // use data to write to contract -- do NOT use writeContract
            // writeContract will NOT generate the correct selector for some reason
            // probably THEIR bug.. no abi works
            sendTransaction({
                to: formData.get('tba') as `0x${string}`,
                data: data,
                gas: 1000000n,
            })
        } catch (error) {
            console.error('Failed to read or write to contract:', error)
        }
    }, [config, getValidCustomRouters, hnsName, setHnsName, upgradable, direct, specifyRouters, customRouters, address, sendTransaction, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, openConnectModal])

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
                                    <summary>Advanced Network Options</summary>
                                    <div className="flex flex-col gap-3">
                                        <UpgradableCheckbox {...{ upgradable, setUpgradable }} />
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
                                                    onChange={(e) => handleCustomRoutersChange(e.target.value)}
                                                    placeholder="Enter one router name per line, e.g.:&#10;direct-router-1.hypr&#10;direct-other.hypr&#10;mydirectrouter.os"
                                                    className={`input resize-vertical min-h-[80px] ${
                                                        specifyRouters && !isCustomRoutersValid()
                                                            ? 'border-red-500 focus:border-red-500'
                                                            : ''
                                                    }`}
                                                    rows={4}
                                                />
                                                {routerValidationErrors.length > 0 ? (
                                                    <div className="text-xs text-red-500">
                                                        {routerValidationErrors.map((error, idx) => (
                                                            <div key={idx}>{error}</div>
                                                        ))}
                                                        <div className="mt-1">Router names must contain only lowercase letters, numbers, hyphens (not at start/end), and dots.</div>
                                                    </div>
                                                ) : (
                                                    <span className={`text-xs ${
                                                        !isCustomRoutersValid() ? 'text-red-500' : 'text-gray-500'
                                                    }`}>
                                                        {!isCustomRoutersValid()
                                                            ? 'At least one valid router name is required'
                                                            : 'Enter one router name per line. These routers will be used for your indirect node.'
                                                        }
                                                    </span>
                                                )}
                                            </div>
                                        )}
                                    </div>
                                </details>
                                <button
                                    type="submit"
                                    className="button"
                                    disabled={
                                        isPending ||
                                        isConfirming ||
                                        (specifyRouters && !isCustomRoutersValid())
                                    }
                                >
                                    Mint Custom Name
                                </button>

                                <BackButton mode="wide" />
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
                }
            </div>
        </div>
    );
}

export default MintCustom;
