import React, { useEffect, useState, useCallback, useMemo } from "react";
import { useParams } from "react-router-dom";
import { FaDownload, FaCheck, FaTimes, FaPlay, FaSpinner, FaTrash, FaSync, FaChevronDown, FaChevronUp } from "react-icons/fa";
import useAppsStore from "../store";
import { AppListing, PackageState, ManifestResponse } from "../types/Apps";
import { compareVersions } from "../utils/compareVersions";
import { MirrorSelector, ManifestDisplay } from '../components';

const MOCK_APP: AppListing = {
  package_id: {
    package_name: 'mock-app',
    publisher_node: 'mock-node'
  },
  metadata: {
    name: 'Mock App with an Unreasonably Long Name for Testing Wrapping, Obviously, why else would you have a name this long?',
    description: `This is a mock app. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page.`,
    image: 'https://via.placeholder.com/150',
    properties: {
      code_hashes: [['1.0.0', '1234567890']],
      package_name: 'mock-app',
      publisher: 'mock-node',
      current_version: '1.0.0',
      mirrors: ['https://mock-mirror.com'],
      screenshots: ['https://via.placeholder.com/150', 'https://via.placeholder.com/150', 'https://via.placeholder.com/150']
    }
  },
  tba: '0x0000000000000000000000000000000000000000',
  metadata_uri: 'https://mock-metadata.com',
  metadata_hash: '1234567890',
  auto_update: false
};

