import React from 'react';
import { Link, useLocation } from 'react-router-dom';
import { STORE_PATH, PUBLISH_PATH, MY_APPS_PATH } from '../constants/path';
import { ConnectButton } from '@rainbow-me/rainbowkit';
import { FaChevronLeft } from "react-icons/fa";
import NotificationBay from './NotificationBay';
import useAppsStore from '../store';
import classNames from 'classnames';
const Header: React.FC = () => {
    const location = useLocation();
    const { updates } = useAppsStore();
    const updateCount = Object.keys(updates || {}).length;
    const isMobile = window.innerWidth < 768;

    return (
        <header className={classNames("flex items-center justify-between gap-2 flex-wrap py-2 px-4", { "flex-col": isMobile })}>
            <div className="flex items-center gap-2 self-stretch flex-wrap">
                <nav className="flex items-center gap-2 self-stretch flex-wrap">
                    <button
                        onClick={() => window.location.href = window.location.origin.replace('//app-store-sys.', '//') + '/'}
                        className="alt"
                    >
                        <FaChevronLeft />
                    </button>
                    <Link
                        to={STORE_PATH}
                        className={classNames('button', { clear: location.pathname !== STORE_PATH })}
                    >Store</Link>
                    <Link
                        to={MY_APPS_PATH}
                        className={classNames('button', { clear: location.pathname !== MY_APPS_PATH })}
                    >
                        My Apps
                        {updateCount > 0 && <span className="update-badge">{updateCount}</span>}
                    </Link>
                    <Link
                        to={PUBLISH_PATH}
                        className={classNames('button', { clear: location.pathname !== PUBLISH_PATH })}
                    >
                        Publish
                    </Link>
                </nav>
            </div>
            <div className="flex items-center gap-2 self-stretch">
                <ConnectButton />
                <NotificationBay />
            </div>
        </header>
    );
};

export default Header;