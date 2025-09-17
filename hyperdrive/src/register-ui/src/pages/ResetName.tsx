import {
  FormEvent,
  useCallback,
  useEffect,
  useState,
} from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";
import { MULTICALL, generateNetworkingKeys, mechAbi } from "../abis";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import SpecifyRoutersCheckbox from "../components/SpecifyRoutersCheckbox";
import EnterHnsName from "../components/EnterHnsName";

import { useAccount, useWaitForTransactionReceipt, useWriteContract } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit";
import BackButton from "../components/BackButton";

interface ResetProps extends PageProps { }

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
    document.title = "Reset";
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

      try {
        const data = await generateNetworkingKeys({
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
    [address, direct, specifyRouters, customRouters, tba, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, writeContract, openConnectModal]
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
                              id="custom-routers-reset"
                              value={customRouters}
                              onChange={(e) => setCustomRouters(e.target.value)}
                              placeholder="Enter one router name per line, e.g.:&#10;router-node-1.hypr&#10;other-router.hypr&#10;myrouter.os"
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
                        (specifyRouters && customRouters.split('\n').map(r => r.trim()).filter(r => r.length > 0).length === 0)
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
