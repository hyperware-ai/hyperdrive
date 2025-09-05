import React, { useMemo } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { STORE_PATH, PUBLISH_PATH, MY_APPS_PATH, APP_PAGE_PATH, DOWNLOAD_PATH } from '../constants/path';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import NotificationBay from './NotificationBay';
import useAppsStore from '../store/appStoreStore';
import classNames from 'classnames';
import { BsLightning, BsLayers, BsCloudArrowUp } from 'react-icons/bs';
const Header: React.FC = () => {
    const isMobile = useMemo(() => window.innerWidth < 768, [window.innerWidth]);
    const location = useLocation();
    const { updates } = useAppsStore();
    const updateCount = Object.keys(updates || {}).length;
    const navigate = useNavigate();
    const appColonPublisher = useMemo(() => {
        const match = location.pathname.match(APP_PAGE_PATH);
        if (match) return location.pathname.replace(APP_PAGE_PATH, '');
        return null;
    }, [location.pathname]);

    const lesBoutons = <>
        <Link
            to={STORE_PATH}
            className={classNames('button text-sm md:text-base flex-col md:flex-row', {
                'max-md:!text-neon clear': location.pathname !== STORE_PATH
            })}
        >
            <BsLightning className="text-xl" />
            <span>Store</span></Link>
        <Link
            to={MY_APPS_PATH}
            className={classNames('button text-sm md:text-base flex-col md:flex-row relative', {
                'max-md:!text-neon clear': location.pathname !== MY_APPS_PATH
            })}
        >
            <BsLayers className="text-xl" />
            <span>My Apps</span>
            {updateCount > 0 && <span className="absolute -top-2 -right-2 bg-red-500 text-white rounded-full w-4 h-4 flex items-center justify-center text-xs">{updateCount}</span>}
        </Link>
        <Link
            to={PUBLISH_PATH}
            className={classNames('button text-sm md:text-base flex-col md:flex-row', {
                'max-md:!text-neon clear': location.pathname !== PUBLISH_PATH
            })}
        >
            <BsCloudArrowUp className="text-xl" />
            <span>Publish</span>
        </Link>
        {isMobile && <div className="text-xs md:text-base"><ConnectButton label={`Wallet`} /></div>}
    </>

    return (
        <header className={classNames("flex items-center justify-between gap-2 py-2  max-w-screen md:px-4  mx-auto mb-4 md:mb-6")}>
            <div className="prose flex items-center gap-2 grow">
                <Link
                    to={STORE_PATH}
                    className={classNames('font-bold !text-inherit text-lg md:text-xl', {
                        'opacity-50': location.pathname !== STORE_PATH,
                    })}>App Store</Link>
                {location.pathname !== STORE_PATH && <>
                    <span className="font-bold text-lg md:text-xl">/</span>
                    {location.pathname === MY_APPS_PATH && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold text-lg md:text-xl">My Apps</span>
                            <span className="text-xs opacity-50 dark:text-neon absolute -bottom-3 left-0 pointer-events-none">Manage installed apps</span>
                        </div>
                    </>}
                    {location.pathname === PUBLISH_PATH && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold text-lg md:text-xl">Publish</span>
                            <span className="text-xs opacity-50 dark:text-neon absolute -bottom-3 left-0 pointer-events-none">Publish an app to the store</span>
                        </div>
                    </>}
                    {location.pathname.includes(DOWNLOAD_PATH + '/') && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold text-lg md:text-xl">Download</span>
                        </div>
                    </>}
                    {appColonPublisher && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold text-lg md:text-xl">{appColonPublisher.split(':')?.[0] || 'app'}</span>
                            <span className="text-xs text-iris dark:text-neon absolute -bottom-3 left-0 pointer-events-none">{appColonPublisher.split(':')?.[1] || 'publisher'}</span>
                        </div>
                    </>}
                </>}
            </div>
            <nav className="hidden md:flex items-center gap-2 self-stretch flex-wrap">
                {lesBoutons}
            </nav>
            <nav className="fixed md:hidden bottom-0 left-0 right-0 p-2 bg-iris flex items-center gap-2 justify-center flex-wrap z-20">
                {lesBoutons}
            </nav>
            <div className="flex items-center ml-auto gap-1 md:gap-2 self-stretch">
                {!isMobile && <ConnectButton />}

                <NotificationBay />
            </div>
        </header>
    );
};

export default Header;