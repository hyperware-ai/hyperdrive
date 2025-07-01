import classNames from "classnames";
import React, { ReactNode } from "react";
import { FaX } from "react-icons/fa6";

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
    return (
        <div className={classNames("fixed inset-0 bg-black/10 flex items-center justify-center z-50", backdropClassName)}>
            <div className={classNames("bg-white dark:bg-stone shadow-lg dark:shadow-white/10 p-4 rounded-lg relative max-w-md max-h-screen overflow-y-auto flex flex-col items-stretch gap-4", modalClassName)}    >
                <button className="absolute top-2 right-2 hover:scale-110 hover:text-red-500 transition-all duration-200" onClick={onClose}>
                    <FaX />
                </button>
                {children}
            </div>
        </div>
    );
};