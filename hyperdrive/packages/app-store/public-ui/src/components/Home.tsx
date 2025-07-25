import React, { useState, useEffect } from "react";
import type { AppListing } from "../types/app";
import { AppCard } from "./AppCard";
import { BsSearch } from "react-icons/bs";
import { FaChevronLeft, FaChevronRight } from "react-icons/fa6";

export default function Home() {
    const [listings, setListings] = useState<AppListing[]>([]);
    const [loading, setLoading] = useState<boolean>(true);
    const [error, setError] = useState<string>("");
    const [searchQuery, setSearchQuery] = useState<string>("");
    const [currentPage, setCurrentPage] = useState<number>(1);
    const [pageSize, setPageSize] = useState<number>(10);

    const onInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
        setSearchQuery(e.target.value);
        setCurrentPage(1); // Reset to first page when searching
    };

    const onPageSizeChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
        const newPageSize = parseInt(e.target.value);
        setPageSize(newPageSize);
        setCurrentPage(1); // Reset to first page when changing page size
    };

    useEffect(() => {
        const fetchApps = async () => {
            try {
                setLoading(true);
                const response = await fetch('/main:app-store:sys/apps-public');
                if (!response.ok) {
                    throw new Error('Failed to fetch apps');
                }
                const data = await response.json();
                setListings(data || []);
            } catch (err) {
                setError(err instanceof Error ? err.message : 'Failed to load apps');
            } finally {
                setLoading(false);
            }
        };

        fetchApps();
    }, []);

    const filteredApps = React.useMemo(() => {
        if (!Array.isArray(listings)) return [];
        return listings.filter((app) => {
            if (!app || !app.package_id) return false;
            const nameMatch = app.package_id.package_name.toLowerCase().includes(searchQuery.toLowerCase());
            const descMatch = app.metadata?.description?.toLowerCase().includes(searchQuery.toLowerCase()) || false;
            const publisherMatch = app.package_id.publisher_node.toLowerCase().includes(searchQuery.toLowerCase());
            return nameMatch || descMatch || publisherMatch;
        });
    }, [listings, searchQuery]);

    const totalPages = Math.ceil(filteredApps.length / pageSize);
    const paginatedApps = filteredApps.slice(
        (currentPage - 1) * pageSize,
        currentPage * pageSize
    );

    if (loading) {
        return (
            <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-center justify-center min-h-96">
                <div className="text-lg">Loading apps...</div>
            </div>
        );
    }

    if (error) {
        return (
            <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-center justify-center min-h-96">
                <div className="text-red-500">Error: {error}</div>
            </div>
        );
    }

    return (
        <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-stretch gap-4">
            <div className="flex items-center self-stretch gap-2 items-center bg-black/10 dark:bg-white/10 rounded-lg pl-4">
                <span className="text-xl opacity-50"><BsSearch /></span>
                <input
                    type="text"
                    placeholder="Search apps..."
                    value={searchQuery}
                    onChange={onInputChange}
                    className="grow text-sm !bg-transparent border-none outline-none p-3"
                />
            </div>

            {filteredApps.length === 0 ? (
                <div className="text-center py-8">
                    <p className="text-gray-600 dark:text-gray-400">
                        {searchQuery ? 'No apps match your search.' : 'No apps available.'}
                    </p>
                </div>
            ) : (
                <>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                        {paginatedApps.map((app) => (
                            <AppCard
                                key={`${app.package_id?.package_name}:${app.package_id?.publisher_node}`}
                                app={app}
                            />
                        ))}
                    </div>

                    {totalPages > 1 && (
                        <div className="flex items-center justify-center gap-2 text-sm">
                            <button
                                onClick={() => setCurrentPage(currentPage - 1)}
                                disabled={currentPage === 1}
                                className={`flex items-center justify-center p-2 rounded ${currentPage === 1
                                    ? "opacity-50 cursor-not-allowed"
                                    : "hover:bg-gray-100 dark:hover:bg-gray-700"
                                    }`}
                            >
                                <span className="text-xl"><FaChevronLeft /></span>
                            </button>
                            <span>Page {currentPage} of {totalPages}</span>
                            <span className="opacity-50 mx-2">|</span>
                            <select
                                value={pageSize}
                                onChange={onPageSizeChange}
                                className="bg-transparent border border-gray-300 dark:border-gray-600 rounded px-2 py-1 text-sm"
                            >
                                <option value={10}>10</option>
                                <option value={20}>20</option>
                                <option value={50}>50</option>
                                <option value={100}>100</option>
                            </select>
                            <span className="opacity-50">per page</span>
                            <button
                                onClick={() => setCurrentPage(currentPage + 1)}
                                disabled={currentPage === totalPages}
                                className={`flex items-center justify-center p-2 rounded ${currentPage === totalPages
                                    ? "opacity-50 cursor-not-allowed"
                                    : "hover:bg-gray-100 dark:hover:bg-gray-700"
                                    }`}
                            >
                                <span className="text-xl"><FaChevronRight /></span>
                            </button>
                        </div>
                    )}
                </>
            )}

            <div className="text-center py-4">
                <p className="text-sm text-gray-600 dark:text-gray-400">
                    Browse available apps in the Hyperdrive ecosystem
                </p>
            </div>
        </div>
    );
}
