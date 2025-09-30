import { FormEvent, useCallback, useEffect, useState } from "react";
import { PageProps, UnencryptedIdentity } from "../lib/types";
import Loader from "../components/Loader";
import { useNavigate } from "react-router-dom";
import { redirectToHomepage } from "../utils/redirect-to-homepage";
import classNames from "classnames";
import SpecifyCacheSourcesCheckbox from "../components/SpecifyCacheSourcesCheckbox";
import SpecifyBaseL2AccessProvidersCheckbox from "../components/SpecifyBaseL2AccessProvidersCheckbox";

interface LoginProps extends PageProps { }

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
  const [specifyBaseL2AccessProviders, setSpecifyBaseL2AccessProviders] = useState(false);
  const [customBaseL2AccessProviders, setCustomBaseL2AccessProviders] = useState('');

  const [keyErrs, setKeyErrs] = useState<string[]>([]);
  const [loading, setLoading] = useState<string>("");

  useEffect(() => {
    document.title = "Login";

    (async () => {
      try {
        const infoData = (await fetch("/info", { method: "GET", credentials: 'include' }).then((res) =>
          res.json()
        )) as UnencryptedIdentity;
        setRouters(infoData.allowed_routers);
        setHnsName(infoData.name);
      } catch { }
    })();
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleLogin = useCallback(
    async (e?: FormEvent) => {
      e?.preventDefault();
      e?.stopPropagation();

      setLoading("Logging in...");
      try {
        // Process custom cache sources if specified
        let cacheSourcesToUse: string[] | undefined = undefined;
        if (specifyCacheSources && customCacheSources.trim()) {
          cacheSourcesToUse = customCacheSources
              .split('\n')
              .map(source => source.trim())
              .filter(source => source.length > 0);

          console.log("Custom cache sources:", cacheSourcesToUse);
        }

        // Process custom Base L2 access providers if specified
        let baseL2AccessProvidersToUse: string[] | undefined = undefined;
        if (specifyBaseL2AccessProviders && customBaseL2AccessProviders.trim()) {
          baseL2AccessProvidersToUse = customBaseL2AccessProviders
              .split('\n')
              .map(provider => provider.trim())
              .filter(provider => provider.length > 0);

          console.log("Custom Base L2 access providers:", baseL2AccessProvidersToUse);
        }

        let result;

        try {
          // Try argon2 hash first

          // salt is either node name (if node name is longer than 8 characters)
          //  or node name repeated enough times to be longer than 8 characters
          const minSaltL = 8;
          const nodeL = hnsName.length;
          const salt = nodeL >= minSaltL ? hnsName : hnsName.repeat(1 + Math.floor(minSaltL / nodeL));
          console.log(salt);

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
            body: JSON.stringify({ password_hash: hashed_password_hex,
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
    [pw, hnsName, specifyCacheSources, customCacheSources, specifyBaseL2AccessProviders, customBaseL2AccessProviders]
  );

  const isDirect = Boolean(routers?.length === 0);

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
              setSpecifyCacheSources={setSpecifyCacheSources}
          />
          {specifyCacheSources && (
              <div className="flex flex-col gap-2 ml-6">
                <label htmlFor="custom-cache-sources" className="text-sm font-medium">
                  Cache Source Names: <span className="text-red-500">*</span>
                </label>
                <textarea
                    id="custom-cache-sources-login"
                    value={customCacheSources}
                    onChange={(e) => setCustomCacheSources(e.target.value)}
                    placeholder="Enter one cache source name per line, e.g.:&#10;cache-node-1.hypr&#10;other-cache.hypr&#10;mycache.os"
                    className={`input resize-vertical min-h-[80px] ${
                        specifyCacheSources && customCacheSources.split('\n').map(c => c.trim()).filter(c => c.length > 0).length === 0
                            ? 'border-red-500 focus:border-red-500'
                            : ''
                    }`}
                    rows={4}
                />
                <span className={`text-xs ${
                    customCacheSources.split('\n').map(c => c.trim()).filter(c => c.length > 0).length === 0
                        ? 'text-red-500'
                        : 'text-gray-500'
                }`}>
                    {customCacheSources.split('\n').map(c => c.trim()).filter(c => c.length > 0).length === 0
                        ? 'At least one cache source name is required'
                        : 'Enter one cache source name per line. These nodes will serve as cache sources for hypermap data.'
                    }
                  </span>
              </div>
          )}

          <SpecifyBaseL2AccessProvidersCheckbox
              specifyBaseL2AccessProviders={specifyBaseL2AccessProviders}
              setSpecifyBaseL2AccessProviders={setSpecifyBaseL2AccessProviders}
          />
          {specifyBaseL2AccessProviders && (
              <div className="flex flex-col gap-2 ml-6">
                <label htmlFor="custom-base-l2-providers" className="text-sm font-medium">
                  Base L2 Access Provider Names: <span className="text-red-500">*</span>
                </label>
                <textarea
                    id="custom-base-l2-providers-login"
                    value={customBaseL2AccessProviders}
                    onChange={(e) => setCustomBaseL2AccessProviders(e.target.value)}
                    placeholder="Enter one provider or URL per line, e.g.:&#10;base-provider-1.hypr&#10;wss://base-mainnet.infura.io/v3/your-key&#10;myprovider.os&#10;wss://rpc.example.com"
                    className={`input resize-vertical min-h-[80px] ${
                        specifyBaseL2AccessProviders && customBaseL2AccessProviders.split('\n').map(p => p.trim()).filter(p => p.length > 0).length === 0
                            ? 'border-red-500 focus:border-red-500'
                            : ''
                    }`}
                    rows={4}
                />
                <span className={`text-xs ${
                    customBaseL2AccessProviders.split('\n').map(p => p.trim()).filter(p => p.length > 0).length === 0
                        ? 'text-red-500'
                        : 'text-gray-500'
                }`}>
                    {customBaseL2AccessProviders.split('\n').map(p => p.trim()).filter(p => p.length > 0).length === 0
                        ? 'At least one Base L2 access provider name is required'
                        : 'Enter one provider name per line. These nodes will provide access to Base Layer 2 blockchain data.'
                    }
                  </span>
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
          disabled={
              (specifyCacheSources && customCacheSources.split('\n').map(c => c.trim()).filter(c => c.length > 0).length === 0) ||
              (specifyBaseL2AccessProviders && customBaseL2AccessProviders.split('\n').map(p => p.trim()).filter(p => p.length > 0).length === 0)
          }
      >Log in</button>

      <button
        className="clear "
        onClick={() => navigate('/reset')}
      >
        Reset Password & Networking Info
      </button>
    </form>
  </div>;
}

export default Login;
