import React, { useState, useEffect, useCallback } from 'react';
import useAppsStore from "../store/appStoreStore";

interface MirrorSelectorProps {
    packageId: string | undefined;
    onMirrorSelect: (mirror: string, status: boolean | null | 'http') => void;
    onError?: (error: string) => void;
}

type MirrorStatus = boolean | null | 'http';
type MirrorStatuses = { [mirror: string]: MirrorStatus };

const MirrorSelector: React.FC<MirrorSelectorProps> = ({ packageId, onMirrorSelect, onError }) => {
    const { fetchListing, checkMirror } = useAppsStore();
    const [mirrorStatuses, setMirrorStatuses] = useState<MirrorStatuses>({});
    const [mirrors, setMirrors] = useState<string[]>([]);
    const [selectedMirror, setSelectedMirror] = useState("");
    const [customMirror, setCustomMirror] = useState("");
    const [isCustomMirrorSelected, setIsCustomMirrorSelected] = useState(false);
    const [checkingMirror, setCheckingMirror] = useState<string | null>(null);

    const checkSingleMirror = useCallback(async (mirror: string): Promise<MirrorStatus> => {
        if (mirror.startsWith('http')) return 'http';
        setCheckingMirror(mirror);
        try {
            const status = await checkMirror(packageId!, mirror);
            return status?.is_online ?? false;
        } catch {
            return false;
        } finally {
            setCheckingMirror(null);
        }
    }, [packageId, checkMirror]);

    useEffect(() => {
        if (!packageId) return;

        fetchListing(packageId).then(appData => {
            if (!appData) {
                onError?.("Failed to fetch app data");
                return;
            }

            const mirrorList = Array.from(new Set([
                appData.package_id.publisher_node,
                ...(appData.metadata?.properties?.mirrors || [])
            ]));

            setMirrors(mirrorList);

            if (mirrorList.length > 0) {
                setSelectedMirror(mirrorList[0]);
                onMirrorSelect(mirrorList[0], null);
            }

            const statuses: MirrorStatuses = {};
            mirrorList.forEach(m => {
                statuses[m] = m.startsWith('http') ? 'http' : null;
            });
            setMirrorStatuses(statuses);
        }).catch(() => onError?.("Failed to load mirrors"));
    }, [packageId, fetchListing, onError]);

    const handleMirrorChange = async (e: React.ChangeEvent<HTMLSelectElement>) => {
        const value = e.target.value;
        if (value === "custom") {
            setIsCustomMirrorSelected(true);
            setSelectedMirror("");
            onMirrorSelect("", null);
            return;
        }

        setIsCustomMirrorSelected(false);
        setCustomMirror("");
        setSelectedMirror(value);

        if (mirrorStatuses[value] === null) {
            const status = await checkSingleMirror(value);
            setMirrorStatuses(prev => ({ ...prev, [value]: status }));
            onMirrorSelect(value, status);
        } else {
            onMirrorSelect(value, mirrorStatuses[value]);
        }
    };

    const handleSetCustomMirror = async () => {
        if (!customMirror) return;

        const status = await checkSingleMirror(customMirror);
        setMirrorStatuses(prev => ({ ...prev, [customMirror]: status }));
        setSelectedMirror(customMirror);
        setMirrors(prev => Array.from(new Set([...prev, customMirror])));
        onMirrorSelect(customMirror, status);
        setIsCustomMirrorSelected(false);
    };

    const formatMirrorOption = (mirror: string) => {
        const status = checkingMirror === mirror ? 'checking...' :
            mirrorStatuses[mirror] === 'http' ? 'HTTP' :
                mirrorStatuses[mirror] === true ? 'online' :
                    mirrorStatuses[mirror] === false ? 'offline' : '';
        return `${mirror}${status ? ` (${status})` : ''}`;
    };

    return (
        <div className="flex flex-col gap-2 self-stretch items-stretch">
            <select
                value={selectedMirror || (isCustomMirrorSelected ? "custom" : "")}
                onChange={handleMirrorChange}
                className="bg-black/10 dark:bg-white/10 text-gray-500 self-stretch p-2 rounded-lg text-sm md:text-base"
                onFocus={() => mirrors.forEach(m => {
                    if (mirrorStatuses[m] === null) {
                        checkSingleMirror(m).then(status =>
                            setMirrorStatuses(prev => ({ ...prev, [m]: status }))
                        );
                    }
                })}
            >
                <option value="">Select a mirror</option>
                {mirrors.map(mirror => (
                    <option
                        key={mirror}
                        value={mirror}
                        className={mirrorStatuses[mirror] === false ? 'text-gray-400' : ''}
                    >
                        {formatMirrorOption(mirror)}
                    </option>
                ))}
                <option value="custom">Custom mirror</option>
            </select>

            {isCustomMirrorSelected && (
                <div className="flex gap-2 items-center">
                    <input
                        type="text"
                        value={customMirror}
                        onChange={(e) => setCustomMirror(e.target.value)}
                        placeholder="Enter custom mirror URL or node ID"
                        className="grow"
                    />
                    <button
                        onClick={handleSetCustomMirror}
                        disabled={!customMirror}
                        className="clear thin"
                    >
                        Add
                    </button>
                </div>
            )}
        </div>
    );
};

export default MirrorSelector;