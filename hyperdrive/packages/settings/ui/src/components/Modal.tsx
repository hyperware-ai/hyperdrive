import classNames from "classnames";
import React, { ReactNode, useEffect } from "react";
import { BsX } from "react-icons/bs";

interface ModalProps {
    children: ReactNode;
    onClose: () => void;
    backdropClassName?: string;
    modalClassName?: string;
}

export const Modal: React.FC<ModalProps> = ({
    children,
    backdropClassName,
    modalClassName,
    onClose
}) => {
    useEffect(() => {
        window.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') {
                onClose();
            }
        });
    }, [onClose]);

    return (
        <div className={classNames("fixed inset-0 backdrop-blur-sm bg-black/10 dark:bg-black/50 flex items-center justify-center z-50", backdropClassName)}>
            <div className={classNames("bg-white dark:bg-black shadow-lg dark:shadow-white/10 p-4 rounded-lg relative w-full max-w-screen md:max-w-md min-h-0 max-h-screen overflow-y-auto flex flex-col items-stretch gap-4", modalClassName)}    >
                <button
                    className="clear thin absolute top-2 right-2 hover:scale-110 hover:text-red-500 transition-all duration-200"
                    onClick={onClose}>
                    <BsX />
                </button>
                {children}
            </div>
        </div>
    );
};