export default function AppPage() {
  const { id } = useParams();
  const {
    fetchListing,
    fetchInstalledApp,
    fetchDownloadsForApp,
    uninstallApp,
    setAutoUpdate,
    getLaunchUrl,
    fetchHomepageApps,
    downloadApp,
    downloads,
    activeDownloads,
    installApp,
    clearAllActiveDownloads,
    checkMirrors
  } = useAppsStore();

  const [app, setApp] = useState<AppListing | null>(null);
  const [installedApp, setInstalledApp] = useState<PackageState | null>(null);
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);
  const [latestVersion, setLatestVersion] = useState<string | null>(null);
  const [upToDate, setUpToDate] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);
  const [isUninstalling, setIsUninstalling] = useState(false);
  const [isTogglingAutoUpdate, setIsTogglingAutoUpdate] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [selectedMirror, setSelectedMirror] = useState<string>("");
  const [selectedVersion, setSelectedVersion] = useState<string>("");
  const [isMirrorOnline, setIsMirrorOnline] = useState<boolean | null>(null);
  const [showCapApproval, setShowCapApproval] = useState(false);
  const [manifestResponse, setManifestResponse] = useState<ManifestResponse | null>(null);
  const [canLaunch, setCanLaunch] = useState(false);
  const [attemptedDownload, setAttemptedDownload] = useState(false);
  const [mirrorError, setMirrorError] = useState<string | null>(null);

  const [isDevMode, setIsDevMode] = useState(false);
  const [backtickPressCount, setBacktickPressCount] = useState(0);

  useEffect(() => {
    window.addEventListener('keydown', (e) => {
      console.log('keydown', e.key);
      if (e.key === '`') {
        setBacktickPressCount(backtickPressCount + 1);
      }
      if (backtickPressCount >= 5) {
        setIsDevMode(!isDevMode);
        setBacktickPressCount(0);
      }
    });
  }, []);

  const appDownloads = useMemo(() => downloads[id || ""] || [], [downloads, id]);

  const sortedVersions = useMemo(() => {
    if (!app?.metadata?.properties?.code_hashes) return [];
    return app.metadata.properties.code_hashes
      .map(([version, hash]) => ({ version, hash }))
      .sort((a, b) => compareVersions(b.version, a.version));
  }, [app]);

  const isDownloaded = useMemo(() => {
    if (!app || !selectedVersion) return false;
    const versionData = sortedVersions.find(v => v.version === selectedVersion);
    return versionData ? appDownloads.some(d => d.File?.name === `${versionData.hash}.zip`) : false;
  }, [app, selectedVersion, sortedVersions, appDownloads]);

  const isDownloading = useMemo(() => {
    if (!app) return false;
    return Object.keys(activeDownloads).some(key => key.startsWith(`${app.package_id.package_name}:`));
  }, [app, activeDownloads]);

  const downloadProgress = useMemo(() => {
    if (!isDownloading || !app) return null;
    const activeDownloadKey = Object.keys(activeDownloads).find(key =>
      key.startsWith(`${app.package_id.package_name}:`)
    );
    if (!activeDownloadKey) return null;
    const progress = activeDownloads[activeDownloadKey];
    return progress ? Math.round((progress.downloaded / progress.total) * 100) : 0;
  }, [isDownloading, app, activeDownloads]);

  const loadData = useCallback(async () => {
    if (!id) return;
    setIsLoading(true);
    setError(null);

    try {
      const [appData, installedAppData] = await Promise.all([
        isDevMode ? Promise.resolve(MOCK_APP) : fetchListing(id),
        fetchInstalledApp(id)
      ]);

      if (!appData) {
        setError("App not found");
        return;
      }

      setApp(appData);
      console.log('app data loaded: ', appData);
      setInstalledApp(installedAppData);

      const versions = appData.metadata?.properties?.code_hashes || [];
      if (versions.length > 0) {
        const latestVer = versions.reduce((latest, current) =>
          compareVersions(current[0], latest[0]) > 0 ? current : latest
        )[0];
        setLatestVersion(latestVer);
        setSelectedVersion(latestVer);

        if (installedAppData) {
          const installedVersion = versions.find(([_, hash]) => hash === installedAppData.our_version_hash);
          if (installedVersion) {
            setCurrentVersion(installedVersion[0]);
            setUpToDate(installedVersion[0] === latestVer);
          }
        }
      }

      await fetchHomepageApps();
      setCanLaunch(!!getLaunchUrl(`${appData.package_id.package_name}:${appData.package_id.publisher_node}`));
    } catch (err) {
      setError("Failed to load app details. Please try again.");
      console.error(err);
    } finally {
      setIsLoading(false);
    }
  }, [id, fetchListing, fetchInstalledApp, fetchHomepageApps, getLaunchUrl]);

  const handleMirrorSelect = useCallback((mirror: string, status: boolean | null | 'http') => {
    setSelectedMirror(mirror);
    setIsMirrorOnline(status === 'http' ? true : status);
    setMirrorError(null);
  }, []);

  const handleMirrorError = useCallback((error: string) => {
    setMirrorError(error);
    setIsMirrorOnline(false);
    setAttemptedDownload(false);
  }, []);

  const handleInstallFlow = useCallback(async (isDownloadNeeded: boolean = false) => {
    if (!id || !selectedMirror || !app || !selectedVersion) return;

    const versionData = sortedVersions.find(v => v.version === selectedVersion);
    if (!versionData) return;

    try {
      if (isDownloadNeeded) {
        const appId = `${id}:${versionData.hash}`;
        await downloadApp(id, versionData.hash, selectedMirror);

        // Poll activeDownloads until this download is complete
        while (true) {
          const activeDownloads = useAppsStore.getState().activeDownloads;
          if (!activeDownloads[appId]) {
            break;
          }
          await new Promise(resolve => setTimeout(resolve, 500));
        }
      }

      const downloads = await fetchDownloadsForApp(id);
      const download = downloads.find(d => d.File?.name === `${versionData.hash}.zip`);

      if (download?.File?.manifest) {
        const manifest_response: ManifestResponse = {
          package_id: app.package_id,
          version_hash: versionData.hash,
          manifest: download.File.manifest
        };
        setManifestResponse(manifest_response);
        setShowCapApproval(true);
      } else {
        throw new Error('Manifest not found for the selected version');
      }
    } catch (error) {
      console.error('Installation flow failed:', error);
      setError('Installation failed. Please try again.');
    }
  }, [id, selectedMirror, app, selectedVersion, sortedVersions, downloadApp, fetchDownloadsForApp]);

  const confirmInstall = useCallback(async () => {
    if (!id || !selectedVersion || !app) return;

    const versionData = sortedVersions.find(v => v.version === selectedVersion);
    if (!versionData) return;

    try {
      setIsInstalling(true);
      await installApp(id, versionData.hash);
      // Refresh all relevant data after 3 seconds
      setTimeout(async () => {
        setShowCapApproval(false);
        setManifestResponse(null);
        setIsInstalling(false);
        await Promise.all([
          fetchHomepageApps(),
          loadData()
        ]);
      }, 3000);
    } catch (error) {
      console.error('Installation failed:', error);
      setError('Installation failed. Please try again.');
    }
  }, [id, selectedVersion, app, sortedVersions, installApp, fetchHomepageApps, loadData]);

  const handleLaunch = useCallback(() => {
    if (!app) return;
    const launchUrl = getLaunchUrl(`${app.package_id.package_name}:${app.package_id.publisher_node}`);
    if (launchUrl) {
      window.location.href = window.location.origin.replace('//app-store-sys.', '//') + launchUrl;
    }
  }, [app, getLaunchUrl]);

  const handleUninstall = async () => {
    if (!app) return;
    setIsUninstalling(true);
    try {
      await uninstallApp(`${app.package_id.package_name}:${app.package_id.publisher_node}`);
      await loadData();
    } catch (error) {
      console.error('Uninstallation failed:', error);
      setError(`Uninstallation failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsUninstalling(false);
      window.location.reload();
    }
  };

  const handleToggleAutoUpdate = async () => {
    if (!app || !latestVersion) return;
    setIsTogglingAutoUpdate(true);
    try {
      await setAutoUpdate(
        `${app.package_id.package_name}:${app.package_id.publisher_node}`,
        latestVersion,
        !app.auto_update
      );
      await loadData();
    } catch (error) {
      console.error('Failed to toggle auto-update:', error);
      setError(`Failed to toggle auto-update: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setIsTogglingAutoUpdate(false);
    }
  };

  useEffect(() => {
    if (attemptedDownload && !selectedMirror) {
      const checkMirrorsAndStart = async () => {
        const result = await checkMirrors(id!, handleMirrorSelect);
        if ('error' in result) {
          handleMirrorError(result.error);
        }
      };
      checkMirrorsAndStart();
    }
  }, [attemptedDownload, selectedMirror, id, checkMirrors, handleMirrorSelect, handleMirrorError]);

  useEffect(() => {
    if (attemptedDownload && selectedMirror && isMirrorOnline !== null) {
      setAttemptedDownload(false);
      handleInstallFlow(true);
    }
  }, [attemptedDownload, selectedMirror, isMirrorOnline, handleInstallFlow]);

  useEffect(() => {
    loadData();
    clearAllActiveDownloads();
    window.scrollTo(0, 0);
  }, [loadData, clearAllActiveDownloads]);

  if (isLoading) {
    return (
      <div className="app-page min-h-screen">
        <div className="h-40 flex items-center justify-center">
          <h4>Loading app details...</h4>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="app-page min-h-screen">
        <div className="h-40 flex items-center justify-center">
          <h4>{error}</h4>
        </div>
      </div>
    );
  }

  if (!app) {
    return (
      <div className="app-page min-h-screen">
        <div className="h-40 flex items-center justify-center">
          <h4>App details not found for {id}</h4>
        </div>
      </div>
    );
  }

  const valid_wit_version = app.metadata?.properties?.wit_version === 1;
  const canDownload = !isDownloading && !isDownloaded;

  return (
    <div className="app-page min-h-screen">
      {showCapApproval && manifestResponse && (
        <div className="cap-approval-popup">
          <div className="cap-approval-content">
            <h3 className="prose">Approve Capabilities</h3>
            <ManifestDisplay manifestResponse={manifestResponse} />
            <div className="approval-buttons">
              <button onClick={() => {
                setShowCapApproval(false);
                setIsInstalling(false);
              }}>Cancel</button>
              {isInstalling
                ? <><FaSpinner className="fa-spin" /> Installing...</>
                : <button onClick={confirmInstall}>Approve and Install</button>
              }
            </div>
          </div>
        </div>
      )}
      <div className="flex items-center justify-between gap-2 flex-wrap min-h-md">
        <div className="flex flex-col items-center gap-4">
          <div className="w-32 h-32 flex items-center justify-center">
            <img
              src={app.metadata?.image || '/h-green.svg'}
              alt={app.metadata?.name || app.package_id.package_name}
              className="w-32 h-32 object-cover rounded-lg"
            />
          </div>
          <div className="app-title">
            <h2>{app.metadata?.name || app.package_id.package_name}</h2>
          </div>
        </div>
        <ul className="detail-list min-h-md">
          <li>
            <span>Installed:</span>
            <span className="status-icon">
              {installedApp ? <FaCheck className="installed" /> : <FaTimes className="not-installed" />}
            </span>
          </li>
          {installedApp && <li>
            <span>Auto Update:</span>
            <span className="status-icon">
              {app.auto_update ? <FaCheck className="installed" /> : <FaTimes className="not-installed" />}
            </span>
          </li>}
          {latestVersion && (
            <>
              <li><span>Version:</span> <span>{latestVersion}</span></li>
            </>
          )}
          {currentVersion && latestVersion && currentVersion !== latestVersion && (
            <li><span>Installed Version:</span> <span>{currentVersion}</span></li>
          )}
          {currentVersion && latestVersion && currentVersion === latestVersion && (
            <li><span>Up to date:</span> <span className="status-icon">
              {upToDate ? <FaCheck className="installed" /> : <FaTimes className="not-installed" />}
            </span></li>
          )}
          {installedApp?.pending_update_hash && (
            <li className="warning">
              <span>Failed Auto-Update:</span>
              <span>Update to version with hash {installedApp.pending_update_hash.slice(0, 8)}... failed</span>
            </li>
          )}
          <li><span>Publisher:</span> <span>{app.package_id.publisher_node}</span></li>
          {app.metadata?.properties?.license && (
            <li><span>License:</span> <span>{app.metadata.properties.license}</span></li>
          )}
        </ul>
      </div>

      {!valid_wit_version && <div className="p-2 bg-neon text-black">This app must be updated to 1.0</div>}

      <div className="app-actions min-h-md flex flex-col gap-2">
        {installedApp && (
          <>
            {canLaunch && (
              <button onClick={handleLaunch} >
                <FaPlay /> Launch
              </button>
            )}
            <button onClick={handleUninstall} className="clear">
              {isUninstalling ? <FaSpinner className="animate-spin" /> : <FaTrash />} Uninstall
            </button>
            <button onClick={handleToggleAutoUpdate} className="clear">
              {isTogglingAutoUpdate ? <FaSpinner className="animate-spin" /> : <FaSync />}
              {app.auto_update ? " Disable" : " Enable"} Auto Update
            </button>
          </>
        )}
        {valid_wit_version && !upToDate && (
          <div className="flex flex-col gap-2">
            {(isDevMode || isDownloaded) ? (
              !showCapApproval && (
                <button
                  onClick={() => handleInstallFlow(false)}
                >
                  {isInstalling ? (
                    <><FaSpinner className="animate-spin" /> Installing...</>
                  ) : (
                    <>{installedApp ? <><FaDownload /> Update</> : <><FaDownload /> Download</>}</>
                  )}
                </button>
              )
            ) : (
              <button
                onClick={() => {
                  if (!selectedMirror || isMirrorOnline === null) {
                    setAttemptedDownload(true);
                  } else {
                    handleInstallFlow(true);
                  }
                }}
                className={` ${isDownloading ? 'loading' : ''}`}
              >
                {isDownloading ? (
                  <><FaSpinner className="animate-spin" /> Downloading... {downloadProgress}%</>
                ) : mirrorError ? (
                  <><FaTimes /> {mirrorError}</>
                ) : !selectedMirror && attemptedDownload ? (
                  <><FaSpinner className="animate-spin" /> Choosing mirrors...</>
                ) : (
                  <>{installedApp ? <><FaDownload /> Update</> : <><FaDownload /> Download</>}</>
                )}
              </button>
            )}
          </div>
        )}
      </div>

      {app.metadata?.properties?.screenshots && (
        <div className="flex flex-col gap-2">
          <h3 className="prose">Screenshots</h3>
          <div className="flex flex-wrap gap-2 overflow-y-auto min-h-0 max-h-lg">
            {app.metadata.properties.screenshots.map((screenshot, index) => (
              <img
                src={screenshot}
                alt={`Screenshot ${index + 1}`}
                className="rounded-lg w-full h-full object-contain aspect-video max-w-md max-h-md"
                loading="lazy"
              />
            ))}
          </div>
        </div>
      )}

      <div className="wrap-anywhere ">
        {app.metadata?.description || "No description available"}
      </div>

      {valid_wit_version && !upToDate && (
        <>
          <button
            onClick={() => setShowAdvanced(!showAdvanced)} className="clear"
          >
            {showAdvanced ? <FaChevronUp /> : <FaChevronDown />} Advanced Download Options
          </button>
          {showAdvanced && (
            <div className="flex flex-col gap-2">
              <h3 className="prose">Advanced Options</h3>
              <label className=" text-sm font-medium">Mirror Selection</label>
              <MirrorSelector
                packageId={id}
                onMirrorSelect={handleMirrorSelect}
                onError={handleMirrorError}
              />
              {mirrorError && (
                <p className=" text-sm text-red-600">{mirrorError}</p>
              )}
              <label className=" text-sm font-medium ">Version Selection</label>
              <select
                value={selectedVersion}
                onChange={(e) => setSelectedVersion(e.target.value)}
                className="version-selector"
              >
                <option value="">Select version</option>
                {sortedVersions.map((version) => (
                  <option key={version.version} value={version.version}>
                    {version.version}
                  </option>
                ))}
              </select>
            </div>
          )}
        </>
      )}
    </div>
  );
}
