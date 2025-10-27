import {
    FormEvent,
    useCallback,
    useEffect,
    useState,
} from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps, UnencryptedIdentity } from "../lib/types";
import { MULTICALL, mechAbi } from "../abis";
import { generateNetworkingKeys } from "../abis/helpers";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import SpecifyRoutersCheckbox from "../components/SpecifyRoutersCheckbox";
import EnterHnsName from "../components/EnterHnsName";

import { useAccount, useWaitForTransactionReceipt, useWriteContract } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit";
import BackButton from "../components/BackButton";

interface ResetProps extends PageProps { }

// Regex for valid router names (domain format)
const ROUTER_NAME_REGEX = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$/;

function ResetHnsName({
                          direct,
                          setDirect,
                          setReset,
                          hnsName,
                          setHnsName,
                          setNetworkingKey,
                          setIpAddress,
                          setWsPort,
                          setTcpPort,
                          setRouters,
                      }: ResetProps) {
    const { address } = useAccount();
    const navigate = useNavigate();
    const { openConnectModal } = useConnectModal();

    const { data: hash, writeContract, isPending, isError, error } = useWriteContract({
        mutation: {
            onSuccess: (data) => {
                addRecentTransaction({ hash: data, description: `Reset HNS ID: ${name}` });
            }
        }
    });
    const { isLoading: isConfirming, isSuccess: isConfirmed } =
        useWaitForTransactionReceipt({
            hash,
        });
    const addRecentTransaction = useAddRecentTransaction();

    const [name, setName] = useState<string>(hnsName);
    const [nameValidities, setNameValidities] = useState<string[]>([])
    const [tba, setTba] = useState<string>("");
    const [triggerNameCheck, setTriggerNameCheck] = useState<boolean>(false);
    const [specifyRouters, setSpecifyRouters] = useState(false)
    const [customRouters, setCustomRouters] = useState('')
    const [routerValidationErrors, setRouterValidationErrors] = useState<string[]>([])

    // Track initial states for checkbox help text
    const [initiallyDirect, setInitiallyDirect] = useState<boolean | undefined>(undefined);
    const [initiallySpecifyRouters, setInitiallySpecifyRouters] = useState<boolean | undefined>(undefined);

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
        document.title = "Reset";

        // Fetch current node info and prepopulate routers if indirect node
        (async () => {
            try {
                const infoData = (await fetch("/info", { method: "GET", credentials: 'include' }).then((res) =>
                    res.json()
                )) as UnencryptedIdentity;

                // Determine if node has specified routers (indirect node)
                const hasRouters = infoData.allowed_routers && infoData.allowed_routers.length > 0;

                // If allowed_routers is empty, the node is direct; if it has routers, it's indirect
                const isDirect = !hasRouters;

                // Set initial states for checkbox help text
                setInitiallyDirect(isDirect);
                setInitiallySpecifyRouters(hasRouters);

                // Set the current state to match the node's current configuration
                setDirect(isDirect);

                // Prepopulate customRouters if this is an indirect node with existing routers
                if (hasRouters) {
                    const routersText = infoData.allowed_routers.join('\n');
                    setCustomRouters(routersText);
                    setSpecifyRouters(true); // Auto-enable the checkbox
                }
            } catch (error) {
                console.log("Could not fetch node info:", error);
            }
        })();
    }, []);

    // so inputs will validate once wallet is connected
    useEffect(() => setTriggerNameCheck(!triggerNameCheck), [address]); // eslint-disable-line react-hooks/exhaustive-deps

    useEffect(() => {
        if (!address) {
            openConnectModal?.();
        }
    }, [address, openConnectModal]);

    const handleResetRecords = useCallback(
        async (e: FormEvent) => {
            e.preventDefault();
            e.stopPropagation();

            if (!address) {
                openConnectModal?.();
                return;
            }

            setHnsName(name);

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
                const data = await generateNetworkingKeys({
                    upgradable: false,
                    direct,
                    label: name,
                    our_address: address,
                    setNetworkingKey,
                    setIpAddress,
                    setWsPort,
                    setTcpPort,
                    setRouters: routersToUse.length > 0 ? () => setRouters(routersToUse) : setRouters,
                    reset: true,
                    customRouters: routersToUse.length > 0 ? routersToUse : undefined,
                });

                writeContract({
                    address: tba as `0x${string}`,
                    abi: mechAbi,
                    functionName: "execute",
                    args: [
                        MULTICALL,
                        BigInt(0),
                        data,
                        1
                    ],
                    gas: 1000000n,
                });
            } catch (error) {
                console.error("An error occurred:", error);
            }
        },
        [address, direct, specifyRouters, customRouters, name, tba, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, writeContract, openConnectModal, getValidCustomRouters, setHnsName]
    );

    useEffect(() => {
        if (isConfirmed) {
            setReset(true);
            setDirect(direct);
            navigate("/set-password");
        }
    }, [isConfirmed, setReset, setDirect, direct, navigate]);


    return (
        <div className="container fade-in" id="register-ui--reset-name">
            <div className="section">
                {
                    <form className="form" onSubmit={handleResetRecords}>
                        {isPending || isConfirming ? (
                            <Loader msg={isConfirming ? "Resetting Networking Information..." : "Please confirm the transaction in your wallet"} />
                        ) : (
                            <>
                                <h3 className="text-iris dark:text-neon">
                                    Node ID to reset:
                                </h3>
                                <EnterHnsName {...{ address, name, setName, triggerNameCheck, nameValidities, setNameValidities, setTba, isReset: true }} />
                                <p className="text-sm text-gray-500">
                                    Nodes use an onchain username in order to identify themselves to other nodes in the network.
                                </p>
                                <details className="advanced-options">
                                    <summary>Advanced Network Options</summary>
                                    <div className="flex flex-col gap-3">
                                        <DirectNodeCheckbox
                                            direct={direct}
                                            setDirect={handleSetDirect}
                                            initiallyChecked={initiallyDirect}
                                        />
                                        <SpecifyRoutersCheckbox
                                            specifyRouters={specifyRouters}
                                            setSpecifyRouters={handleSetSpecifyRouters}
                                            initiallyChecked={initiallySpecifyRouters}
                                        />
                                        {specifyRouters && (
                                            <div className="flex flex-col gap-2 ml-6">
                                                <label htmlFor="custom-routers" className="text-sm font-medium">
                                                    Router Names: <span className="text-red-500">*</span>
                                                </label>
                                                <textarea
                                                    id="custom-routers-reset"
                                                    value={customRouters}
                                                    onChange={(e) => handleCustomRoutersChange(e.target.value)}
                                                    placeholder="Enter one router name per line, e.g.:&#10;router-node-1.hypr&#10;other-router.hypr&#10;myrouter.os"
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
                                <p className="text-sm text-gray-500">
                                    A reset will not delete any data. It only updates the networking info your node publishes onchain.
                                </p>
                                <button
                                    type="submit"
                                    className="button mt-2 self-stretch"
                                    disabled={
                                        isPending ||
                                        isConfirming ||
                                        nameValidities.length !== 0 ||
                                        (specifyRouters && !isCustomRoutersValid())
                                    }
                                >
                                    Reset Node
                                </button>

                                <BackButton mode="wide" />
                            </>
                        )}
                        {isError && (
                            <p className="text-red-500 wrap-anywhere mt-2">
                                Error: {error?.message || "An error occurred, please try again."}
                            </p>
                        )}
                    </form>
                }
            </div>
        </div>
    );
}

export default ResetHnsName;
