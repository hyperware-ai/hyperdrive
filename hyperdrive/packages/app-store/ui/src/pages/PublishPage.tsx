import React, { useState, useCallback, FormEvent, useEffect } from "react";
import { Link, useLocation } from "react-router-dom";
import { useAccount, useWriteContract, useWaitForTransactionReceipt, usePublicClient } from 'wagmi'
import { ConnectButton, useConnectModal } from '@rainbow-me/rainbowkit';
import { keccak256, toBytes } from 'viem';
import { mechAbi, HYPERMAP, encodeIntoMintCall, encodeMulticalls, hypermapAbi, MULTICALL } from "../abis";
import { hyperhash } from '../utils/hyperhash';
import useAppsStore from "../store";
import { PackageSelector } from "../components";
import { Tooltip } from '../components/Tooltip';
import { FaCircleNotch, FaInfo, FaWallet } from "react-icons/fa6";
import { AppListing } from "../types/Apps";

const NAME_INVALID = "Package name must contain only valid characters (a-z, 0-9, -, and .)";

const MOCK_APPS: AppListing[] = [
  {
    package_id: {
      package_name: "The App That Kills You",
      publisher_node: "lethal-wish.os"
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata.json",
    metadata_hash: "0x0000000000000000000000000000000000000000",
    auto_update: false
  },
  {
    package_id: {
      package_name: "One More for the Road",
      publisher_node: "coffee-enthusiast.os"
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata.json",
    metadata_hash: "0x0000000000000000000000000000000000000000",
    auto_update: false
  },

  {
    package_id: {
      package_name: "A Gentleman's Guide to the Galaxy",
      publisher_node: "gentleman-guide.os"
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata.json",
    metadata_hash: "0x0000000000000000000000000000000000000000",
    auto_update: false
  },
];

export default function PublishPage() {
  const { openConnectModal } = useConnectModal();
  const { ourApps, fetchOurApps, downloads, fetchDownloadsForApp } = useAppsStore();
  const publicClient = usePublicClient();

  const { address, isConnected, isConnecting } = useAccount();
  const { data: hash, writeContract, error } = useWriteContract();
  const { isLoading: isConfirming, isSuccess: isConfirmed } =
    useWaitForTransactionReceipt({
      hash,
    });

  const [packageName, setPackageName] = useState<string>("");
  // @ts-ignore
  const [publisherId, setPublisherId] = useState<string>(window.our?.node || "");
  const [metadataUrl, setMetadataUrl] = useState<string>("");
  const [metadataHash, setMetadataHash] = useState<string>("");

  const [nameValidity, setNameValidity] = useState<string | null>(null);
  const [metadataError, setMetadataError] = useState<string | null>(null);

  const [isDevMode, setIsDevMode] = useState<boolean>(false);
  const [backtickKeyPresses, setBacktickKeyPresses] = useState<number>(0);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === '`') {
        setBacktickKeyPresses(prev => prev + 1);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  useEffect(() => {
    if (backtickKeyPresses >= 5) {
      setIsDevMode(prev => !prev);
      setBacktickKeyPresses(0);
    }
  }, [backtickKeyPresses]);

  useEffect(() => {
    fetchOurApps();
  }, [fetchOurApps]);

  useEffect(() => {
    if (packageName && publisherId) {
      const id = `${packageName}:${publisherId}`;
      fetchDownloadsForApp(id);
    }
  }, [packageName, publisherId, fetchDownloadsForApp]);

  useEffect(() => {
    if (isConfirmed) {
      // Fetch our apps again after successful publish
      fetchOurApps();
      // Reset form fields
      setPackageName("");
      // @ts-ignore
      setPublisherId(window.our?.node || "");
      setMetadataUrl("");
      setMetadataHash("");
    }
  }, [isConfirmed, fetchOurApps]);

  const validatePackageName = useCallback((name: string) => {
    // Allow lowercase letters, numbers, hyphens, and dots
    const validNameRegex = /^[a-z0-9.-]+$/;

    if (!validNameRegex.test(name)) {
      setNameValidity(NAME_INVALID);
    } else {
      setNameValidity(null);
    }
  }, []);

  useEffect(() => {
    if (packageName) {
      validatePackageName(packageName);
    } else {
      setNameValidity(null);
    }
  }, [packageName, validatePackageName]);


  const calculateMetadataHash = useCallback(async () => {
    if (!metadataUrl) {
      setMetadataHash("");
      setMetadataError("");
      return;
    }

    try {
      const metadataResponse = await fetch(metadataUrl);
      const metadataText = await metadataResponse.text();
      const metadata = JSON.parse(metadataText);

      // Check if code_hashes exist in metadata and is an object
      if (metadata.properties && metadata.properties.code_hashes && typeof metadata.properties.code_hashes === 'object') {
        const codeHashes = metadata.properties.code_hashes;
        console.log('Available downloads:', downloads[`${packageName}:${publisherId}`]);

        const missingHashes = Object.entries(codeHashes).filter(([version, hash]) => {
          const hasDownload = downloads[`${packageName}:${publisherId}`]?.some(d => d.File?.name === `${hash}.zip`);
          return !hasDownload;
        });

        if (missingHashes.length == codeHashes.length) {
          setMetadataError(`Missing local downloads for mirroring versions: ${missingHashes.map(([version]) => version).join(', ')}`);
        } else {
          setMetadataError("");
        }
      } else {
        setMetadataError("The metadata does not contain the required 'code_hashes' property or it is not in the expected format");
      }

      const metadataHash = keccak256(toBytes(metadataText));
      setMetadataHash(metadataHash);
    } catch (error) {
      if (error instanceof SyntaxError) {
        setMetadataError("The metadata is not valid JSON. Please check the file for syntax errors.");
      } else if (error instanceof Error) {
        setMetadataError(`Error processing metadata: ${error.message}`);
      } else {
        setMetadataError("An unknown error occurred while processing the metadata.");
      }
      setMetadataHash("");
    }
  }, [metadataUrl, packageName, publisherId, downloads]);

  const handlePackageSelection = (packageName: string, publisherId: string) => {
    setPackageName(packageName);
    setPublisherId(publisherId);
  };

  const publishPackage = useCallback(
    async (e: FormEvent<HTMLFormElement>) => {
      e.preventDefault();
      e.stopPropagation();

      if (!publicClient || !address) {
        openConnectModal?.();
        return;
      }

      try {
        // Check if the package already exists and get its TBA
        console.log('packageName, publisherId: ', packageName, publisherId)
        let data = await publicClient.readContract({
          abi: hypermapAbi,
          address: HYPERMAP,
          functionName: 'get',
          args: [hyperhash(`${packageName}.${publisherId}`)]
        });

        let [tba, owner, _data] = data as [string, string, string];
        let isUpdate = Boolean(tba && tba !== '0x' && owner === address);
        let currentTBA = isUpdate ? tba as `0x${string}` : null;
        console.log('currenttba, isupdate, owner, address: ', currentTBA, isUpdate, owner, address)
        // If the package doesn't exist, check for the publisher's TBA
        if (!currentTBA) {
          data = await publicClient.readContract({
            abi: hypermapAbi,
            address: HYPERMAP,
            functionName: 'get',
            args: [hyperhash(publisherId)]
          });

          [tba, owner, _data] = data as [string, string, string];
          isUpdate = false; // It's a new package, but we might have a publisher TBA
          currentTBA = (tba && tba !== '0x') ? tba as `0x${string}` : null;
        }

        let metadata = metadataHash;
        if (!metadata) {
          const metadataResponse = await fetch(metadataUrl);
          await metadataResponse.json(); // confirm it's valid JSON
          const metadataText = await metadataResponse.text(); // hash as text
          metadata = keccak256(toBytes(metadataText));
        }

        const multicall = encodeMulticalls(metadataUrl, metadata);
        const args = isUpdate ? multicall : encodeIntoMintCall(multicall, address, packageName);

        writeContract({
          abi: mechAbi,
          address: currentTBA || HYPERMAP,
          functionName: 'execute',
          args: [
            isUpdate ? MULTICALL : HYPERMAP,
            BigInt(0),
            args,
            isUpdate ? 1 : 0
          ],
          gas: BigInt(1000000),
        });

      } catch (error) {
        console.error(error);
      }
    },
    [publicClient, openConnectModal, packageName, publisherId, address, metadataUrl, metadataHash, writeContract]
  );

  const unpublishPackage = useCallback(
    async (packageName: string, publisherName: string) => {
      try {
        if (!publicClient) {
          openConnectModal?.();
          return;
        }

        const data = await publicClient.readContract({
          abi: hypermapAbi,
          address: HYPERMAP,
          functionName: 'get',
          args: [hyperhash(`${packageName}.${publisherName}`)]
        });

        const [tba, _owner, _data] = data as [string, string, string];

        if (!tba || tba === '0x') {
          console.error("No TBA found for this package");
          return;
        }

        const multicall = encodeMulticalls("", "");

        writeContract({
          abi: mechAbi,
          address: tba as `0x${string}`,
          functionName: 'execute',
          args: [
            MULTICALL,
            BigInt(0),
            multicall,
            1
          ],
          gas: BigInt(1000000),
        });

      } catch (error) {
        console.error(error);
      }
    },
    [publicClient, openConnectModal, writeContract]
  );

  return (
    <div
      className="publish-page p-4 bg-black/10 dark:bg-white/10 rounded-lg min-h-md max-w-screen md:max-w-screen-md mx-auto flex flex-col gap-4 mb-64 md:mb-auto overflow-y-auto"
    >
      {(isDevMode || address) && (
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <FaWallet className="text-sm" />
            <span className="text-sm font-mono wrap-anywhere">{address || 'Dev Mode'}</span>
          </div>
          <span className="text-xs opacity-50">Make sure the connected wallet is the owner of this node!</span>
        </div>
      )}

      {(isDevMode || Object.keys(ourApps).length === 0) && (
        <div className="font-bold flex items-center gap-1 text-lg">
          <span className="text-iris dark:text-neon">First time?</span>
          <a
            className="font-bold "
            href="https://book.hyperware.ai/my_first_app/chapter_5.html"
            target="_blank">
            <span className="text-black dark:text-white">Read the guide.</span>
          </a>
        </div>
      )}

      {(isDevMode || Object.keys(ourApps).length > 0) && <>
        <h2 className="prose">Your Published Apps</h2>
        <div className="flex flex-col gap-2">
          {Object.values(isDevMode ? MOCK_APPS : ourApps).map((app: AppListing) => (
            <div
              key={`${app.package_id.package_name}:${app.package_id.publisher_node}`}
              className="flex items-center gap-2"
            >
              <Link to={`/app/${app.package_id.package_name}:${app.package_id.publisher_node}`} className="app-name">
                {app.metadata?.image && (
                  <img src={app.metadata.image} alt="" className="package-icon" />
                )}
                <span>{app.metadata?.name || app.package_id.package_name}</span>
              </Link>

              <button
                onClick={() => unpublishPackage(app.package_id.package_name, app.package_id.publisher_node)}
                className="clear !text-red-500 text-sm thin ml-auto">
                Unpublish
              </button>
            </div>
          ))}
        </div>
      </>}

      {(isDevMode || isConfirming) && (
        <div className="flex items-center gap-2 font-bold p-4 rounded-lg bg-iris text-white">
          <FaCircleNotch className="animate-spin" />
          <span>Publishing...</span>
        </div>
      )}

      {(isDevMode || (!address || !isConnected)) && (
        <h4 className="text-center prose font-bold bg-white rounded text-iris p-4"> Connect your wallet to publish an app.</h4>
      )}

      {(isDevMode || isConnecting) && (
        <div className="flex items-center gap-2 font-bold p-4 rounded-lg bg-iris text-white">
          <FaCircleNotch className="animate-spin" />
          <span>Waiting for wallet connection...</span>
        </div>
      )}

      {(isDevMode || (!isConfirming && !isConnecting && address && isConnected)) && (
        <form
          className="flex flex-col items-stretch gap-4 p-4 bg-black/10 dark:bg-white/10 rounded-lg"
          onSubmit={publishPackage}
        >
          <div className="flex flex-col gap-2 items-stretch">
            <label
              htmlFor="package-select"
              className="text-sm opacity-50"
            >
              Select app to publish
            </label>
            <PackageSelector
              onPackageSelect={handlePackageSelection}
              publisherId={publisherId}
            />
            {(isDevMode || nameValidity) && <p className="p-2 bg-red-500 text-white rounded-lg">{nameValidity || (isDevMode && 'Dev mode is enabled')}</p>}
          </div>

          <div className="flex flex-col gap-2 items-stretch">
            <label
              htmlFor="metadata-url"
              className="text-sm opacity-50"
            >
              Metadata URL
            </label>
            <input
              type="text"
              value={metadataUrl}
              onChange={(e) => setMetadataUrl(e.target.value)}
              onBlur={calculateMetadataHash}
              placeholder="https://raw.githubusercontent.com/hyperware-ai/kit/47cdf82f70b36f2a102ddfaaeed5efa10d7ef5b9/src/new/templates/rust/ui/chat/metadata.json"
            />
            {(isDevMode || metadataError) && <p className="p-2 bg-red-500 text-white rounded-lg">{metadataError || (isDevMode && 'Dev mode is enabled')}</p>}
          </div>
          {(metadataHash || isDevMode) && <div className="flex flex-col gap-2 items-stretch">
            <label
              htmlFor="metadata-hash"
              className="text-sm opacity-50"
            >
              Metadata Hash
            </label>
            <input
              readOnly
              type="text"
              value={metadataHash}
              placeholder="Calculated automatically from metadata URL"
            />
          </div>}
          <button
            type="submit"
            disabled={isConfirming || nameValidity !== null || Boolean(metadataError)}>
            {isConfirming ? (
              <>
                <FaCircleNotch className="animate-spin" />
                <span>Publishing...</span>
              </>
            ) : (
              'Publish'
            )}
          </button>
        </form>
      )}

      {(isConfirmed || isDevMode) && (
        <div className="flex items-center font-bold p-4 rounded-lg bg-neon text-black">
          App published successfully!
        </div>
      )}

      {(error || isDevMode) && (
        <div className="flex items-center font-bold p-4 rounded-lg bg-red-500 text-white">
          <pre>
            Error: {error?.message || 'Dev mode is enabled'}
          </pre>
        </div>
      )}
    </div>
  );
}