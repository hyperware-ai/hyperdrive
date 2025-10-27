import React, { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import type { AppListing } from "../types/app";
import { FaChevronDown, FaChevronRight } from "react-icons/fa6";
import classNames from "classnames";

const mockApp: AppListing = {
    package_id: {
        package_name: 'mock-app',
        publisher_node: 'mock-node'
    },
    metadata: {
        name: 'Mock App with an Unreasonably Long Name for Testing Wrapping, Obviously, why else would you have a name this long?',
        description: `This is a mock app. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page. I have written an incredibly long description to test the app page.`,
        image: 'https://via.placeholder.com/150',
        properties: {
            code_hashes: [['1.0.0', '1234567890']],
            package_name: 'mock-app',
            publisher: 'mock-node',
            current_version: '1.0.0',
            mirrors: ['https://mock-mirror.com'],
            screenshots: ['https://via.placeholder.com/300x200', 'https://via.placeholder.com/300x200', 'https://via.placeholder.com/300x200']
        }
    },
    tba: '0x0000000000000000000000000000000000000000',
    metadata_uri: 'https://mock-metadata.com',
    metadata_hash: '1234567890',
    auto_update: false
};

export default function AppDetail() {
    const { id } = useParams();
    const [app, setApp] = useState<AppListing | null>(null);
    const [isLoading, setIsLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [showScreenshots, setShowScreenshots] = useState(false);
    const [isDevMode, setIsDevMode] = useState(false);
    const [backtickPressCount, setBacktickPressCount] = useState(0);
    const [detailExpanded, setDetailExpanded] = useState(false);
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

    const derivedId = React.useMemo(() => {
        if (id) return id;
        const match = window.location.pathname.match(/\/app\/([^/?#]+)/);
        if (match && match[1]) {
            return decodeURIComponent(match[1]);
        }
        return undefined;
    }, [id]);

    useEffect(() => {
        const loadApp = async () => {
            if (!derivedId) {
                setIsLoading(false);
                return;
            }
            setIsLoading(true);
            setError(null);

            try {
                if (isDevMode) {
                    setApp(mockApp);
                } else {
                    const encodedId = encodeURIComponent(derivedId).replace(/%3A/g, ":");
                    const response = await fetch(`/main:app-store:sys/apps-public/${encodedId}`);
                    if (!response.ok) {
                        throw new Error('Failed to fetch app details');
                    }
                    const appData = await response.json();
                    setApp(appData);
                }
            } catch (err) {
                setError("Failed to load app details. Please try again.");
                console.error(err);
            } finally {
                setIsLoading(false);
            }
        };

        loadApp();
        window.scrollTo(0, 0);
    }, [derivedId, isDevMode]);

    if (isLoading) {
        return (
            <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-center justify-center min-h-96">
                <div className="text-lg">Loading app details...</div>
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

    if (!app) {
        return (
            <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-center justify-center min-h-96">
                <div>App details not found for {derivedId ?? "unknown app"}</div>
            </div>
        );
    }

    const latestVersion = app.metadata?.properties?.code_hashes?.length > 0
        ? app.metadata.properties.code_hashes[0][0]
        : null;

    const hasScreenshots = (app.metadata?.properties?.screenshots &&
        app.metadata.properties.screenshots.length > 0) || isDevMode;

    return (
        <div className="max-w-screen md:max-w-screen-md mx-auto flex flex-col items-stretch gap-4">

            <div className="flex justify-between gap-4 flex-wrap">
                <div className="w-16 md:w-32 h-16 md:h-32 flex items-center justify-center rounded-lg">
                    {app.metadata?.image ? (
                        <img
                            src={app.metadata.image}
                            alt={`${app.metadata?.name || app.package_id.package_name} icon`}
                            className="w-16 md:w-32 h-16 md:h-32 object-cover rounded-lg aspect-square bg-white dark:bg-black"
                        />
                    ) : (
                        <div className="w-16 md:w-32 h-16 md:h-32 rounded-lg aspect-square bg-iris dark:bg-neon flex items-center justify-center">
                            <span className="text-white font-bold text-2xl md:text-4xl">
                                {app.package_id.package_name.charAt(0).toUpperCase() + (app.package_id.package_name.charAt(1) || '').toLowerCase()}
                            </span>
                        </div>
                    )}
                </div>


                <div className="grid grid-cols-2 gap-2 flex-1">
                    <span className="text-gray-600 dark:text-gray-400">Publisher:</span>
                    <span className="font-medium text-blue-600 dark:text-blue-400">
                        {app.package_id.publisher_node}
                    </span>

                    {latestVersion && (
                        <>
                            <span className="text-gray-600 dark:text-gray-400">Version:</span>
                            <span className="font-medium">{latestVersion}</span>
                        </>
                    )}

                    {app.metadata?.properties?.license && (
                        <>
                            <span className="text-gray-600 dark:text-gray-400">License:</span>
                            <span className="font-medium">{app.metadata.properties.license}</span>
                        </>
                    )}

                    <span className="text-gray-600 dark:text-gray-400">Package:</span>
                    <span className="font-mono text-sm">{app.package_id.package_name}</span>
                </div>
            </div>


            <div className="flex items-center justify-between gap-2 flex-wrap">
                <h1 className="text-2xl md:text-3xl font-bold text-black prose dark:text-white">
                    {app.metadata?.name || app.package_id.package_name}
                </h1>
                <div className="flex self-stretch items-center gap-2 max-w-sm">
                    <span className="text-sm opacity-50">To use this app:</span>
                    <a
                        href={`https://valet.hyperware.ai/?installApp=${app?.package_id?.package_name}:${app?.package_id?.publisher_node}`}
                        className=" button thin text-sm"
                    >
                        <span>Get a node</span>
                    </a>
                    <a
                        href="https://book.hyperware.ai/getting_started/install.html"
                        className="text-sm button thin clear"
                    >
                        <span className="text-black dark:text-white opacity-50">or host your own</span>
                    </a>
                </div>
            </div>


            <div className="prose prose-gray dark:prose-invert max-w-none">
                <p className="opacity-50 leading-relaxed">
                    {app.metadata?.description || "No description available"}
                </p>
            </div>

            {(hasScreenshots || isDevMode) && (
                <div className="flex flex-col gap-4">
                    <button
                        onClick={() => setShowScreenshots(!showScreenshots)}
                        className="flex items-center gap-2 text-lg font-medium text-gray-900 dark:text-white hover:text-iris dark:hover:text-neon transition-colors"
                    >
                        {showScreenshots ? <FaChevronDown /> : <FaChevronRight />}
                        Screenshots
                    </button>

                    {showScreenshots && (
                        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
                            {(isDevMode ? mockApp.metadata!.properties!.screenshots! : app.metadata?.properties?.screenshots || []).map((screenshot: string, index: number) => (
                                <img
                                    key={index}
                                    src={screenshot}
                                    alt={`Screenshot ${index + 1}`}
                                    className="rounded-lg w-full h-auto object-cover aspect-video max-h-64 hover:scale-105 transition-transform cursor-pointer"
                                    loading="lazy"
                                    onClick={() => window.open(screenshot, '_blank')}
                                />
                            ))}
                        </div>
                    )}
                </div>
            )}

            <div className={classNames("bg-black/10 dark:bg-white/10 rounded-lg p-4 flex flex-col transition-all duration-300", {
                'gap-4': detailExpanded,
                'gap-0': !detailExpanded
            })}>
                <h3 className="text-lg font-medium text-black dark:text-white prose flex gap-2 items-center">
                    <button
                        className="thin clear"
                        onClick={() => setDetailExpanded(!detailExpanded)}
                    >
                        {detailExpanded ? <FaChevronDown /> : <FaChevronRight />}
                    </button>
                    <span>Technical Details</span>
                </h3>
                <div className={classNames("grid grid-cols-1 md:grid-cols-2 gap-3 text-sm transition-all duration-300", {
                    "opacity-0 max-h-0 overflow-hidden": !detailExpanded,
                    "opacity-100 max-h-96 overflow-visible": detailExpanded
                })}>
                    <div>
                        <span className="opacity-75">Package ID:</span>
                        <p className="font-mono break-all">
                            {app.package_id.package_name}:{app.package_id.publisher_node}
                        </p>
                    </div>

                    <div>
                        <span className="opacity-75">Metadata Hash:</span>
                        <p className="font-mono break-all">{app.metadata_hash}</p>
                    </div>

                    {app.metadata?.properties?.mirrors && app.metadata.properties.mirrors.length > 0 && (
                        <div className="md:col-span-2">
                            <span className="opacity-75">Available Mirrors:</span>
                            <p className="opacity-50">
                                {app.metadata.properties.mirrors.length} mirror{app.metadata.properties.mirrors.length !== 1 ? 's' : ''} available
                            </p>
                        </div>
                    )}

                    {app.metadata?.properties?.code_hashes && app.metadata.properties.code_hashes.length > 0 && (
                        <div className="md:col-span-2">
                            <span className="opacity-75">Available Versions:</span>
                            <p className="opacity-50">
                                {app.metadata.properties.code_hashes.length} version{app.metadata.properties.code_hashes.length !== 1 ? 's' : ''} available
                            </p>
                        </div>
                    )}
                </div>
            </div>

        </div>
    );
}
