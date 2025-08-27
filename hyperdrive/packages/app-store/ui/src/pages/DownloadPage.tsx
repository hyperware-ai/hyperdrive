import React, { useState, useEffect, useCallback, useMemo } from "react";
import { useParams } from "react-router-dom";
import { FaDownload, FaSpinner, FaChevronDown, FaChevronUp, FaRocket, FaTrash, FaPlay } from "react-icons/fa";
import useAppsStore from "../store/appStoreStore";
import { MirrorSelector, ManifestDisplay } from '../components';
import { ManifestResponse } from "../types/Apps";
import { Modal } from "../components/Modal";
import { FaChevronRight } from "react-icons/fa6";
import { BsX } from "react-icons/bs";

export default function DownloadPage() {
    const { id } = useParams<{ id: string }>();
    const {
        listings,
        downloads,
        installed,
        activeDownloads,
        fetchData,
        downloadApp,
        installApp,
        removeDownload,
        clearAllActiveDownloads,
        fetchHomepageApps,
        getLaunchUrl,
        homepageApps,
    } = useAppsStore();

    const [showMetadata, setShowMetadata] = useState(false);
    const [selectedMirror, setSelectedMirror] = useState<string>("");
    const [selectedVersion, setSelectedVersion] = useState<string>("");
    const [showMyDownloads, setShowMyDownloads] = useState(false);
    const [isMirrorOnline, setIsMirrorOnline] = useState<boolean | null>(null);
    const [showCapApproval, setShowCapApproval] = useState(false);
    const [manifestResponse, setManifestResponse] = useState<ManifestResponse | null>(null);
    const [isInstalling, setIsInstalling] = useState(false);
    const [isPolling, setIsPolling] = useState(false);

    const app = useMemo(() => listings[id || ""], [listings, id]);
    const appDownloads = useMemo(() => downloads[id || ""] || [], [downloads, id]);
    const installedApp = useMemo(() => installed[id || ""], [installed, id]);

    useEffect(() => {
        if (id) {
            fetchData(id);
            clearAllActiveDownloads();
            fetchHomepageApps();
        }
    }, [id, fetchData, clearAllActiveDownloads, fetchHomepageApps]);

    const handleMirrorSelect = useCallback((mirror: string, status: boolean | null | 'http') => {
        setSelectedMirror(mirror);
        setIsMirrorOnline(status === 'http' ? true : status);
    }, []);

    const sortedVersions = useMemo(() => {
        if (!app || !app.metadata?.properties?.code_hashes) return [];
        return app.metadata.properties.code_hashes
            .map(([version, hash]) => ({ version, hash }))
            .sort((a, b) => {
                const vA = a.version.split('.').map(Number);
                const vB = b.version.split('.').map(Number);
                for (let i = 0; i < Math.max(vA.length, vB.length); i++) {
                    if (vA[i] > vB[i]) return -1;
                    if (vA[i] < vB[i]) return 1;
                }
                return 0;
            });
    }, [app]);

    useEffect(() => {
        if (sortedVersions.length > 0 && !selectedVersion) {
            setSelectedVersion(sortedVersions[0].version);
        }
    }, [sortedVersions, selectedVersion]);

    useEffect(() => {
        if (isPolling) {
            const pollInterval = setInterval(() => {
                fetchHomepageApps();
            }, 1000);

            // Stop polling after 5 seconds
            const timeout = setTimeout(() => {
                setIsPolling(false);
            }, 5000);

            return () => {
                clearInterval(pollInterval);
                clearTimeout(timeout);
            };
        }
    }, [isPolling, fetchHomepageApps]);

    const isDownloaded = useMemo(() => {
        if (!app || !selectedVersion) return false;
        const versionData = sortedVersions.find(v => v.version === selectedVersion);
        if (!versionData) return false;
        return appDownloads.some(d => d.File && d.File.name === `${versionData.hash}.zip`);
    }, [app, selectedVersion, sortedVersions, appDownloads]);

    const isDownloading = useMemo(() => {
        if (!app || !selectedVersion) return false;
        const versionData = sortedVersions.find(v => v.version === selectedVersion);
        if (!versionData) return false;
        // Check for any active download for this app, not just the selected version
        return Object.keys(activeDownloads).some(key => key.startsWith(`${app.package_id.package_name}:`));
    }, [app, selectedVersion, sortedVersions, activeDownloads]);

    const downloadProgress = useMemo(() => {
        if (!isDownloading || !app || !selectedVersion) return null;
        const versionData = sortedVersions.find(v => v.version === selectedVersion);
        if (!versionData) return null;
        // Find the active download for this app
        const activeDownloadKey = Object.keys(activeDownloads).find(key =>
            key.startsWith(`${app.package_id.package_name}:`)
        );
        if (!activeDownloadKey) return null;
        const progress = activeDownloads[activeDownloadKey];
        return progress ? Math.round((progress.downloaded / progress.total) * 100) : 0;
    }, [isDownloading, app, selectedVersion, sortedVersions, activeDownloads]);

    const isCurrentVersionInstalled = useMemo(() => {
        if (!app || !selectedVersion || !installedApp) return false;
        const versionData = sortedVersions.find(v => v.version === selectedVersion);
        return versionData ? installedApp.our_version_hash === versionData.hash : false;
    }, [app, selectedVersion, installedApp, sortedVersions]);

    const canLaunch = useMemo(() => {
        if (!app) return false;
        return !!getLaunchUrl(`${app.package_id.package_name}:${app.package_id.publisher_node}`);
    }, [app, getLaunchUrl, homepageApps]);

    const handleLaunch = useCallback(() => {
        if (app) {
            const launchUrl = getLaunchUrl(`${app.package_id.package_name}:${app.package_id.publisher_node}`);
            if (launchUrl) {
                window.location.href = window.location.origin.replace('//app-store-sys.', '//') + launchUrl;
            }
        }
    }, [app, getLaunchUrl]);

    const handleDownload = useCallback(() => {
        if (!id || !selectedMirror || !app || !selectedVersion) return;
        const versionData = sortedVersions.find(v => v.version === selectedVersion);
        if (versionData) {
            downloadApp(id, versionData.hash, selectedMirror);
        }
    }, [id, selectedMirror, app, selectedVersion, sortedVersions, downloadApp]);

    const handleRemoveDownload = useCallback((hash: string) => {
        if (!id) return;
        removeDownload(id, hash).then(() => fetchData(id));
    }, [id, removeDownload, fetchData]);

    const handleInstall = useCallback((version: string, hash: string) => {
        if (!id || !app) return;
        const download = appDownloads.find(d => d.File && d.File.name === `${hash}.zip`);
        if (download?.File?.manifest) {
            try {
                const manifest_response: ManifestResponse = {
                    package_id: app.package_id,
                    version_hash: hash,
                    manifest: download.File.manifest
                };
                setManifestResponse(manifest_response);
                setShowCapApproval(true);
            } catch (error) {
                console.error('Failed to parse manifest:', error);
            }
        } else {
            console.error('Manifest not found for the selected version');
        }
    }, [id, app, appDownloads]);

    const confirmInstall = useCallback(() => {
        if (!id || !selectedVersion) return;
        const versionData = sortedVersions.find(v => v.version === selectedVersion);
        if (versionData) {
            setIsInstalling(true);
            installApp(id, versionData.hash).then(() => {
                setShowCapApproval(false);
                setManifestResponse(null);
                fetchData(id);
                fetchHomepageApps();
                setIsPolling(true);  // Start polling
                setIsInstalling(false);
            });
        }
    }, [id, selectedVersion, sortedVersions, installApp, fetchData, fetchHomepageApps]);

    const canDownload = useMemo(() => {
        return selectedMirror && (isMirrorOnline === true || selectedMirror.startsWith('http')) && !isDownloading && !isDownloaded;
    }, [selectedMirror, isMirrorOnline, isDownloading, isDownloaded]);

    const renderActionButton = () => {
        if (isCurrentVersionInstalled) {
            return (
                <button className="action-button installed-button" disabled>
                    <FaRocket /> Installed
                </button>
            );
        }

        if (isInstalling) {
            return (
                <button className="action-button installing-button" disabled>
                    <FaSpinner className="fa-spin" /> Installing...
                </button>
            );
        }

        if (isDownloaded) {
            return (
                <button
                    onClick={() => {
                        const versionData = sortedVersions.find(v => v.version === selectedVersion);
                        if (versionData) {
                            handleInstall(versionData.version, versionData.hash);
                        }
                    }}
                    className="action-button install-button"
                >
                    <FaRocket /> Install
                </button>
            );
        }

        return (
            <button
                onClick={handleDownload}
                disabled={!canDownload}
                className="action-button download-button"
            >
                {isDownloading ? (
                    <>
                        <FaSpinner className="fa-spin" /> Downloading... {downloadProgress}%
                    </>
                ) : (
                    <>
                        <FaDownload /> Download
                    </>
                )}
            </button>
        );
    };

    if (!app) {
        return <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-stretch gap-4">
            <h4 className="prose">Loading app details...</h4>
        </div>;
    }

    return (
        <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-stretch gap-4">
            <div className="flex items-center justify-between gap-2 flex-wrap">
                <div className=" flex flex-col gap-2">
                    {app.metadata?.image && (
                        <img src={app.metadata.image} alt={app.metadata?.name || app.package_id.package_name}
                            className="w-24 h-24 object-cover rounded-lg"
                        />
                    )}
                    <div className=" flex flex-col gap-2">
                        <h2 className="prose">{app.metadata?.name || app.package_id.package_name}</h2>
                        <p className="app-id">{`${app.package_id.package_name}.${app.package_id.publisher_node}`}</p>
                    </div>
                </div>
                {canLaunch ? (
                    <button
                        onClick={handleLaunch}
                        className="launch-button"
                    >
                        <FaPlay /> Launch
                    </button>
                ) : (isInstalling || isPolling) ? (
                    <button className="launch-button" disabled>
                        <FaSpinner className="fa-spin" /> {isInstalling ? 'Installing...' : 'Checking...'}
                    </button>
                ) : null}
            </div>
            <p className="app-description">{app.metadata?.description}</p>

            <div className="download-section flex flex-col md:flex-row gap-2 items-stretch">
                <select
                    value={selectedVersion}
                    onChange={(e) => setSelectedVersion(e.target.value)}
                    className="version-selector self-stretch px-2 py-1 border border-black/10 dark:border-white/10 rounded-md"
                >
                    <option value="">Select version</option>
                    {sortedVersions.map((version) => (
                        <option key={version.version} value={version.version}>
                            {version.version}
                        </option>
                    ))}
                </select>

                <MirrorSelector
                    packageId={id}
                    onMirrorSelect={handleMirrorSelect}
                />

                {renderActionButton()}
            </div>

            <div className="my-downloads flex flex-col gap-2">
                <button
                    className="clear text-lg"
                    onClick={() => setShowMyDownloads(!showMyDownloads)}
                >
                    {showMyDownloads ? <FaChevronDown /> : <FaChevronRight />} <span>My Downloads</span>
                </button>
                {showMyDownloads && (
                    <div className="grid grid-cols-2 gap-2">
                        <div className="font-bold">
                            Version
                        </div>
                        <div className="font-bold">
                            Actions
                        </div>

                        {appDownloads.map((download) => {
                            const fileName = download.File?.name;
                            const hash = fileName ? fileName.replace('.zip', '') : '';
                            const versionData = sortedVersions.find(v => v.hash === hash);
                            if (!versionData) return null;
                            return <div key={hash} className="flex items-center gap-2 col-span-2 self-stretch">
                                <div className="grow">{versionData.version}</div>
                                <div className="flex gap-2 self-stretch grow">
                                    <button
                                        className="clear thin"
                                        onClick={() => handleInstall(versionData.version, hash)}
                                    >
                                        <FaRocket /> <span>Install</span>
                                    </button>
                                    <button
                                        className="clear thin"
                                        onClick={() => handleRemoveDownload(hash)}
                                    >
                                        <BsX /> <span>Delete</span>
                                    </button>
                                </div>
                            </div>
                        })}
                    </div>
                )}
            </div>

            {showCapApproval && manifestResponse && (<Modal onClose={() => setShowCapApproval(false)}>

                <h3 className="prose">Approve Capabilities</h3>
                <ManifestDisplay manifestResponse={manifestResponse} />
                <div className="flex gap-2 flex-col md:flex-row">
                    <button
                        className="clear"
                        onClick={() => setShowCapApproval(false)}>Cancel</button>
                    <button

                        onClick={confirmInstall}>
                        Approve and Install
                    </button>
                </div>
            </Modal>
            )}

            <div className="flex flex-col gap-2">
                <h3 className="prose">App Details</h3>
                <button
                    className="clear"
                    onClick={() => setShowMetadata(!showMetadata)}>
                    {showMetadata ? <FaChevronDown /> : <FaChevronRight />} Metadata
                </button>
                {showMetadata && <Modal onClose={() => setShowMetadata(false)}>
                    <pre>{JSON.stringify(app.metadata, null, 2)}</pre>
                </Modal>}
            </div>
        </div>
    );
}