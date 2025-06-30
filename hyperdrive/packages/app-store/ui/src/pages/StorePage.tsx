import React, { useState, useEffect } from "react";
import useAppsStore from "../store";
import { AppListing } from "../types/Apps";
import { FaSearch } from "react-icons/fa";
import { ResetButton } from "../components";
import { AppCard } from "../components/AppCard";
import { BsSearch } from "react-icons/bs";
const mockApps: AppListing[] = [
  {
    package_id: {
      package_name: "test-app",
      publisher_node: "test-node",
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata",
    metadata_hash: "1234567890",
    auto_update: false,
    metadata: {
      name: "Test App",
      description: "This is a test app",
      properties: {
        package_name: "test-app",
        publisher: "test-node",
        current_version: "1.0.0",
        mirrors: [],
        code_hashes: [],
      },
    },
  },
  {
    package_id: {
      package_name: "test-app",
      publisher_node: "test-node",
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata",
    metadata_hash: "1234567890",
    auto_update: false,
    metadata: {
      name: "Test App",
      description: "This is a test app",
      properties: {
        package_name: "test-app",
        publisher: "test-node",
        current_version: "1.0.0",
        mirrors: [],
        code_hashes: [],
      },
    },
  },
  {
    package_id: {
      package_name: "test-app",
      publisher_node: "test-node",
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata",
    metadata_hash: "1234567890",
    auto_update: false,
    metadata: {
      name: "Test App TestappTestappTestappTestappTestapp",
      description: "adsf adf adsf asdf asdf adgfagafege aadsf adf adsf asdf asdf adgfagafege aadsf adf adsf asdf asdf adgfagafege aadsf adf adsf asdf asdf adgfagafege aadsf adf adsf asdf asdf adgfagafege aadsf adf adsf asdf asdf adgfagafege a",
      properties: {
        package_name: "test-app",
        publisher: "test-node",
        current_version: "1.0.0",
        mirrors: [],
        code_hashes: [],
      },
    },
  },
  {
    package_id: {
      package_name: "test-app",
      publisher_node: "test-node",
    },
    tba: "0x0000000000000000000000000000000000000000",
    metadata_uri: "https://example.com/metadata",
    metadata_hash: "1234567890",
    auto_update: false,
    metadata: {
      name: "Test App",
      description: "This is a test app",
      properties: {
        package_name: "test-app",
        publisher: "test-nodetest-nodetest-nodetest-nodetest-node",
        current_version: "1.0.0",
        mirrors: [],
        code_hashes: [],
      },
    },
  },
];

export default function StorePage() {
  const { listings, fetchListings, fetchUpdates, fetchHomepageApps, getLaunchUrl, fetchInstalledApp } = useAppsStore();
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [launchableApps, setLaunchableApps] = useState<AppListing[]>([]);
  const [appsNotInstalled, setAppsNotInstalled] = useState<AppListing[]>([]);
  const [isDevMode, setIsDevMode] = useState<boolean>(false);

  const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    console.log(e.target.value, searchQuery);
    setSearchQuery(e.target.value);
  }

  useEffect(() => {
    if (searchQuery.match(/``````/)) {
      setIsDevMode(!isDevMode);
    }
  }, [searchQuery]);

  useEffect(() => {
    fetchListings();
    fetchUpdates();
    fetchHomepageApps();
  }, [fetchListings, fetchUpdates, fetchHomepageApps]);

  useEffect(() => {
    if (listings) {
      setLaunchableApps(Object.values(listings).filter((app) => getLaunchUrl(`${app.package_id.package_name}:${app.package_id.publisher_node}`)));
      setAppsNotInstalled(Object.values(listings).filter((app) => !appIsInstalled(app)));
    }
  }, [listings, getLaunchUrl]);

  const appIsInstalled = async (app: AppListing) => {
    const installedAppData = await fetchInstalledApp(app.package_id.package_name);
    return installedAppData !== null;
  }

  // extensive temp null handling due to weird prod bug
  const filteredApps = React.useMemo(() => {
    if (!listings) return [];
    return Object.values(listings).filter((app) => {
      if (!app || !app.package_id) return false;
      const nameMatch = app.package_id.package_name.toLowerCase().includes(searchQuery.toLowerCase());
      const descMatch = app.metadata?.description?.toLowerCase().includes(searchQuery.toLowerCase()) || false;
      return nameMatch || descMatch;
    });
  }, [listings, searchQuery]);

  return (
    <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-stretch gap-4">
      <div className="flex items-center self-stretch gap-2 relative">
        <BsSearch className="text-xl opacity-50 absolute left-2" />
        <input
          type="text"
          placeholder="Search apps..."
          value={searchQuery}
          onChange={onInputChange}
          className="grow self-stretch text-sm pl-32"
        />
      </div>

      {/* @ts-ignore */}
      {isDevMode && <div
        className="grid grid-cols-1 md:grid-cols-2 gap-4"
      >
        {mockApps.map((app) => (
          <AppCard
            key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`}
            app={app}
          >
            <span className="bg-iris/10 text-iris dark:bg-black dark:text-neon font-bold px-3 py-1 rounded-full">Install</span>
            <span className="bg-iris/10 text-iris dark:bg-black dark:text-neon font-bold px-3 py-1 rounded-full">Launch</span>
          </AppCard>
        ))}
      </div>}
      {!listings ? (
        <p>Loading...</p>
      ) : filteredApps.length === 0 ? (
        <>
          <p>No apps available.</p>
        </>
      ) : (
        <div
          className="grid grid-cols-1 md:grid-cols-2  gap-4"
        >
          {filteredApps.map((app) => (
            <AppCard
              key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`}
              app={app}
            >
              {appsNotInstalled.includes(app)
                ? <span className="bg-iris/10 text-iris dark:bg-black dark:text-neon font-bold  px-3 py-1 rounded-full">Install</span>
                : launchableApps.includes(app)
                  ? <span className="bg-iris/10 text-iris dark:bg-black dark:text-neon font-bold  px-3 py-1 rounded-full">View</span>
                  : <span className="bg-iris/10 text-iris dark:bg-black dark:text-neon  font-bold px-3 py-1 rounded-full">Installed</span>}
            </AppCard>
          ))}
        </div>
      )}
      <div className="flex flex-col items-center justify-center text-center gap-2">
        <p className="text-sm font-bold uppercase">Can't find the app you're looking for?</p>
        <ResetButton />
      </div>
    </div>
  );
}
