import { useEffect } from "react"
import { useNavigate } from "react-router-dom"

type OsHomeProps = {
    hnsName: string
    nodeChainId: string
}

function HyperdriveHome({ hnsName }: OsHomeProps) {
    const navigate = useNavigate()
    const registerRedir = () => navigate('/commit-os-name')
    const resetRedir = () => navigate('/reset')
    const importKeyfileRedir = () => navigate('/import-keyfile')
    const loginRedir = () => navigate('/login')
    const customRegisterRedir = () => navigate('/custom-register')
    const customUpgradeRedir = () => navigate('/custom-upgrade')
    const previouslyBooted = Boolean(hnsName)

    useEffect(() => {
        document.title = "Welcome | Hyperdrive"
    }, [])

    return (
        <>
            <div className="container fade-in" id="register-ui--home">
                <div className="section">
                    <div className="flex flex-col items-stretch gap-2">
                        {previouslyBooted ? (
                            <div className="text-center">
                                <h2 className="mb-2">Welcome back!</h2>
                                <button onClick={loginRedir} className="button">Log in</button>
                            </div>
                        ) : (
                            <>
                                <h2 className="text-center">Welcome to Hyperdrive</h2>
                                <h4 className="text-center">New here? Register a node to get started:</h4>
                                <div className="flex flex-col gap-1">
                                    <button onClick={registerRedir} className="button">
                                        Register .os Name
                                    </button>
                                </div>
                                <h4 className="text-center">Other options</h4>
                                <div className="flex flex-col gap-1">
                                    <button onClick={importKeyfileRedir} className="button secondary">
                                        Import Keyfile
                                    </button>
                                    <button onClick={resetRedir} className="button secondary">
                                        Reset Existing Name
                                    </button>
                                    <button onClick={customRegisterRedir} className="button secondary">
                                        Register Non-.os Name (Advanced)
                                    </button>
                                    <button onClick={customUpgradeRedir} className="button secondary">
                                        Upgrade Hyper Account (Advanced)
                                    </button>
                                </div>
                            </>
                        )}
                    </div>
                </div>
            </div>
        </>
    )
}

export default HyperdriveHome
