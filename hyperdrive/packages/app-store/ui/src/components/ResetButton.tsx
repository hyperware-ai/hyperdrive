import React, { useState } from 'react';
import { FaExclamationTriangle } from 'react-icons/fa';
import useAppsStore from '../store';
import { BsArrowClockwise } from 'react-icons/bs';
import { Modal } from './Modal';
import classNames from 'classnames'

interface ResetButtonProps {
    className?: string
}
const ResetButton: React.FC<ResetButtonProps> = ({ className }) => {
    const resetStore = useAppsStore(state => state.resetStore);
    const [isOpen, setIsOpen] = useState(false);
    const [isLoading, setIsLoading] = useState(false);

    const handleReset = async () => {
        try {
            setIsLoading(true);
            await resetStore();
            setIsOpen(false);
        } catch (error) {
            console.error('Reset failed:', error);
            alert('Failed to reset the app store. Please try again.');
        } finally {
            setIsLoading(false);
        }
    };

    return (
        <>
            <button
                onClick={() => setIsOpen(true)}
                className={classNames(className, {
                    '!bg-red-500 !text-white grow md:grow-0 self-stretch md:self-center ': !className
                })}
            >
                <span>Reset store</span>
            </button>

            {isOpen && (
                <Modal onClose={() => setIsOpen(false)}>
                    <div className="flex items-center gap-2 mb-2">
                        <FaExclamationTriangle size={24} className="text-red-500" />
                        <h3 className="prose font-bold">Warning</h3>
                    </div>

                    <p className="text-sm">
                        This action will re-index all apps and reset the store state.
                    </p>

                    <div className="flex items-center gap-2">
                        <button
                            onClick={() => setIsOpen(false)}
                            className="button clear grow self-stretch"
                        >
                            Cancel
                        </button>
                        <button
                            onClick={handleReset}
                            disabled={isLoading}
                            className="button grow self-stretch"
                        >
                            {isLoading ? 'Resetting...' : 'Reset Store'}
                        </button>
                    </div>
                </Modal>
            )}
        </>
    );
};

export default ResetButton;
