import React, { useState, useEffect, FormEvent, useCallback } from "react";
import Loader from "../components/Loader";
import { downloadKeyfile } from "../utils/download-keyfile";
import { Tooltip } from "../components/Tooltip";
import { useSignTypedData, useAccount, useChainId } from 'wagmi'
import { HYPERMAP } from "../abis";
import { getHypermapAddress } from "../abis/addresses";
import { redirectToHomepage } from "../utils/redirect-to-homepage";

type SetPasswordDebugProps = {
  direct: boolean;
  pw: string;
  reset: boolean;
  hnsName: string;
  setPw: React.Dispatch<React.SetStateAction<string>>;
  nodeChainId: string;
  closeConnect: () => void;
};

function SetPasswordDebug({
  hnsName,
  direct,
  pw,
  reset,
  setPw,
}: SetPasswordDebugProps) {
  const [pw2, setPw2] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState<boolean>(false);
  const [debugInfo, setDebugInfo] = useState<string>("");
  const [showDebug, setShowDebug] = useState<boolean>(false);

  const { signTypedDataAsync } = useSignTypedData();
  const { address } = useAccount();
  const chainId = useChainId();

  useEffect(() => {
    document.title = "Set Password (Debug)";
  }, []);

  useEffect(() => {
    setError("");
  }, [pw, pw2]);

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
        console.log("Salt:", salt);

        argon2.hash({
          pass: pw,
          salt: salt,
          hashLen: 32,
          time: 2,
          mem: 19456,
          type: argon2.ArgonType.Argon2id
        }).then(async h => {
          const hashed_password_hex = `0x${h.hashHex}` as `0x${string}`;
          let owner = address;
          let timestamp = Date.now();

          // Get the correct HYPERMAP address for this chain
          const hypermapAddress = getHypermapAddress(chainId);

          console.log("Debug info:");
          console.log("Chain ID:", chainId);
          console.log("HYPERMAP address:", hypermapAddress);
          console.log("Owner address:", owner);
          console.log("HNS name:", hnsName);

          // Prepare signing data
          const domain = {
            name: "Hypermap",
            version: "1",
            chainId: chainId,
            verifyingContract: hypermapAddress,
          };

          const types = {
            Boot: [
              { name: 'username', type: 'string' },
              { name: 'password_hash', type: 'bytes32' },
              { name: 'timestamp', type: 'uint256' },
              { name: 'direct', type: 'bool' },
              { name: 'reset', type: 'bool' },
              { name: 'chain_id', type: 'uint256' },
            ],
          };

          const message = {
            username: hnsName,
            password_hash: hashed_password_hex,
            timestamp: BigInt(timestamp),
            direct,
            reset,
            chain_id: BigInt(chainId),
          };

          // Show debug info
          const debugData = {
            domain,
            types,
            message: {
              ...message,
              timestamp: message.timestamp.toString(),
              chain_id: message.chain_id.toString(),
            },
            primaryType: 'Boot',
            additionalInfo: {
              hypermapAddress,
              ownerAddress: owner,
              chainId,
              originalHYPERMAP: HYPERMAP,
            }
          };

          setDebugInfo(JSON.stringify(debugData, null, 2));
          setShowDebug(true);

          try {
            console.log("Signing with data:", debugData);

            const signature = await signTypedDataAsync({
              domain,
              types,
              primaryType: 'Boot',
              message,
            });

            console.log("Signature received:", signature);

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

          } catch (signError) {
            console.error("Signing error:", signError);
            alert(`Signing error: ${signError.message || JSON.stringify(signError)}`);
            setLoading(false);
          }
        }).catch(err => {
          alert(String(err));
          setLoading(false);
        });
      }, 500);
    },
    [direct, pw, pw2, reset, hnsName, address, chainId, signTypedDataAsync]
  );

  return (
    <>
      {loading ? (
        <Loader msg="Please sign the structured message in your wallet to set your password." />
      ) : (
        <form className="form" onSubmit={handleSubmit}>
          <div className="form-group">
            <Tooltip text="This password will be used to log in when you restart your node or switch browsers.">
              <label className="form-label" htmlFor="password">Set password for {hnsName}</label>
            </Tooltip>
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
          {Boolean(error) && <p className="error-message">{error}</p>}

          {showDebug && (
            <details>
              <summary>Debug Info (for Gnosis Safe)</summary>
              <pre style={{
                background: '#f0f0f0',
                padding: '10px',
                borderRadius: '5px',
                fontSize: '12px',
                overflow: 'auto',
                maxHeight: '300px'
              }}>
                {debugInfo}
              </pre>
              <p style={{ fontSize: '12px', marginTop: '10px' }}>
                If the message doesn't appear in Gnosis Safe, you can copy the above data and manually verify the signing request.
              </p>
              <div style={{ marginTop: '10px', padding: '10px', background: '#e8f4f8', borderRadius: '5px' }}>
                <strong style={{ fontSize: '12px' }}>Troubleshooting:</strong>
                <ul style={{ fontSize: '12px', marginTop: '5px', paddingLeft: '20px' }}>
                  <li>Check that the chainId matches your wallet's network</li>
                  <li>The HYPERMAP address might need to be updated for your network</li>
                  <li>Try using the CLI script instead: <code>node password-only-setup.js</code></li>
                </ul>
                <p style={{ fontSize: '12px', marginTop: '10px' }}>
                  <strong>Current HYPERMAP address:</strong> {hypermapAddress || 'Not set'}
                </p>
              </div>
            </details>
          )}

          <button type="submit" className="button">Submit</button>
        </form>
      )}
    </>
  );
}

export default SetPasswordDebug;
