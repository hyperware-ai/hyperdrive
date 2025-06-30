import React, { ReactNode, useState } from 'react';
import { FaBell, FaChevronDown, FaChevronUp, FaTrash, FaTimes } from 'react-icons/fa';
import useAppsStore from '../store';
import { Notification, NotificationAction } from '../types/Apps';
import { useNavigate } from 'react-router-dom';
import classNames from 'classnames';

interface ModalProps {
    children: ReactNode;
    onClose: () => void;
}

const Modal: React.FC<ModalProps> = ({ children, onClose }) => {
    return (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-black/50 p-4 rounded-lg relative max-w-md max-h-screen overflow-y-auto">
                <button className="modal-close" onClick={onClose}>
                    <FaTimes />
                </button>
                {children}
            </div>
        </div>
    );
};

const NotificationBay: React.FC = () => {
    const { notifications, removeNotification } = useAppsStore();
    const hasErrors = notifications.some(n => n.type === 'error');
    const [isExpanded, setIsExpanded] = useState(false);
    const [modalContent, setModalContent] = useState<React.ReactNode | null>(null);
    const navigate = useNavigate();

    const handleActionClick = (action: NotificationAction) => {
        switch (action.action.type) {
            case 'modal':
                const content = typeof action.action.modalContent === 'function'
                    ? action.action.modalContent()
                    : action.action.modalContent;
                setModalContent(content);
                break;
            case 'click':
                action.action.onClick?.();
                break;
            case 'redirect':
                if (action.action.path) {
                    navigate(action.action.path);
                }
                break;
        }
    };

    const handleDismiss = (notificationId: string, event: React.MouseEvent) => {
        event.stopPropagation(); // Prevent event bubbling
        removeNotification(notificationId);
    };


    return (
        <>
            <div className={classNames("relative rounded-md p-2 z-50")}>
                <button
                    onClick={() => setIsExpanded(!isExpanded)}
                    className={`notification-button ${hasErrors ? 'has-errors' : ''}`}
                >
                    <FaBell />
                    {notifications.length > 0 && (
                        <span className={`badge ${hasErrors ? 'error-badge' : ''}`}>
                            {notifications.length}
                        </span>
                    )}
                </button>

                {isExpanded && (
                    <div className="absolute top-full right-0 w-md max-h-md overflow-y-auto bg-white dark:bg-black rounded-md shadow-md z-50 p-2 flex flex-col gap-2 items-stretch">
                        {notifications.length === 0 ? (
                            <p>All clear, no notifications!</p>
                        ) : (
                            notifications.map(notification => (
                                <NotificationItem
                                    key={notification.id}
                                    notification={notification}
                                    handleActionClick={handleActionClick}
                                    handleDismiss={handleDismiss}
                                />
                            ))
                        )}
                    </div>
                )}
            </div>

            {modalContent && (
                <Modal onClose={() => setModalContent(null)}>
                    {modalContent}
                </Modal>
            )}
        </>
    );
};

export function NotificationItem({
    notification,
    handleActionClick,
    handleDismiss
}: {
    notification: Notification,
    handleActionClick: (action: NotificationAction) => void,
    handleDismiss: (notificationId: string, event: React.MouseEvent) => void
}) {
    return (
        <div key={notification.id} className={`notification-item ${notification.type}`}>
            {notification.renderContent ? (
                notification.renderContent(notification)
            ) : (
                <>
                    <div className="notification-content">
                        <p>{notification.message}</p>
                        {notification.type === 'download' && notification.metadata?.progress && (
                            <div className=" mt-2 h-1 bg-white dark:bg-black rounded overflow-hidden">
                                <div
                                    className="h-full bg-blue dark:bg-neon transition-width duration-300"
                                    style={{ width: `${notification.metadata.progress}%` }}
                                />
                            </div>
                        )}
                    </div>

                    {notification.actions && (
                        <div className="notification-actions">
                            {notification.actions.map((action, index) => (
                                <button
                                    key={index}
                                    onClick={() => handleActionClick(action)}
                                    className={`action-button ${action.variant || 'secondary'}`}
                                >
                                    {action.icon && <action.icon />}
                                    {action.label}
                                </button>
                            ))}
                        </div>
                    )}

                    {!notification.persistent && (
                        <button
                            className="clear"
                            onClick={(e) => handleDismiss(notification.id, e)}
                        >
                            <FaTrash />
                        </button>
                    )}
                </>
            )}
        </div>
    );
};
export default NotificationBay;