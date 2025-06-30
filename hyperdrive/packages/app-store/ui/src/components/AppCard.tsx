import { AppListing } from "../types/Apps";
import React from "react";
import { useNavigate } from "react-router-dom";

export const AppCard: React.FC<{ app: AppListing, children?: React.ReactNode }> = ({ app, children }) => {
    if (!app || !app.package_id) return null;
    const navigate = useNavigate();

    return (
        <div
            className="
                app-card
                p-2 rounded-lg hover:bg-black/10 dark:hover:bg-white/10 
                transition-colors duration-200 
                flex flex-col gap-2
                cursor-pointer
            "
            onClick={() => {
                navigate(`/app/${app.package_id.package_name}:${app.package_id.publisher_node}`);
            }}
        >
            <div className="flex grow self-stretch items-center gap-2">
                {app.metadata?.image && <img
                    src={app.metadata?.image}
                    alt={`${app.metadata?.name || app.package_id.package_name} icon`}
                    className="w-1/6 min-w-1/6 object-cover rounded-xl aspect-square bg-white dark:bg-black"
                />}
                {!app.metadata?.image && <div
                    className="w-1/6 min-w-1/6 object-cover rounded-xl aspect-square bg-neon"
                />}
                <div className="flex flex-col grow ">
                    <p className="font-bold prose wrap-anywhere max-w-full overflow-hidden leading-tight line-clamp-1">
                        {app.metadata?.name || app.package_id.package_name}
                    </p>
                    {app.metadata?.description && (
                        <p className="text-sm opacity-50 wrap-anywhere leading-tight line-clamp-2">
                            {app.metadata.description.length > 100
                                ? `${app.metadata.description.substring(0, 100)}...`
                                : app.metadata.description}
                        </p>
                    )}
                    <p className="text-xs opacity-50 wrap-anywhere leading-tight line-clamp-1">
                        {app.package_id.publisher_node}
                    </p>
                </div>
                <div className="flex gap-2 flex-col">
                    {children}
                </div>
            </div>
        </div>
    );
};