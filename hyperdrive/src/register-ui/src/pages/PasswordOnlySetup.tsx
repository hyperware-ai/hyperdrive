import { useState, useEffect, FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { PageProps } from "../lib/types";
import BackButton from "../components/BackButton";

interface PasswordOnlySetupProps extends PageProps { }

function PasswordOnlySetup({
    hnsName,
    setHnsName,
    direct,
    setDirect,
    reset,
}: PasswordOnlySetupProps) {
    const navigate = useNavigate();
    const [inputHnsName, setInputHnsName] = useState("");
    const [inputDirect, setInputDirect] = useState(false);

    useEffect(() => {
        document.title = "Password Setup Only"
    }, [])

    const handleSubmit = (e: FormEvent) => {
        e.preventDefault();
        e.stopPropagation();

        // Set the global state with the form values
        setHnsName(inputHnsName);
        setDirect(inputDirect);

        // Navigate directly to password setup page
        navigate("/set-password");
    }

    return (
        <div className="container fade-in">
            <div className="section">
                <form className="form" onSubmit={handleSubmit}>
                    <p className="form-label">
                        <BackButton />
                        <span>
                            Set up password for an already-minted NFT with networking keys already stored onchain
                        </span>
                    </p>
                    <input
                        type="text"
                        value={inputHnsName}
                        onChange={(e) => setInputHnsName(e.target.value)}
                        placeholder="Enter full HNS name (e.g., myname.os)"
                        required
                    />
                    <details>
                        <summary>Node Configuration</summary>
                        <label className="checkbox-container">
                            <input
                                type="checkbox"
                                checked={inputDirect}
                                onChange={(e) => setInputDirect(e.target.checked)}
                            />
                            <span>Direct node (was this minted as a direct node?)</span>
                        </label>
                    </details>
                    <div className="button-group">
                        <button type="submit" className="button">
                            Continue to Password Setup
                        </button>
                    </div>
                </form>
            </div>
        </div>
    );
}

export default PasswordOnlySetup;
