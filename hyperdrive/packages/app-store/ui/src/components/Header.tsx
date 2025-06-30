import React from 'react';
import { Link, useLocation } from 'react-router-dom';
import { STORE_PATH, PUBLISH_PATH, MY_APPS_PATH } from '../constants/path';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import NotificationBay from './NotificationBay';
import useAppsStore from '../store';
import classNames from 'classnames';
import { BsLightning, BsLayers, BsCloudArrowUp } from 'react-icons/bs';
const Header: React.FC = () => {
    const location = useLocation();
    const { updates } = useAppsStore();
    const updateCount = Object.keys(updates || {}).length;

    const lesBoutons = <>
        <Link
            to={STORE_PATH}
            className={classNames('button text-sm md:text-base flex-col md:flex-row', { clear: location.pathname !== STORE_PATH })}
        >
            <BsLightning className="text-xl" />
            <span>Store</span></Link>
        <Link
            to={MY_APPS_PATH}
            className={classNames('button text-sm md:text-base flex-col md:flex-row relative', { clear: location.pathname !== MY_APPS_PATH })}
        >
            <BsLayers className="text-xl" />
            <span>My Apps</span>
            {updateCount > 0 && <span className="absolute -top-2 -right-2 bg-red-500 text-white rounded-full w-4 h-4 flex items-center justify-center text-xs">{updateCount}</span>}
        </Link>
        <Link
            to={PUBLISH_PATH}
            className={classNames('button text-sm md:text-base flex-col md:flex-row', { clear: location.pathname !== PUBLISH_PATH })}
        >
            <BsCloudArrowUp className="text-xl" />
            <span>Publish</span>
        </Link>
    </>

    return (
        <header className={classNames("flex items-center justify-between gap-2 py-2 px-4 max-w-screen  mx-auto")}>
            <h1 className="prose flex items-center gap-2 !text-xl grow">
                <span className={classNames('font-bold', {
                    'opacity-50': location.pathname !== STORE_PATH,
                })}>App Store</span>
                {location.pathname !== STORE_PATH && <>
                    <span className="font-bold">/</span>
                    {location.pathname === MY_APPS_PATH && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold">My Apps</span>
                            <span className="text-xs opacity-50 dark:text-neon absolute -bottom-3 left-0 pointer-events-none">Manage installed apps</span>
                        </div>
                    </>}
                    {location.pathname === PUBLISH_PATH && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold">Publish</span>
                            <span className="text-xs opacity-50 dark:text-neon absolute -bottom-3 left-0 pointer-events-none">Publish an app to the store</span>
                        </div>
                    </>}
                </>}
            </h1>
            <nav className="hidden md:flex items-center gap-2 self-stretch flex-wrap mx-auto">
                {lesBoutons}
            </nav>
            <nav className="fixed md:hidden bottom-0 left-0 right-0 p-2 bg-iris flex items-center gap-2 justify-center flex-wrap">
                {lesBoutons}
            </nav>
            <div className="flex items-center ml-auto  gap-2 self-stretch">
                <ConnectButton />
                <NotificationBay />
            </div>
        </header>
    );
};

export default Header;