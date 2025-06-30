import React, { useState } from 'react';
import { FaExclamationTriangle } from 'react-icons/fa';
import useAppsStore from '../store';
import { Tooltip } from './Tooltip';
import { BsArrowClockwise } from 'react-icons/bs';

const ResetButton: React.FC = () => {
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
                className="button grow md:grow-0 self-stretch md:self-center !bg-red-500 !text-white"
            >
                <span>Reset Store</span>
                <BsArrowClockwise className="text-xl" />
            </button>

            {isOpen && (
                <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
                    onClick={() => setIsOpen(false)}>
                    <div
                        className="bg-black/50 p-4 rounded-lg relative max-w-md max-h-screen overflow-y-auto flex flex-col gap-2"
                        onClick={e => e.stopPropagation()}>
                        <button className="modal-close" onClick={() => setIsOpen(false)}>×</button>
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
                    </div>
                </div>
            )}
        </>
    );
};

export default ResetButton;
