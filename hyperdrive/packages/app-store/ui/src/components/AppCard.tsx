import { AppListing } from "../types/Apps";
import React from "react";
import { useNavigate } from "react-router-dom";

export const AppCard: React.FC<{ app: AppListing }> = ({ app }) => {
    if (!app || !app.package_id) return null;
    const navigate = useNavigate();

    return (
        <div
            className="
                app-card
                p-4 rounded-lg hover:bg-black/10 dark:hover:bg-white/10 shadow-md
                transition-colors duration-200 
                flex flex-col items-center gap-2
                cursor-pointer
            "
            onClick={() => {
                navigate(`/app/${app.package_id.package_name}:${app.package_id.publisher_node}`);
            }}
        >
            <div className="flex flex-wrap flex-col md:flex-row gap-2">
                <img
                    src={app.metadata?.image || '/h-green.svg'}
                    alt={`${app.metadata?.name || app.package_id.package_name} icon`}
                    className="w-24 h-24 object-cover rounded-lg aspect-square bg-white dark:bg-black p-2"
                />
                <div className="flex flex-col items-center md:items-start gap-2  lg:max-w-1/2">
                    <h3 className="break-words break-all max-w-full text-wrap overflow-hidden">
                        {app.metadata?.name || app.package_id.package_name}
                    </h3>
                    <p>
                        {app.package_id.publisher_node}
                    </p>
                </div>
            </div>
            {app.metadata?.description && (
                <p className="text-center">
                    {app.metadata.description.length > 100
                        ? `${app.metadata.description.substring(0, 100)}...`
                        : app.metadata.description}
                </p>
            )}
        </div>
    );
};