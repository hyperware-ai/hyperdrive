import classNames from "classnames";
import React, { ReactNode } from "react";
import { BsX } from "react-icons/bs";

interface ModalProps {
    children: ReactNode;
    onClose: () => void;
    backdropClassName?: string;
    modalClassName?: string;
    title?: string;
}

export const Modal: React.FC<ModalProps> = ({
    children,
    backdropClassName,
    modalClassName,
    onClose,
    title
}) => {
    return (
        <div className={classNames("fixed inset-0 backdrop-blur-sm bg-black/10 dark:bg-black/50 flex items-center justify-center z-50 animate-modal-backdrop", backdropClassName)}>
            <div className={classNames("bg-white dark:bg-black shadow-lg dark:shadow-white/10 p-4 rounded-lg relative w-full max-w-screen md:max-w-md min-h-0 max-h-screen overflow-y-auto flex flex-col items-stretch gap-4 animate-modal-content", modalClassName)}>
                <div className="flex items-center justify-between">
                    {title && <h2 className="text-lg font-bold prose">{title}</h2>}
                <button
                    className="clear thin"
                    onClick={onClose}>
                    <BsX className="text-2xl" />
                </button>
                </div>
                {children}
            </div>
        </div>
    );
};