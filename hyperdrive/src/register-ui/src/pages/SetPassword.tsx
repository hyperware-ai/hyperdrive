import React, { useState, useEffect, FormEvent, useCallback } from "react";
import Loader from "../components/Loader";
import { downloadKeyfile } from "../utils/download-keyfile";
import { useSignTypedData, useAccount, useChainId } from 'wagmi'
import { HYPERMAP } from "../abis";
import { redirectToHomepage } from "../utils/redirect-to-homepage";
import SpecifyCacheSourcesCheckbox from "../components/SpecifyCacheSourcesCheckbox";
import SpecifyBaseL2AccessProvidersCheckbox from "../components/SpecifyBaseL2AccessProvidersCheckbox";
import { RpcProviderEditor, RpcProviderData } from "../components/RpcProviderEditor";
import { InfoResponse } from "../lib/types";

type SetPasswordProps = {
  direct: boolean;
  pw: string;
  reset: boolean;
  hnsName: string;
  setPw: React.Dispatch<React.SetStateAction<string>>;
  nodeChainId: string;
  closeConnect: () => void;
  routers?: string[];
};

// Regex for valid cache source names (domain format)
const CACHE_SOURCE_REGEX = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$/;

// Validate that URL is a secure WebSocket URL
const validateWebSocketUrl = (url: string): boolean => {
  if (!url.trim()) return false;
  return url.startsWith('wss://');
};

