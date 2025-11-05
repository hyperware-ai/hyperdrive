import { useState, useEffect, FormEvent, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { toAscii } from "idna-uts46-hx";
import EnterHnsName from "../components/EnterHnsName";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import SpecifyRoutersCheckbox from "../components/SpecifyRoutersCheckbox";

import { useAccount, useWaitForTransactionReceipt, useWriteContract } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit"
import { dotOsAbi, DOTOS } from "../abis";
import { createPublicClient, http, stringToHex, encodeAbiParameters, parseAbiParameters, keccak256, BaseError, ContractFunctionRevertedError } from "viem";
import { base } from 'viem/chains'
import BackButton from "../components/BackButton";
interface RegisterOsNameProps extends PageProps { }

// Regex for valid router names (domain format)
const ROUTER_NAME_REGEX = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$/;

function CommitDotOsName({
    direct,
    setDirect,
    setHnsName,
    setNetworkingKey,
    setIpAddress,
    setWsPort,
    setTcpPort,
    setRouters,
}: RegisterOsNameProps) {
    let { address } = useAccount();
    let navigate = useNavigate();
    let { openConnectModal } = useConnectModal();

    const { data: hash, writeContract, isPending, isError, error } = useWriteContract({
        mutation: {
            onSuccess: (data) => {
                addRecentTransaction({ hash: data, description: `Pre-commit to .os ID: ${name}.os` });
            }
        }
    });
    const { isLoading: isConfirming, isSuccess: txConfirmed } =
        useWaitForTransactionReceipt({
            hash,
        });
    const addRecentTransaction = useAddRecentTransaction();

    const [name, setName] = useState('')
    const [nameValidities, setNameValidities] = useState<string[]>([])
    const [triggerNameCheck, setTriggerNameCheck] = useState<boolean>(false)
    const [isConfirmed, setIsConfirmed] = useState(false)
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
        document.title = "Register"
    }, [])

    useEffect(() => setTriggerNameCheck(!triggerNameCheck), [address])

    const enterOsNameProps = { address, name, setName, fixedTlz: ".os", nameValidities, setNameValidities, triggerNameCheck }

    useEffect(() => {
        if (!address) {
            openConnectModal?.();
        }
    }, [address, openConnectModal]);

    let handleCommit = useCallback(async (e: FormEvent) => {
        e.preventDefault()
        e.stopPropagation()
        if (!address) {
            openConnectModal?.()
            return
        }
        setName(toAscii(name));
        console.log("committing to .os name: ", name)

        // Process custom routers only if the checkbox is checked
        if (specifyRouters && customRouters.trim()) {
            const routersToUse = getValidCustomRouters();
            setRouters(routersToUse);
        } else {
            // Clear routers in app state if not specifying custom routers
            setRouters([]);
        }

        const commit = keccak256(
            encodeAbiParameters(
                parseAbiParameters('bytes memory, address'),
                [stringToHex(name), address]
            )
        )

        const publicClient = createPublicClient({
            chain: base,
            transport: http(),
        });

        try {
            const { request } = await publicClient.simulateContract({
                abi: dotOsAbi,
                address: DOTOS,
                functionName: 'commit',
                args: [commit],
                account: address
            });

            writeContract(request);
        } catch (err) {
            if (err instanceof BaseError) {
                const revertError = err.walk(err => err instanceof ContractFunctionRevertedError)
                if (revertError instanceof ContractFunctionRevertedError) {
                    if (revertError?.data) {
                        const errorName = revertError.data.errorName;
                        const args = revertError.data.args;
                        console.log(`Reverted with ${errorName}`, args);
                    }
                }
            }
            throw err;
        }

    }, [name, specifyRouters, customRouters, direct, address, writeContract, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, openConnectModal])

    useEffect(() => {
        if (txConfirmed) {
            console.log("confirmed commit to .os name: ", name)
            console.log("waiting 16 seconds to make commit valid...")
            setTimeout(() => {
                setIsConfirmed(true);
                setHnsName(`${name}.os`);

                if (specifyRouters && customRouters.trim()) {
                    const routersToUse = getValidCustomRouters();
                    setRouters(routersToUse);
                }
                navigate("/mint-os-name");
            }, 16000)
        }
    }, [txConfirmed, address, name, setHnsName, navigate, specifyRouters, customRouters, setRouters]);

    return (
        <div className="container fade-in">
            <div className="section">
                {
                    <form className="form" onSubmit={handleCommit}>
                        {isPending || isConfirming || (txConfirmed && !isConfirmed) ? (
                            <Loader msg={
                                isConfirming ? 'Pre-committing to chosen name...' :
                                    (txConfirmed && !isConfirmed) ? 'Waiting 15s for commit to become valid...' :
                                        'Please confirm the transaction in your wallet'
                            } />
                        ) : (
                            <>
                                <h3 className="form-label">
                                    Choose a name for your node
                                </h3>
                                <p className="text-sm text-gray-500">
                                    Nodes need an onchain node identity in order to communicate with other nodes in the network.
                                </p>
                                <EnterHnsName {...enterOsNameProps} />
                                <details className="advanced-options">
                                    <summary>Advanced Network Options</summary>
                                    <div className="flex flex-col gap-3">
                                        <DirectNodeCheckbox direct={direct} setDirect={handleSetDirect} />
                                        <SpecifyRoutersCheckbox specifyRouters={specifyRouters} setSpecifyRouters={handleSetSpecifyRouters} />
                                        {specifyRouters && (
                                            <div className="flex flex-col gap-2 ml-6">
                                                <label htmlFor="custom-routers" className="text-sm font-medium">
                                                    Router Names: <span className="text-red-500">*</span>
                                                </label>
                                                <textarea
                                                    id="custom-routers"
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
                                <button
                                    type="submit"
                                    className="button"
                                    disabled={
                                        isPending ||
                                        isConfirming ||
                                        nameValidities.length !== 0 ||
                                        (specifyRouters && !isCustomRoutersValid())
                                    }
                                >
                                    Pre-commit
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

export default CommitDotOsName;
