import { useState, useEffect, FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { PageProps } from "../lib/types";
import BackButton from "../components/BackButton";
import DirectNodeCheckbox from "../components/DirectCheckbox";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

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
    const [useDebugMode, setUseDebugMode] = useState(false);

    useEffect(() => {
        document.title = "Password Setup Only"
    }, [])

    const handleSubmit = (e: FormEvent) => {
        e.preventDefault();
        e.stopPropagation();

        // Set the global state with the form values
        setHnsName(inputHnsName);
        setDirect(inputDirect);

        // Navigate to password setup page (debug or normal)
        navigate(useDebugMode ? "/set-password-debug" : "/set-password");
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
                        <summary>Advanced Options</summary>
                        <DirectNodeCheckbox direct={inputDirect} setDirect={setInputDirect} />
                        <div style={{ marginTop: '10px', display: 'flex', alignItems: 'center', gap: '10px' }}>
                            <button
                                className="icon"
                                type="button"
                                onClick={(e) => {
                                    e.preventDefault();
                                    e.stopPropagation();
                                    setUseDebugMode(!useDebugMode);
                                }}
                            >
                                {useDebugMode ? <FaSquareCheck /> : <FaRegSquare />}
                            </button>
                            <span>Use debug mode (for Gnosis Safe issues)</span>
                        </div>
                    </details>
                    <div className="button-group">
                        <button type="submit" className="button">
                            Continue to Password Setup
                        </button>
                    </div>

                    {/* CLI Alternative Info */}
                    <div style={{
                        marginTop: '20px',
                        padding: '15px',
                        background: '#f5f5f5',
                        borderRadius: '8px',
                        fontSize: '14px'
                    }}>
                        <strong>Alternative: CLI Script</strong>
                        <p style={{ margin: '10px 0 5px 0', fontSize: '13px' }}>
                            If you're having issues with wallet signing (especially with Gnosis Safe), you can use the CLI script:
                        </p>
                        <code style={{
                            display: 'block',
                            padding: '10px',
                            background: '#fff',
                            borderRadius: '4px',
                            fontSize: '12px',
                            overflowX: 'auto'
                        }}>
                            node password-only-setup.js {inputHnsName || '<hnsName>'} {'<password>'} {'<privateKey>'} {'<rpcUrl>'} {inputDirect ? '--direct' : ''} --output keyfile.json
                        </code>
                    </div>
                </form>
            </div>
        </div>
    );
}

export default PasswordOnlySetup;
