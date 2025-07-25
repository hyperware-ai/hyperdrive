import React from "react";
import type { AppListing } from "../types/app";

export const AppCard: React.FC<{
    app: AppListing;
    children?: React.ReactNode;
}> = ({ app, children }) => {
    return (
        <div className="bg-white dark:bg-gray-800 rounded-lg shadow-md p-4 border border-gray-200 dark:border-gray-700">
            <div className="flex items-start gap-3">
                {app.metadata?.image ? (
                    <img
                        src={app.metadata.image}
                        alt={app.metadata.name}
                        className="w-12 h-12 rounded-lg object-cover flex-shrink-0"
                    />
                ) : (
                    <div className="w-12 h-12 rounded-lg bg-blue-500 flex items-center justify-center flex-shrink-0">
                        <span className="text-white font-bold text-lg">
                            {app.package_id.package_name.charAt(0).toUpperCase()}
                        </span>
                    </div>
                )}
                <div className="flex-1 min-w-0">
                    <h3 className="font-semibold text-lg text-gray-900 dark:text-white truncate">
                        {app.metadata?.name || app.package_id.package_name}
                    </h3>
                    <p className="text-sm text-gray-600 dark:text-gray-400 mb-2">
                        by {app.package_id.publisher_node}
                    </p>
                    <p className="text-sm text-gray-700 dark:text-gray-300 line-clamp-3">
                        {app.metadata?.description}
                    </p>
                    {children && (
                        <div className="flex gap-2 mt-3">
                            {children}
                        </div>
                    )}
                </div>
            </div>
        </div>
    );
};