function SetPassword({
                       hnsName,
                       direct,
                       pw,
                       reset,
                       setPw,
                       routers,
                     }: SetPasswordProps) {
  const [pw2, setPw2] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState<boolean>(false);

  // Advanced options state - cache sources and Base L2 access providers
  const [specifyCacheSources, setSpecifyCacheSources] = useState(false);
  const [customCacheSources, setCustomCacheSources] = useState('');
  const [cacheSourceValidationErrors, setCacheSourceValidationErrors] = useState<string[]>([]);
  const [specifyBaseL2AccessProviders, setSpecifyBaseL2AccessProviders] = useState(false);
  const [rpcProviders, setRpcProviders] = useState<RpcProviderData[]>([]);

  // Track initial states after data is loaded
  const [initialCacheSourcesChecked, setInitialCacheSourcesChecked] = useState(false);
  const [initialBaseL2ProvidersChecked, setInitialBaseL2ProvidersChecked] = useState(false);

  const { signTypedDataAsync } = useSignTypedData();
  const { address } = useAccount();
  const chainId = useChainId();

  useEffect(() => {
    document.title = "Set Password";
  }, []);

  // Fetch default values from /info endpoint
  useEffect(() => {
    (async () => {
      try {
        const infoData = (await fetch("/info", { method: "GET", credentials: 'include' }).then((res) =>
            res.json()
        )) as InfoResponse;

        // Prepopulate cache sources
        if (infoData.initial_cache_sources && infoData.initial_cache_sources.length > 0) {
          setCustomCacheSources(infoData.initial_cache_sources.join('\n'));
          setSpecifyCacheSources(true);
          setInitialCacheSourcesChecked(true);
        }

        // Parse and prepopulate Base L2 providers
        if (infoData.initial_base_l2_providers && infoData.initial_base_l2_providers.length > 0) {
          const parsedProviders: RpcProviderData[] = infoData.initial_base_l2_providers.map(providerStr => {
            try {
              const parsed = JSON.parse(providerStr);
              // Convert from backend format to frontend format
              let authData = null;
              if (parsed.auth) {
                if (parsed.auth.Basic) {
                  authData = { type: 'Basic' as const, value: parsed.auth.Basic };
                } else if (parsed.auth.Bearer) {
                  authData = { type: 'Bearer' as const, value: parsed.auth.Bearer };
                } else if (parsed.auth.Raw) {
                  authData = { type: 'Raw' as const, value: parsed.auth.Raw };
                }
              }
              return {
                url: parsed.url,
                auth: authData
              };
            } catch {
              // If parsing fails, treat as plain URL string
              return {
                url: providerStr,
                auth: null
              };
            }
          });
          setRpcProviders(parsedProviders);
          setSpecifyBaseL2AccessProviders(true);
          setInitialBaseL2ProvidersChecked(true);
        }
      } catch (error) {
        console.error('Failed to fetch default configuration:', error);
      }
    })();
  }, []);

  useEffect(() => {
    setError("");
  }, [pw, pw2]);

  // Modified setSpecifyCacheSources function to handle clearing
  const handleSetSpecifyCacheSources = (value: boolean) => {
    setSpecifyCacheSources(value);
    if (!value) {
      setCustomCacheSources('');
      setCacheSourceValidationErrors([]);
    }
  };

  // Validate custom cache sources against the regex
  const validateCacheSources = (sourcesText: string): string[] => {
    if (!sourcesText.trim()) return [];

    const sources = sourcesText
        .split('\n')
        .map(source => source.trim())
        .filter(source => source.length > 0);

    const errors: string[] = [];
    sources.forEach((source, index) => {
      if (!CACHE_SOURCE_REGEX.test(source)) {
        errors.push(`Line ${index + 1}: "${source}" is not a valid cache source name`);
      }
    });

    return errors;
  };

  // Handle custom cache sources change with validation
  const handleCustomCacheSourcesChange = (value: string) => {
    setCustomCacheSources(value);
    if (specifyCacheSources && value.trim()) {
      const errors = validateCacheSources(value);
      setCacheSourceValidationErrors(errors);
    } else {
      setCacheSourceValidationErrors([]);
    }
  };

  // Add a validation function for custom cache sources
  const getValidCustomCacheSources = () => {
    if (!specifyCacheSources) return [];
    return customCacheSources
        .split('\n')
        .map(source => source.trim())
        .filter(source => source.length > 0 && CACHE_SOURCE_REGEX.test(source));
  };

  const isCustomCacheSourcesValid = () => {
    if (!specifyCacheSources) return true; // Not required if checkbox is unchecked
    const validSources = getValidCustomCacheSources();
    return validSources.length > 0 && cacheSourceValidationErrors.length === 0;
  };

  const handleSubmit = useCallback(
      async (e: FormEvent) => {
        e.preventDefault();

        if (pw !== pw2) {
          setError("Passwords do not match");
          return false;
        }

        setTimeout(async () => {
          setLoading(true);

          // Process custom cache sources if specified
          let cacheSourcesToUse: string[] | undefined = undefined;
          if (specifyCacheSources && customCacheSources.trim()) {
            cacheSourcesToUse = getValidCustomCacheSources();
            console.log("Custom cache sources:", cacheSourcesToUse);
          }

          // Process RPC providers - convert to JSON strings
          let baseL2AccessProvidersToUse: string[] | undefined = undefined;
          if (specifyBaseL2AccessProviders && rpcProviders.length > 0) {
            baseL2AccessProvidersToUse = rpcProviders.map(provider => {
              const authObj: Record<string, string> | null = provider.auth ? {
                [provider.auth.type]: provider.auth.value
              } : null;

              return JSON.stringify({
                url: provider.url,
                auth: authObj
              });
            });

            console.log("Custom Base L2 access providers:", baseL2AccessProvidersToUse);
          }

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
                  custom_routers: routers && routers.length > 0 ? routers : null,
                  custom_cache_sources: cacheSourcesToUse && cacheSourcesToUse.length > 0 ? cacheSourcesToUse : null,
                  custom_base_l2_access_providers: baseL2AccessProvidersToUse && baseL2AccessProvidersToUse.length > 0 ? baseL2AccessProvidersToUse : null,
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
      [direct, pw, pw2, reset, hnsName, routers, specifyCacheSources, customCacheSources, specifyBaseL2AccessProviders, rpcProviders, address, chainId, signTypedDataAsync]
  );

  // Validation for the submit button
  const hasInvalidRpcProviders = specifyBaseL2AccessProviders && (
      rpcProviders.length === 0 ||
      rpcProviders.some(p => !p.url.trim() || !validateWebSocketUrl(p.url) || (p.auth && !p.auth.value.trim()))
  );

  return (
      <>
        {loading ? (
            <Loader msg="Please sign the structured message in your wallet to set your password." />
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

              {/* Advanced Options Section */}
              <details className="advanced-options">
                <summary>Advanced Options</summary>
                <div className="flex flex-col gap-3">
                  <SpecifyCacheSourcesCheckbox
                      specifyCacheSources={specifyCacheSources}
                      setSpecifyCacheSources={handleSetSpecifyCacheSources}
                      initiallyChecked={initialCacheSourcesChecked}
                  />
                  {specifyCacheSources && (
                      <div className="flex flex-col gap-2 ml-6">
                        <label htmlFor="custom-cache-sources" className="text-sm font-medium">
                          Cache Source Names: <span className="text-red-500">*</span>
                        </label>
                        <textarea
                            id="custom-cache-sources-setpassword"
                            value={customCacheSources}
                            onChange={(e) => handleCustomCacheSourcesChange(e.target.value)}
                            placeholder="Enter one cache source name per line, e.g.:&#10;cache-node-1.hypr&#10;other-cache.hypr&#10;mycache.os"
                            className={`input resize-vertical min-h-[80px] ${
                                specifyCacheSources && !isCustomCacheSourcesValid()
                                    ? 'border-red-500 focus:border-red-500'
                                    : ''
                            }`}
                            rows={4}
                        />
                        {cacheSourceValidationErrors.length > 0 ? (
                            <div className="text-xs text-red-500">
                              {cacheSourceValidationErrors.map((error, idx) => (
                                  <div key={idx}>{error}</div>
                              ))}
                              <div className="mt-1">Cache source names must contain only lowercase letters, numbers, hyphens (not at start/end), and dots.</div>
                            </div>
                        ) : (
                            <span className={`text-xs ${
                                !isCustomCacheSourcesValid() ? 'text-red-500' : 'text-gray-500'
                            }`}>
                              {!isCustomCacheSourcesValid()
                                  ? 'At least one valid cache source name is required'
                                  : 'Enter one cache source name per line. These nodes will serve as cache sources for hypermap data.'
                              }
                            </span>
                        )}
                      </div>
                  )}

                  <SpecifyBaseL2AccessProvidersCheckbox
                      specifyBaseL2AccessProviders={specifyBaseL2AccessProviders}
                      setSpecifyBaseL2AccessProviders={setSpecifyBaseL2AccessProviders}
                      initiallyChecked={initialBaseL2ProvidersChecked}
                  />
                  {specifyBaseL2AccessProviders && (
                      <div className="ml-6">
                        <RpcProviderEditor
                            providers={rpcProviders}
                            onChange={setRpcProviders}
                            label="Base L2 RPC Providers"
                        />
                      </div>
                  )}
                </div>
              </details>

              {Boolean(error) && <p className="text-red-500 wrap-anywhere mt-2">{error}</p>}
              <button
                  type="submit"
                  className="button"
                  disabled={specifyCacheSources && !isCustomCacheSourcesValid() || hasInvalidRpcProviders}
              >
                Submit
              </button>
            </form>
        )}
      </>
  );
}

export default SetPassword;