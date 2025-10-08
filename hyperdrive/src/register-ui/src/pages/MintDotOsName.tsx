
import { useState, useEffect, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import Loader from "../components/Loader";
import { PageProps } from "../lib/types";

import { useAccount, useWaitForTransactionReceipt, useWriteContract } from "wagmi";
import { useConnectModal, useAddRecentTransaction } from "@rainbow-me/rainbowkit"
import { generateNetworkingKeys, HYPER_ACCOUNT_IMPL, DOTOS, tbaMintAbi } from "../abis";
import { createPublicClient, encodePacked, http, stringToHex, BaseError, ContractFunctionRevertedError } from "viem";
import { base } from 'viem/chains'

interface RegisterOsNameProps extends PageProps { }

function MintDotOsName({
                         direct,
                         hnsName,
                         setNetworkingKey,
                         setIpAddress,
                         setWsPort,
                         setTcpPort,
                         setRouters,
                         routers,
                       }: RegisterOsNameProps) {
  let { address } = useAccount();
  let navigate = useNavigate();
  let { openConnectModal } = useConnectModal();

  // Add debugging for props received
  useEffect(() => {
  }, [direct, hnsName, routers]);

  const { data: hash, writeContract, isPending, isError, error } = useWriteContract({
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

  const [hasMinted, setHasMinted] = useState(false);

  useEffect(() => {
    document.title = "Mint"
  }, [])

  useEffect(() => {
    if (!address) {
      openConnectModal?.();
    }
  }, [address, openConnectModal]);

  const handleMint = useCallback(async () => {
    if (!address) {
      openConnectModal?.()
      return
    }
    if (hasMinted) {
      return
    }

    setHasMinted(true);

    // Use the routers from app state if they exist (custom routers from previous page)
    const customRoutersToUse = routers && routers.length > 0 ? routers : undefined;

    const initCall = await generateNetworkingKeys({
      direct,
      our_address: address,
      label: hnsName,
      setNetworkingKey,
      setIpAddress,
      setWsPort,
      setTcpPort,
      setRouters,
      reset: false,
      customRouters: customRoutersToUse, // Pass the custom routers
    });

    // strip .os suffix
    const name = hnsName.replace(/\.os$/, '');

    const publicClient = createPublicClient({
      chain: base,
      transport: http(),
    });

    try {
      const { request } = await publicClient.simulateContract({
        abi: tbaMintAbi,
        address: DOTOS,
        functionName: 'mint',
        args: [
          address,
          encodePacked(["bytes"], [stringToHex(name)]),
          initCall,
          HYPER_ACCOUNT_IMPL,
        ],
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
  }, [direct, address, writeContract, setNetworkingKey, setIpAddress, setWsPort, setTcpPort, setRouters, openConnectModal, hnsName, hasMinted, routers])

  useEffect(() => {
    if (address && !isPending && !isConfirming) {
      handleMint();
    }
  }, [address, handleMint, isPending, isConfirming]);

  useEffect(() => {
    if (isConfirmed) {
      navigate("/set-password");
    }
  }, [isConfirmed, address, navigate]);

  return (
      <div className="container fade-in">
        <div className="section">
          <div className="form">
            {isPending || isConfirming ? (
                <Loader msg={isConfirming ? 'Minting name...' : 'Please confirm the transaction in your wallet'} />
            ) : (
                <Loader msg="Preparing to mint..." />
            )}
            {isError && (
                <p className="text-red-500 wrap-anywhere mt-2">
                  Error: {error?.message || 'There was an error minting your name, please try again.'}
                </p>
            )}
          </div>
        </div>
      </div>
  );
}

export default MintDotOsName;