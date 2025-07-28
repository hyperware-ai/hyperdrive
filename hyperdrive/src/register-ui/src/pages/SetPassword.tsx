import React, { useState, useEffect, FormEvent, useCallback } from "react";
import Loader from "../components/Loader";
import { downloadKeyfile } from "../utils/download-keyfile";
import { Tooltip } from "../components/Tooltip";
import { useSignTypedData, useAccount, useChainId, usePublicClient } from 'wagmi'
import { HYPERMAP } from "../abis";
import { redirectToHomepage } from "../utils/redirect-to-homepage";
import { getWalletType, WalletType } from "../utils/wallet-detection";

type SetPasswordProps = {
  direct: boolean;
  pw: string;
  reset: boolean;
  hnsName: string;
  setPw: React.Dispatch<React.SetStateAction<string>>;
  nodeChainId: string;
  closeConnect: () => void;
};

function SetPassword({
  hnsName,
  direct,
  pw,
  reset,
  setPw,
}: SetPasswordProps) {
  const [pw2, setPw2] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState<boolean>(false);
  const [walletType, setWalletType] = useState<WalletType>('EOA');
  const [loadingMessage, setLoadingMessage] = useState<string>("Please sign the structured message in your wallet to set your password.");

  const { signTypedDataAsync } = useSignTypedData();
  const { address } = useAccount();
  const chainId = useChainId();
  const publicClient = usePublicClient();

  useEffect(() => {
    document.title = "Set Password";
  }, []);

  useEffect(() => {
    setError("");
  }, [pw, pw2]);

  // Detect wallet type when address changes
  useEffect(() => {
    async function detectWallet() {
      if (address && publicClient) {
        const type = await getWalletType(address, publicClient);
        setWalletType(type);

        // Update loading message based on wallet type
        if (type === 'SAFE') {
          setLoadingMessage("Please approve the message in your Safe app. All required owners must sign.");
        } else if (type === 'UNKNOWN_CONTRACT') {
          setLoadingMessage("Please sign the message in your smart contract wallet.");
        } else {
          setLoadingMessage("Please sign the structured message in your wallet to set your password.");
        }
      }
    }
    detectWallet();
  }, [address, publicClient]);

  const handleSubmit = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();

      if (pw !== pw2) {
        setError("Passwords do not match");
        return false;
      }

      setTimeout(async () => {
        setLoading(true);

        // salt is either node name (if node name is longer than 8 characters)
        //  or node name repeated enough times to be longer than 8 characters
        const minSaltL = 8;
        const nodeL = hnsName.length;
        const salt = nodeL >= minSaltL ? hnsName : hnsName.repeat(1 + Math.floor(minSaltL / nodeL));
        console.log(salt);

        //@ts-ignore
        argon2.hash({
          pass: pw,
          salt: salt,
          hashLen: 32,
          time: 2,
          mem: 19456,
          //@ts-ignore
          type: argon2.ArgonType.Argon2id
        }).then(async (h: any) => {
          const hashed_password_hex = `0x${h.hashHex}` as `0x${string}`;
          let owner = address;
          let timestamp = Date.now();

          const signature = await signTypedDataAsync({
            domain: {
              name: "Hypermap",
              version: "1",
              chainId: chainId,
              verifyingContract: HYPERMAP,
            },
            types: {
              Boot: [
                { name: 'username', type: 'string' },
                { name: 'password_hash', type: 'bytes32' },
                { name: 'timestamp', type: 'uint256' },
                { name: 'direct', type: 'bool' },
                { name: 'reset', type: 'bool' },
                { name: 'chain_id', type: 'uint256' },
              ],
            },
            primaryType: 'Boot',
            message: {
              username: hnsName,
              password_hash: hashed_password_hex,
              timestamp: BigInt(timestamp),
              direct,
              reset,
              chain_id: BigInt(chainId),
            },
          });

          try {
            const result = await fetch("/boot", {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              credentials: "include",
              body: JSON.stringify({
                password_hash: hashed_password_hex,
                reset,
                username: hnsName,
                direct,
                owner,
                timestamp,
                signature,
                chain_id: chainId,
              }),
            });
            const base64String = await result.json();

            downloadKeyfile(hnsName, base64String);
            redirectToHomepage();

          } catch {
            alert("There was an error setting your password, please try again.");
            setLoading(false);
          }
        }).catch((err: any) => {
          alert(String(err));
          setLoading(false);
        });
      }, 500);
    },
    [direct, pw, pw2, reset, hnsName]
  );

  return (
    <>
      {loading ? (
        <Loader msg={loadingMessage} />
      ) : (
        <form className="form" onSubmit={handleSubmit}>
          <div className="form-group">
            <h3 className="form-label">Set password for {hnsName}</h3>
            <p className="text-sm text-gray-500">
              This password will be used to log in when you restart your node or switch browsers.
            </p>
            <input
              type="password"
              id="password"
              required
              minLength={6}
              name="password"
              placeholder="6 characters minimum"
              value={pw}
              onChange={(e) => setPw(e.target.value)}
              autoFocus
            />
          </div>
          <div className="form-group">
            <label className="form-label" htmlFor="confirm-password">Confirm Password</label>
            <input
              type="password"
              id="confirm-password"
              required
              minLength={6}
              name="confirm-password"
              placeholder="6 characters minimum"
              value={pw2}
              onChange={(e) => setPw2(e.target.value)}
            />
          </div>
          {Boolean(error) && <p className="text-red-500 wrap-anywhere mt-2">{error}</p>}
          <button type="submit" className="button">Submit</button>
        </form>
      )}
    </>
  );
}

export default SetPassword;
