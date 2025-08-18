import React, { useState, useEffect } from "react";
import useAppsStore from "../store";
import { AppListing } from "../types/Apps";
import { FaChevronLeft, FaChevronRight } from "react-icons/fa6";
import { ResetButton } from "../components";
import { AppCard } from "../components/AppCard";
import { BsSearch } from "react-icons/bs";
import classNames from "classnames";
import { useLocation, useNavigate } from "react-router-dom";

export default function StorePage() {
  const { listings, installed, fetchListings, fetchInstalled, fetchUpdates, fetchHomepageApps, getLaunchUrl, navigateToApp } = useAppsStore();
  const [searchQuery, setSearchQuery] = useState<string>("");
  const [launchableApps, setLaunchableApps] = useState<AppListing[]>([]);
  const [appsNotInstalled, setAppsNotInstalled] = useState<AppListing[]>([]);
  const [currentPage, setCurrentPage] = useState<number>(1);
  const [pageSize, setPageSize] = useState<number>(10);

  // if we have ?search=something, set the search query to that
  const location = useLocation();
  const navigate = useNavigate();
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
                ? <ActionChip
                  label="Install"
                  onClick={() => navigate(`/app/${app.package_id.package_name}:${app.package_id.publisher_node}?intent=install`)}
                />
                : launchableApps.includes(app)
                  ? <ActionChip
                    label="Launch"
                    onClick={() => navigateToApp(`${app.package_id.package_name}:${app.package_id.publisher_node}`)}
                  />
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
  onClick?: () => void;
}> = ({ label, className, onClick }) => {
  return <div
    onClick={onClick}
    data-action-button={!!onClick}
    className={classNames("bg-iris/10 text-iris dark:bg-black dark:text-neon font-bold px-3 py-1 rounded-full flex items-center gap-2", {
      'cursor-pointer hover:opacity-80': onClick,
    }, className)}>{label}
  </div>
}
