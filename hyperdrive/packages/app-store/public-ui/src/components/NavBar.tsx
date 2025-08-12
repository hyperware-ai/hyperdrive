import React, { useMemo } from 'react';
import { Link, useLocation, } from 'react-router-dom';
import { STORE_PATH, APP_DETAILS_PATH } from '../constants/path';
import classNames from 'classnames';
import { BsLightning, } from 'react-icons/bs';
const NavBar: React.FC = () => {
    const location = useLocation();
    const appColonPublisher = useMemo(() => {
        const match = location.pathname.match(APP_DETAILS_PATH);
        if (match) return location.pathname.replace(APP_DETAILS_PATH, '');
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
    </>

    return (
        <header
            className={classNames("flex items-center justify-between gap-2 py-2  max-w-screen md:px-4  mx-auto mb-4 md:mb-6")}>
            <div className="prose flex items-center gap-2 grow">
                <Link
                    to={STORE_PATH}
                    className={classNames('font-bold !text-inherit text-lg md:text-xl', {
                        'opacity-50': location.pathname !== STORE_PATH,
                    })}>App Store</Link>
                {location.pathname !== STORE_PATH && <>
                    <span className="font-bold text-lg md:text-xl">/</span>
                    {appColonPublisher && <>
                        <div className="flex flex-col relative grow">
                            <span className="font-bold text-lg md:text-xl">{(appColonPublisher.split(':')?.[0] || 'app').replace(/^\//, '')}</span>
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
        </header>
    );
};

export default NavBar;