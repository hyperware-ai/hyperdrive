import { FormEvent, useCallback, useEffect, useState } from "react";
import { PageProps, InfoResponse } from "../lib/types";
import Loader from "../components/Loader";
import { useNavigate } from "react-router-dom";
import { redirectToHomepage } from "../utils/redirect-to-homepage";
import classNames from "classnames";
import SpecifyCacheSourcesCheckbox from "../components/SpecifyCacheSourcesCheckbox";
import SpecifyBaseL2AccessProvidersCheckbox from "../components/SpecifyBaseL2AccessProvidersCheckbox";
import { RpcProviderEditor, RpcProviderData } from "../components/RpcProviderEditor";

interface LoginProps extends PageProps { }

// Regex for valid cache source names (domain format)
const CACHE_SOURCE_REGEX = /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)*$/;

// Validate that URL is a secure WebSocket URL
const validateWebSocketUrl = (url: string): boolean => {
  if (!url.trim()) return false;
  return url.startsWith('wss://');
};

function Login({
                 pw,
                 setPw,
                 routers,
                 setRouters,
                 hnsName,
                 setHnsName,
               }: LoginProps) {
  const navigate = useNavigate();

  useEffect(() => {
    if (!hnsName) navigate('/');
  }, [hnsName]);

  // Advanced options state - cache sources and Base L2 access providers
  const [specifyCacheSources, setSpecifyCacheSources] = useState(false);
  const [customCacheSources, setCustomCacheSources] = useState('');
  const [cacheSourceValidationErrors, setCacheSourceValidationErrors] = useState<string[]>([]);
  const [specifyBaseL2AccessProviders, setSpecifyBaseL2AccessProviders] = useState(false);
  const [rpcProviders, setRpcProviders] = useState<RpcProviderData[]>([]);

  const [keyErrs, setKeyErrs] = useState<string[]>([]);
  const [loading, setLoading] = useState<string>("");

  useEffect(() => {
    document.title = "Login";

    (async () => {
      try {
        const infoData = (await fetch("/info", { method: "GET", credentials: 'include' }).then((res) =>
            res.json()
        )) as InfoResponse;
        setRouters(infoData.allowed_routers);
        setHnsName(infoData.name);

        // Prepopulate cache sources
        if (infoData.initial_cache_sources && infoData.initial_cache_sources.length > 0) {
          setCustomCacheSources(infoData.initial_cache_sources.join('\n'));
          setSpecifyCacheSources(true);
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
        }
      } catch (error) {
        console.error('Failed to fetch node info:', error);
      }
    })();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

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

  const handleLogin = useCallback(
      async (e?: FormEvent) => {
        e?.preventDefault();
        e?.stopPropagation();

        setLoading("Logging in...");
        try {
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

          let result;

          try {
            // Try argon2 hash first
            const minSaltL = 8;
            const nodeL = hnsName.length;
            const salt = nodeL >= minSaltL ? hnsName : hnsName.repeat(1 + Math.floor(minSaltL / nodeL));

            //@ts-ignore
            const h = await argon2.hash({
              pass: pw,
              salt: salt,
              hashLen: 32,
              time: 2,
              mem: 19456,
              //@ts-ignore
              type: argon2.ArgonType.Argon2id
            });

            const hashed_password_hex = `0x${h.hashHex}`;

            result = await fetch("/login", {
              method: "POST",
              credentials: 'include',
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                password_hash: hashed_password_hex,
                custom_cache_sources: cacheSourcesToUse && cacheSourcesToUse.length > 0 ? cacheSourcesToUse : null,
                custom_base_l2_access_providers: baseL2AccessProvidersToUse && baseL2AccessProvidersToUse.length > 0 ? baseL2AccessProvidersToUse : null,
              }),
            });

            if (result.status < 399) {
              redirectToHomepage();
              return;
            }
          } catch (argonErr) {
            console.log("This node was instantiated before the switch to argon2");
          }

          throw new Error(result ? await result.text() : "Login failed");

        } catch (err) {
          setKeyErrs([String(err)]);
          setLoading("");
        }
      },
      [pw, hnsName, specifyCacheSources, customCacheSources, specifyBaseL2AccessProviders, rpcProviders]
  );

  const isDirect = Boolean(routers?.length === 0);

  // Validation for the submit button
  const hasInvalidRpcProviders = specifyBaseL2AccessProviders && (
      rpcProviders.length === 0 ||
      rpcProviders.some(p => !p.url.trim() || !validateWebSocketUrl(p.url) || (p.auth && !p.auth.value.trim()))
  );

  return <div className="relative flex flex-col gap-2 items-stretch self-stretch">
    {loading && <div className="absolute top-0 left-0 w-full h-full flex place-content-center place-items-center">
      <Loader msg={loading} className="text-black dark:text-white" />
    </div>}
    <form
        id="registerui--login-form"
        className={classNames("flex flex-col gap-2 items-stretch", {
          'invisible': loading
        })}
        onSubmit={handleLogin}
    >

      <div className="form-group">
        <div className="form-header">
          <h3 className="text-iris dark:text-neon font-bold">{hnsName}</h3>
          <div className="text-xs opacity-50">Login - {isDirect ? "direct" : "indirect"} node</div>
        </div>
        <input
            type="password"
            id="password"
            required
            minLength={6}
            name="password"
            placeholder="Password"
            value={pw}
            onChange={(e) => setPw(e.target.value)}
            autoFocus
        />
      </div>

      {/* Advanced Options Section */}
      <details className="advanced-options">
        <summary>Advanced Options</summary>
        <div className="flex flex-col gap-3">
          <SpecifyCacheSourcesCheckbox
              specifyCacheSources={specifyCacheSources}
              setSpecifyCacheSources={handleSetSpecifyCacheSources}
          />
          {specifyCacheSources && (
              <div className="flex flex-col gap-2 ml-6">
                <label htmlFor="custom-cache-sources" className="text-sm font-medium">
                  Cache Source Names: <span className="text-red-500">*</span>
                </label>
                <textarea
                    id="custom-cache-sources-login"
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
          />
          {specifyBaseL2AccessProviders && (
              <div className="ml-6">
                <RpcProviderEditor
                    providers={rpcProviders}
                    onChange={setRpcProviders}
                    label="Base L2 RPC Providers"
                />
                {hasInvalidRpcProviders && (
                    <div className="text-xs text-red-500 mt-2">
                      All RPC provider URLs must be secure WebSocket URLs starting with wss://
                    </div>
                )}
              </div>
          )}
        </div>
      </details>

      {keyErrs.length > 0 && (
          <div className="flex flex-col gap-2">
            {keyErrs.map((x, i) => (
                <div key={i} className="text-red-500 wrap-anywhere mt-2">{x}</div>
            ))}
          </div>
      )}

      <button
          type="submit"
          disabled={specifyCacheSources && !isCustomCacheSourcesValid() || hasInvalidRpcProviders}
      >Log in</button>

      <button
          className="clear "
          type="button"
          onClick={() => navigate('/reset')}
      >
        Reset Password & Networking Info
      </button>
    </form>
  </div>;
}

export default Login;