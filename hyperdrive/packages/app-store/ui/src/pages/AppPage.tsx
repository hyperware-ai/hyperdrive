import React, { useEffect, useState, useCallback, useMemo } from "react";
import { useParams } from "react-router-dom";
import useAppsStore from "../store";
import { AppListing, PackageState, ManifestResponse } from "../types/Apps";
import { compareVersions } from "../utils/compareVersions";
import { MirrorSelector, ManifestDisplay } from '../components';
import { FaChevronDown, FaChevronRight, FaCheck, FaCircleNotch, FaPlay } from "react-icons/fa6";
import { BsDownload } from "react-icons/bs";
import { Modal } from "../components/Modal";
import classNames from "classnames";
import { VscSync, VscSyncIgnored } from "react-icons/vsc";
import { BsX } from "react-icons/bs";
import ConfirmUninstallModal from "../components/ConfirmUninstallModal";

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

const isMobile = window.innerWidth < 768;

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
  const [showUninstallConfirmModal, setShowUninstallConfirmModal] = useState(false);
  const [isDevMode, setIsDevMode] = useState(false);
  const [backtickPressCount, setBacktickPressCount] = useState(0);

  useEffect(() => {
    const backTickCounter = (e: KeyboardEvent) => {
      if (e.key === '`') {
        setBacktickPressCount(old => old + 1);
      }
    };
    window.addEventListener('keydown', backTickCounter);
    return () => window.removeEventListener('keydown', backTickCounter);
  }, []);

  useEffect(() => {
    if (backtickPressCount >= 5) {
      setIsDevMode(!isDevMode);
      setBacktickPressCount(0);
    }
  }, [backtickPressCount]);

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
    if (!id) {
      setError("App not found");
      return;
    }

    if (!selectedMirror) {
      setError("No mirror selected");
      return;
    }

    if (!app) {
      setError("App not found");
      return;
    }

    if (!selectedVersion) {
      setError("No version selected");
      return;
    }

    const versionData = sortedVersions.find(v => v.version === selectedVersion);
    if (!versionData) {
      setError("Version not found");
      return;
    }

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

  const appButtons = ({ className }: { className?: string }) => <div className={classNames("app-buttons flex items-stretch gap-2 ", className)}>
    {installedApp && <>
      <button
        onClick={() => setShowUninstallConfirmModal(true)}
        className="clear thin"
      >
        {isUninstalling ? <FaCircleNotch className="animate-spin" /> : <BsX />}
        <span >Uninstall</span>
      </button>
      <button
        onClick={handleToggleAutoUpdate}
        className="clear thin"
      >
        {isTogglingAutoUpdate
          ? <FaCircleNotch className="animate-spin" />
          : app.auto_update
            ? <VscSync className="text-lg" />
            : <VscSyncIgnored className="text-lg" />}
        <span >Updates {app.auto_update ? " ON" : " OFF"}</span>
      </button>
      {(canLaunch || isDevMode) && (
        <button
          onClick={handleLaunch}
        >
          <FaPlay />
          <span >Launch</span>
        </button>
      )}
    </>}

    {valid_wit_version && !upToDate && <>
      {(isDevMode || isDownloaded) && <>

        <button
          onClick={() => handleInstallFlow(false)}
          className={classNames("text-sm", {
          })}
        >
          {showCapApproval || isInstalling ? (
            <><FaCircleNotch className="animate-spin" /> Installing...</>
          ) : (
            <>
              <BsDownload />
              <span >{installedApp ? "Update" : "Download"}</span>
            </>
          )}
        </button>
      </>}

      {!isDownloaded && !isDevMode && <button
        onClick={() => {
          if (!selectedMirror || isMirrorOnline === null) {
            setAttemptedDownload(true);
          } else {
            handleInstallFlow(true);
          }
        }}
        className={classNames(' text-sm', {
          'loading': isDownloading,
        })}
        disabled={isDownloading}
      >
        {isDownloading ? (
          <><FaCircleNotch className="animate-spin" /> Downloading... {downloadProgress}%</>
        ) : mirrorError ? (
          <><BsX /> {mirrorError}</>
        ) : !selectedMirror && attemptedDownload ? (
          <><FaCircleNotch className="animate-spin" /> Choosing mirrors...</>
        ) : (
          <>{installedApp ? <><BsDownload /> Update</> : <><BsDownload /> Download</>}</>
        )}
      </button>}
    </>}
  </div>

  return (
    <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-stretch gap-4">
      {showCapApproval && manifestResponse && (
        <Modal onClose={() => setShowCapApproval(false)}>
          <h3 className="prose">Approve Capabilities</h3>
          <ManifestDisplay manifestResponse={manifestResponse} />
          <div className="flex flex-col items-stretch gap-2">
            <button
              className="clear"
              disabled={isInstalling}
              onClick={() => {
                setShowCapApproval(false);
                setIsInstalling(false);
              }}>Cancel</button>
            <button
              disabled={isInstalling}
              onClick={confirmInstall}
            >
              {isInstalling ? <FaCircleNotch className="animate-spin" /> : "Approve and Install"}
            </button>
          </div>
        </Modal>
      )}
      <div className="flex justify-between gap-2 flex-wrap">
        <div className="w-16 md:w-32 h-16 md:h-32 flex items-center justify-center rounded-lg">
          {app.metadata?.image && <img
            src={app.metadata?.image}
            alt={`${app.metadata?.name || app.package_id.package_name} icon`}
            className="w-16 md:w-32 h-16 md:h-32 object-cover rounded-lg aspect-square bg-white dark:bg-black"
          />}
          {!app.metadata?.image && <div
            className="w-16 md:w-32 h-16 md:h-32  rounded-lg aspect-square bg-neon"
          />}
        </div>
        <div className="grid grid-cols-2 gap-2">
          {installedApp && <>
            <span>Auto Update:</span>
            <span className="flex items-center">
              {app.auto_update ? <FaCheck className="rounded-full bg-neon text-black" /> : <BsX className="rounded-full bg-red-500 text-white" />}
            </span>
          </>}
          {latestVersion && <>
            <span>Version:</span>
            <span>{latestVersion}</span>
          </>}
          {currentVersion && latestVersion && currentVersion !== latestVersion && <>
            <span>Installed Version:</span>
            <span>{currentVersion}</span>
          </>}
          {currentVersion && latestVersion && currentVersion === latestVersion && (
            <>
              <span>Up to date:</span>
              <span className="flex items-center">
                {upToDate ? <FaCheck className="rounded-full bg-neon text-black" /> : <BsX className="rounded-full bg-red-500 text-white" />}
              </span>
            </>
          )}
          {installedApp?.pending_update_hash && (
            <div className="bg-red-500 text-white p-2 rounded-lg col-span-2">
              <span>Failed Auto-Update: </span>
              <span>Update to version with hash {installedApp.pending_update_hash.slice(0, 8)}... failed</span>
            </div>
          )}
          <span>Publisher:</span>
          <span className="text-iris dark:text-neon font-bold">{app.package_id.publisher_node}</span>
          {app.metadata?.properties?.license && <>
            <span>License:</span>
            <span>{app.metadata.properties.license}</span>
          </>}
        </div>
      </div>

      <div className="flex items-center justify-between gap-2 flex-wrap">
        <h2 className="prose font-bold min-w-1/2 md:max-w-2/3">{app.metadata?.name || app.package_id.package_name}</h2>
        {appButtons({ className: "hidden md:flex" })}
      </div>

      <div className="wrap-anywhere ">
        {app.metadata?.description || "No description available"}
      </div>

      {!valid_wit_version && <div className="px-4 py-2 bg-neon text-black rounded">This app must be updated to 1.0</div>}

      {appButtons({ className: "flex-col items-stretch md:hidden" })}

      {(app.metadata?.properties?.screenshots || isDevMode) && (
        <div className="flex flex-col gap-2">
          <h3 className="prose">Screenshots</h3>
          <div className="flex flex-wrap gap-2 overflow-y-auto min-h-0 max-h-lg">
            {(isDevMode ? MOCK_APP.metadata!.properties.screenshots! : app!.metadata!.properties!.screenshots!).map((screenshot, index) => (
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

      {valid_wit_version && !upToDate && (
        <>
          <button
            onClick={() => setShowAdvanced(!showAdvanced)} className="clear"
          >
            {showAdvanced ? <FaChevronDown /> : <FaChevronRight />} Advanced Download Options
          </button>
          {showAdvanced && (
            <div className="flex flex-col gap-2">
              <label className=" text-sm font-medium ">Mirror </label>
              <MirrorSelector
                packageId={id}
                onMirrorSelect={handleMirrorSelect}
                onError={handleMirrorError}
              />
              {(mirrorError || isDevMode) && (
                <p className=" text-xs text-red-600">{isDevMode ? mirrorError || "Dev Mode" : mirrorError}</p>
              )}
              <label className=" text-sm font-medium ">Version </label>
              <select
                value={selectedVersion}
                onChange={(e) => setSelectedVersion(e.target.value)}
                className="bg-black/10 dark:bg-white/10 text-gray-500 self-stretch p-2 rounded-lg"
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

        {showUninstallConfirmModal && <ConfirmUninstallModal
      onClose={() => setShowUninstallConfirmModal(false)}
      onUninstall={handleUninstall}
      appName={app.metadata?.name || app.package_id.package_name} />}
    </div>
  );
}
