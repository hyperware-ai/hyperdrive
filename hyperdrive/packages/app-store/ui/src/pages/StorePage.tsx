import React, { useState, useEffect } from "react";
import useAppsStore from "../store";
import { AppListing } from "../types/Apps";
import { FaSearch } from "react-icons/fa";
import { ResetButton } from "../components";
import { AppCard } from "../components/AppCard";
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
  const { listings, fetchListings, fetchUpdates } = useAppsStore();
  const [searchQuery, setSearchQuery] = useState<string>("");

  useEffect(() => {
    fetchListings();
    fetchUpdates();
  }, [fetchListings]);

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
    <div className="store-page">
      <div className="store-header">
        <div className="search-bar">
          <input
            type="text"
            placeholder="Search apps..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          <FaSearch />
        </div>
      </div>
      {!listings ? (
        <p>Loading...</p>
      ) : filteredApps.length === 0 ? (
        <p>No apps available.</p>
        // <div
        //   className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4"
        // >
        //   {mockApps.map((app) => (
        //     <AppCard key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`} app={app} />
        //   ))}
        // </div>
      ) : (
        <div
          className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4"
        >
          {filteredApps.map((app) => (
            <AppCard key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`} app={app} />
          ))}
        </div>
      )}
      <div className="flex flex-col items-center justify-center text-center gap-4">
        <p>Can't find the app you're looking for?</p>
        <ResetButton />
      </div>
    </div>
  );
}
