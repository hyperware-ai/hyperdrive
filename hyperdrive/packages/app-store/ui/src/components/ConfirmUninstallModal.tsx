import React, { useEffect, useState } from "react";
import { Modal } from "./Modal";
import { FaCircleNotch } from "react-icons/fa";

export default function ConfirmUninstallModal({
    onClose,
    onUninstall,
    appName,
}: {
        onClose: () => void,
        onUninstall: () => void,
        appName: string,
    }) {

        const [uninstallButtonEnabled, setUninstallButtonEnabled] = useState(false);

        useEffect(() => {
            setTimeout(() => {
                setUninstallButtonEnabled(true);
            }, 2000);
        }, []);

    return (
        <Modal onClose={onClose}>
            <h3
                className="prose">Confirm Uninstall</h3>
            <p>Are you sure you want to uninstall {appName}?</p>
            <div
                className="flex items-center flex-col md:flex-row gap-2"
            >
                <button
                    onClick={onClose}
                    className="md:grow md:self-center self-stretch clear"
                >Cancel</button>
                <button
                    onClick={onUninstall}
                    className="md:grow md:self-center self-stretch"
                    disabled={!uninstallButtonEnabled}
                >
                    {uninstallButtonEnabled ? "Uninstall" : <FaCircleNotch className="animate-spin" />}

                </button>
            </div>
        </Modal>
    );
}