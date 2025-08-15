import React, { useState, useEffect } from "react";
import useAppsStore from "../store";
import { AppListing } from "../types/Apps";
import { FaChevronLeft, FaChevronRight } from "react-icons/fa6";
import { ResetButton } from "../components";
import { AppCard } from "../components/AppCard";
import { BsSearch } from "react-icons/bs";
import classNames from "classnames";
import { useLocation } from "react-router-dom";
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
  const { listings, installed, fetchListings, fetchInstalled, fetchUpdates, fetchHomepageApps, getLaunchUrl } = useAppsStore();
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [launchableApps, setLaunchableApps] = useState<AppListing[]>([]);
  const [appsNotInstalled, setAppsNotInstalled] = useState<AppListing[]>([]);
  const [isDevMode, setIsDevMode] = useState<boolean>(false);
  const [currentPage, setCurrentPage] = useState<number>(1);
  const [pageSize, setPageSize] = useState<number>(10);

  // if we have ?search=something, set the search query to that
  const location = useLocation();
  useEffect(() => {
    console.log({ location })
    const search = new URLSearchParams(location.search).get("search");
    if (search) {
      setSearchQuery(search);
      setCurrentPage(1);
    }
  }, [location]);

  const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    console.log(e.target.value, searchQuery);
    setSearchQuery(e.target.value);
  }

  const onPageSizeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const newPageSize = parseInt(e.target.value);
    setPageSize(newPageSize);
    setCurrentPage(Math.ceil(filteredApps.length / newPageSize));
  }

  useEffect(() => {
    if (searchQuery.match(/``````/)) {
      setIsDevMode(!isDevMode);
    }
  }, [searchQuery]);

  useEffect(() => {
    fetchListings();
    fetchInstalled();
    fetchUpdates();
    fetchHomepageApps();
  }, [fetchListings, fetchInstalled, fetchUpdates, fetchHomepageApps]);

  useEffect(() => {
    if (listings) {
      setLaunchableApps(Object.values(listings).filter((app) => getLaunchUrl(`${app.package_id.package_name}:${app.package_id.publisher_node}`)));

      // Check if app is installed by looking in the installed state
      const notInstalledApps = Object.values(listings).filter((app) => {
        const appId = `${app.package_id.package_name}:${app.package_id.publisher_node}`;
        return !installed[appId];
      });
      console.log({ notInstalledApps, installedKeys: Object.keys(installed) });
      setAppsNotInstalled(notInstalledApps);
    }
  }, [listings, installed, getLaunchUrl]);

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
      <div className="flex items-center self-stretch gap-2 items-center bg-black/10 dark:bg-white/10 rounded-lg pl-4">
        <BsSearch className="text-xl opacity-50 " />
        <input
          type="text"
          placeholder="Search apps..."
          value={searchQuery}
          onChange={onInputChange}
          className="grow  text-sm !bg-transparent"
          autoFocus
        />
      </div>


      {isDevMode && <div
        className="grid grid-cols-1 md:grid-cols-2 gap-4"
      >
        {mockApps.map((app) => (
          <AppCard
            key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`}
            app={app}
          >
            <ActionChip label="Install" />
            <ActionChip label="Launch" />
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
          className="grid grid-cols-1 md:grid-cols-2 gap-4"
        >
          {filteredApps.slice((currentPage - 1) * pageSize, currentPage * pageSize).map((app, index, slicedAppArray) => (
            <AppCard
              key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`}
              app={app}
            >
              {appsNotInstalled.includes(app)
                ? <ActionChip label="Install" />
                : launchableApps.includes(app)
                  ? <ActionChip label="Launch" />
                  : <ActionChip label="Installed" />}
            </AppCard>
          ))}
        </div>
      )}
      {filteredApps.length > pageSize && (
        <div className="flex items-center justify-center gap-2 text-sm">
          <button
            onClick={() => setCurrentPage(currentPage - 1)}
            disabled={currentPage === 1}
            className="clear thin"
          >
            <FaChevronLeft className="text-xl" />
          </button>
          <span>Page {currentPage} of {Math.ceil(filteredApps.length / pageSize)}</span>
          <span className="opacity-50 mx-2">|</span>
          <select
            value={pageSize}
            onChange={(e) => onPageSizeChange(e)}
            className="clear thin text-gray-500"
          >
            <option value={10}>10</option>
            <option value={20}>20</option>
            <option value={50}>50</option>
            <option value={100}>100</option>
          </select>
          <span className="opacity-50">per page</span>
          <button
            onClick={() => setCurrentPage(currentPage + 1)}
            disabled={currentPage === Math.ceil(filteredApps.length / pageSize)}
            className="clear thin"
          >
            <FaChevronRight className="text-xl" />
          </button>
        </div>
      )}
      <div className="flex items-center justify-center text-center gap-2">
        <p className="text-xs">Can't find the app?</p>
        <ResetButton className="thin clear !text-red-500 !text-xs" />
      </div>
    </div>
  );
}

const ActionChip: React.FC<{
  label: string;
  className?: string;
}> = ({ label, className }) => {
  return <div
    className={classNames("bg-iris/10 text-iris dark:bg-black dark:text-neon font-bold px-3 py-1 rounded-full flex items-center gap-2", className)}>{label}
  </div>
}
