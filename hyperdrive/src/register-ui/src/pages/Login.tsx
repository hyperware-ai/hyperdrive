import { FormEvent, useCallback, useEffect, useState } from "react";
import { PageProps, UnencryptedIdentity } from "../lib/types";
import Loader from "../components/Loader";
import { useNavigate } from "react-router-dom";
import { redirectToHomepage } from "../utils/redirect-to-homepage";
import classNames from "classnames";

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
            body: JSON.stringify({ password_hash: hashed_password_hex }),
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
    [pw]
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

      {keyErrs.length > 0 && (
        <div className="flex flex-col gap-2">
          {keyErrs.map((x, i) => (
            <div key={i} className="text-red-500 wrap-anywhere mt-2">{x}</div>
          ))}
        </div>
      )}

      <button type="submit">Log in</button>

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
