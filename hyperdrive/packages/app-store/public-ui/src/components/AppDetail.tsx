import React, { useEffect, useState } from "react";
import { useParams } from "react-router-dom";
import type { AppListing } from "../types/app";
import { FaChevronDown, FaChevronRight } from "react-icons/fa6";

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

    useEffect(() => {
        const loadApp = async () => {
            if (!id) return;
            setIsLoading(true);
            setError(null);

            try {
                if (isDevMode) {
                    setApp(mockApp);
                } else {
                    const response = await fetch(`/main:app-store:sys/apps-public/${id}`);
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
    }, [id, isDevMode]);

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
                <div>App details not found for {id}</div>
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
            {/* App Header */}
            <div className="flex justify-between gap-4 flex-wrap">
                <div className="w-16 md:w-32 h-16 md:h-32 flex items-center justify-center rounded-lg">
                    {app.metadata?.image ? (
                        <img
                            src={app.metadata.image}
                            alt={`${app.metadata?.name || app.package_id.package_name} icon`}
                            className="w-16 md:w-32 h-16 md:h-32 object-cover rounded-lg aspect-square bg-white dark:bg-black"
                        />
                    ) : (
                        <div className="w-16 md:w-32 h-16 md:h-32 rounded-lg aspect-square bg-blue-500 flex items-center justify-center">
                            <span className="text-white font-bold text-2xl md:text-4xl">
                                {app.package_id.package_name.charAt(0).toUpperCase()}
                            </span>
                        </div>
                    )}
                </div>

                {/* App Info Grid */}
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

            {/* App Title */}
            <div className="flex items-center justify-between gap-2 flex-wrap">
                <h1 className="text-2xl md:text-3xl font-bold text-gray-900 dark:text-white">
                    {app.metadata?.name || app.package_id.package_name}
                </h1>
            </div>

            {/* App Description */}
            <div className="prose prose-gray dark:prose-invert max-w-none">
                <p className="text-gray-700 dark:text-gray-300 leading-relaxed">
                    {app.metadata?.description || "No description available"}
                </p>
            </div>

            {/* Public Notice */}
            <div className="bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg p-4">
                <p className="text-sm text-blue-700 dark:text-blue-300">
                    This is a public view of the app. To install and use this app, access the private app store.
                </p>
            </div>

            {/* Screenshots */}
            {(hasScreenshots || isDevMode) && (
                <div className="flex flex-col gap-4">
                    <button
                        onClick={() => setShowScreenshots(!showScreenshots)}
                        className="flex items-center gap-2 text-lg font-medium text-gray-900 dark:text-white hover:text-blue-600 dark:hover:text-blue-400 transition-colors"
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

            {/* Technical Details */}
            <div className="bg-gray-50 dark:bg-gray-800 rounded-lg p-4">
                <h3 className="text-lg font-medium text-gray-900 dark:text-white mb-3">
                    Technical Details
                </h3>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3 text-sm">
                    <div>
                        <span className="text-gray-600 dark:text-gray-400">Package ID:</span>
                        <p className="font-mono break-all">
                            {app.package_id.package_name}:{app.package_id.publisher_node}
                        </p>
                    </div>

                    <div>
                        <span className="text-gray-600 dark:text-gray-400">Metadata Hash:</span>
                        <p className="font-mono break-all">{app.metadata_hash}</p>
                    </div>

                    {app.metadata?.properties?.mirrors && app.metadata.properties.mirrors.length > 0 && (
                        <div className="md:col-span-2">
                            <span className="text-gray-600 dark:text-gray-400">Available Mirrors:</span>
                            <p className="text-gray-700 dark:text-gray-300">
                                {app.metadata.properties.mirrors.length} mirror{app.metadata.properties.mirrors.length !== 1 ? 's' : ''} available
                            </p>
                        </div>
                    )}

                    {app.metadata?.properties?.code_hashes && app.metadata.properties.code_hashes.length > 0 && (
                        <div className="md:col-span-2">
                            <span className="text-gray-600 dark:text-gray-400">Available Versions:</span>
                            <p className="text-gray-700 dark:text-gray-300">
                                {app.metadata.properties.code_hashes.length} version{app.metadata.properties.code_hashes.length !== 1 ? 's' : ''} available
                            </p>
                        </div>
                    )}
                </div>
            </div>

            {/* Footer */}
            <div className="text-center py-4 border-t border-gray-200 dark:border-gray-700">
                <p className="text-sm text-gray-600 dark:text-gray-400">
                    Part of the Hyperdrive ecosystem
                </p>
            </div>
        </div>
    );
}