import {
  FormEvent,
  useCallback,
  useEffect,
  useState,
} from "react";
import { PageProps } from "../lib/types";
import Loader from "../components/Loader";
import { redirectToHomepage } from "../utils/redirect-to-homepage";
import BackButton from "../components/BackButton";
interface ImportKeyfileProps extends PageProps { }

function ImportKeyfile({
  pw,
  setPw,
}: ImportKeyfileProps) {

  const [localKey, setLocalKey] = useState<Uint8Array | null>(null);
  const [localKeyFileName, setLocalKeyFileName] = useState<string>("");
  const [keyErrs, setKeyErrs] = useState<string[]>([]);

  const [pwErr, setPwErr] = useState<string[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [hnsName, setHnsName] = useState<string>("");

  useEffect(() => {
    document.title = "Import Keyfile";
  }, []);

  // for if we check router validity in future
  // const KEY_BAD_ROUTERS = "Routers from records are offline"

  const handleKeyfile = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    e.preventDefault();
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onloadend = () => {
      if (reader.result instanceof ArrayBuffer) {
        setLocalKey(new Uint8Array(reader.result));
        setLocalKeyFileName(file.name);
      }
    };
    reader.readAsArrayBuffer(file);
  }, []);

  const handleImportKeyfile = useCallback(
    async (e: FormEvent) => {
      e.preventDefault();
      e.stopPropagation();

      setLoading(true);

      try {
        if (keyErrs.length === 0 && localKey !== null) {
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
            const hashed_password_hex = `0x${h.hashHex}`;

            const result = await fetch("/import-keyfile", {
              method: "POST",
              credentials: 'include',
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({
                keyfile: Buffer.from(localKey).toString('utf8'),
                password_hash: hashed_password_hex,
              }),
            });

            if (result.status > 399) {
              throw new Error("Incorrect password");
            }
            redirectToHomepage();
          }).catch((err: any) => {
            window.alert(String(err));
            setLoading(false);
          });
        }
      } catch (err) {
        window.alert(String(err));
        setLoading(false);
      }
    },
    [localKey, pw, keyErrs]
  );

  const onHnsNameChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setHnsName(e.target.value);
    const errs = [];
    if (e.target.value.length < 8) {
      errs.push("Node ID must be at least 8 characters");
    }
    if (!localKeyFileName) {
      errs.push("No keyfile selected");
    }
    setKeyErrs(errs);
  }, []);

  const onPwChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setPw(e.target.value);
    const errs = [];
    if (e.target.value.length < 6) {
      errs.push("Password must be at least 6 characters");
    }
    setPwErr(errs);
  }, []);

  return (
    <div className="container fade-in">
      <div className="section">
        {loading ? (
          <Loader msg="Setting up node..." />
        ) : (
          <>
            <form className="form flex flex-col gap-2" onSubmit={handleImportKeyfile}>
              <div className="form-group">
                <h4 className="form-label">
                  <span>1. Upload Keyfile</span>
                </h4>
                <label className="file-input-label">
                  <input
                    type="file"
                    className="file-input"
                    onChange={handleKeyfile}
                  />
                  <span className="button secondary">
                    {localKeyFileName ? "Change Keyfile" : "Select Keyfile"}
                  </span>
                </label>
                {localKeyFileName && <p className="mt-2">{localKeyFileName}</p>}
              </div>
              <div className="form-group">
                <h4 className="form-label">2. Enter Node ID</h4>
                <input
                  type="text"
                  className="name-input"
                  onChange={onHnsNameChange}
                />
              </div>
              <div className="form-group">
                <h4 className="form-label">3. Enter Password</h4>
                <input
                  type="password"
                  id="password"
                  required
                  minLength={6}
                  name="password"
                  placeholder=""
                  value={pw}
                  onChange={onPwChange}
                />
                {pwErr.length > 0 && <p className="text-red-500 wrap-anywhere mt-2">{pwErr.join(", ")}</p>}
              </div>

              <div className="form-group">
                {keyErrs.map((x, i) => (
                  <p key={i} className="text-red-500 wrap-anywhere mt-2">{x}</p>
                ))}
                <button
                  disabled={keyErrs.length !== 0 || !localKey || !hnsName || !pw || pw.length < 6}
                  type="submit"
                  className="button">
                  Boot Node</button>
                <BackButton mode="wide" />
              </div>
              <p className="text-sm">
                Please note: if the original node was booted as a direct node
                (static IP), then you must run this node from the same IP. If not,
                you will have networking issues. If you need to change the network
                options, please go back and select "Reset OsName".
              </p>
            </form>
          </>
        )}
      </div>
    </div>
  );
}

export default ImportKeyfile;
