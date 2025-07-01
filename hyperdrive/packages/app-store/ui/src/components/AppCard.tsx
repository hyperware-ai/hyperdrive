import classNames from "classnames";
import { AppListing } from "../types/Apps";
import React from "react";
import { useNavigate } from "react-router-dom";

export const AppCard: React.FC<{
    app: AppListing,
    children?: React.ReactNode,
    className?: string
}> = ({
    app,
    children,
    className
}) => {
        if (!app || !app.package_id) return null;
        const navigate = useNavigate();

        return (
            <div
                className={classNames(`
                app-card
                rounded-lg hover:bg-black/10 dark:hover:bg-white/10 
                transition-colors duration-200 
                flex flex-col gap-2
                cursor-pointer
            `, className)}
                onClick={() => {
                    navigate(`/app/${app.package_id.package_name}:${app.package_id.publisher_node}`);
                }}
            >
                <div className="flex grow self-stretch items-center gap-2">
                    {app.metadata?.image && <img
                        src={app.metadata?.image}
                        alt={`${app.metadata?.name || app.package_id.package_name} icon`}
                        className="mb-2 w-1/5 min-w-1/5 md:w-1/4 md:min-w-1/4 object-cover rounded-xl aspect-square bg-white dark:bg-black"
                    />}
                    {!app.metadata?.image && <div
                        className="mb-2 w-1/5 min-w-1/5 md:w-1/4 md:min-w-1/4 object-cover rounded-xl aspect-square bg-neon"
                    />}
                    <div className="flex flex-col grow self-stretch gap-1 border-b-1 border-black/10 dark:border-white/10 pb-2">
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
                        <p className="text-xs font-bold opacity-50 wrap-anywhere leading-tight line-clamp-1 mt-auto">
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