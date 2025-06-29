import React, { useState, useRef } from 'react';
import type { HomepageApp } from '../../../types/app.types';
import { usePersistenceStore } from '../../../stores/persistenceStore';
import { useAppStore } from '../../../stores/appStore';
import { Draggable } from './Draggable';

interface WidgetProps {
  app: HomepageApp;
}

export const Widget: React.FC<WidgetProps> = ({ app }) => {
  const { toggleWidget, widgetSettings, setWidgetPosition, setWidgetSize } = usePersistenceStore();
  const { isEditMode } = useAppStore();
  const [isLoading, setIsLoading] = useState(true);
  const [hasError, setHasError] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const resizeRef = useRef<HTMLDivElement>(null);

  const settings = widgetSettings[app.id] || {};
  if (settings.hide) return null;

  const position = settings.position || { x: 50, y: 50 };
  const size = settings.size || { width: 300, height: 200 };

  // Widgets can either have widget HTML content or be loaded from their app URL
  const isHtmlWidget = app.widget && app.widget !== 'true' && app.widget.includes('<');

  const handleError = () => {
    setHasError(true);
    setIsLoading(false);
  };

  const handleResize = (e: React.MouseEvent | React.TouchEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (!isEditMode) return;

    const isTouch = 'touches' in e;
    const startX = isTouch ? (e as React.TouchEvent).touches[0].clientX : (e as React.MouseEvent).clientX;
    const startY = isTouch ? (e as React.TouchEvent).touches[0].clientY : (e as React.MouseEvent).clientY;
    const startWidth = size.width;
    const startHeight = size.height;

    const handleMove = (clientX: number, clientY: number) => {
      // Calculate new size with minimum constraints
      const newWidth = Math.max(200, startWidth + clientX - startX);
      const newHeight = Math.max(150, startHeight + clientY - startY);

      // Ensure widget doesn't extend beyond screen boundaries
      const maxWidth = window.innerWidth - position.x;
      const maxHeight = window.innerHeight - position.y;

      const constrainedWidth = Math.min(newWidth, maxWidth);
      const constrainedHeight = Math.min(newHeight, maxHeight);

      setWidgetSize(app.id, { width: constrainedWidth, height: constrainedHeight });
    };

    const handleMouseMove = (e: MouseEvent) => handleMove(e.clientX, e.clientY);
    const handleTouchMove = (e: TouchEvent) => {
      e.preventDefault();
      handleMove(e.touches[0].clientX, e.touches[0].clientY);
    };

    const handleEnd = () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('mouseup', handleEnd);
      document.removeEventListener('touchend', handleEnd);
      setIsResizing(false);
    };

    setIsResizing(true);
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('touchmove', handleTouchMove, { passive: false });
    document.addEventListener('mouseup', handleEnd);
    document.addEventListener('touchend', handleEnd);
  };

  return (
    <Draggable
      id={`widget-${app.id}`}
      position={position}
      onMove={(pos) => setWidgetPosition(app.id, pos)}
      isEditMode={isEditMode}
      enableHtmlDrag={false}
    >
      <div
        className={`bg-black/80 backdrop-blur-xl rounded-2xl overflow-hidden shadow-2xl border border-white/20
          ${isEditMode ? 'ring-2 ring-blue-400' : ''}
          ${isResizing ? 'pointer-events-none' : ''}`}
        style={{ width: `${size.width}px`, height: `${size.height}px` }}
      >
        <div className="flex items-center justify-between bg-gradient-to-r from-blue-500/20 to-purple-500/20 px-3 py-2 border-b border-white/10">
          <span className="text-white/90 text-sm font-medium">{app.label}</span>
          {isEditMode && (
            <button
              onClick={(e) => {
                e.stopPropagation();
                toggleWidget(app.id);
              }}
              className="w-6 h-6 bg-red-500 text-white rounded-md flex items-center justify-center text-sm hover:bg-red-600 transition-colors"
            >
              ×
            </button>
          )}
        </div>

        <div className="relative w-full h-[calc(100%-40px)]">
          {isLoading && !isHtmlWidget && !hasError && (
            <div className="absolute inset-0 flex flex-col items-center justify-center bg-black/50">
              <div className="text-white/70 animate-pulse">
                <div className="w-8 h-8 border-2 border-white/30 border-t-white/70 rounded-full animate-spin mb-2"></div>
                <div className="text-sm">Loading...</div>
              </div>
            </div>
          )}

          {hasError ? (
            <div className="flex flex-col items-center justify-center h-full text-white/50 text-center p-4">
              <div className="text-3xl mb-2">⚠️</div>
              <div className="text-sm">Failed to load widget</div>
            </div>
          ) : isHtmlWidget ? (
            <iframe
              srcDoc={app.widget}
              className="w-full h-full bg-white"
              sandbox="allow-scripts allow-same-origin allow-forms allow-popups allow-modals"
              onLoad={() => setIsLoading(false)}
              onError={handleError}
            />
          ) : (
            <iframe
              src={app.path || `/app:${app.process}:${app.publisher}.os/`}
              className="w-full h-full bg-white"
              onLoad={() => setIsLoading(false)}
              onError={handleError}
              // Enhanced permissions for CORS
              allow="accelerometer; camera; encrypted-media; geolocation; gyroscope; microphone; midi; payment; usb; xr-spatial-tracking"
              // Minimal sandbox for widget functionality
              sandbox="allow-same-origin allow-scripts allow-forms allow-popups allow-popups-to-escape-sandbox allow-modals allow-downloads allow-presentation allow-top-navigation-by-user-activation"
            />
          )}
        </div>

        {isEditMode && (
          <div
            ref={resizeRef}
            className="absolute bottom-0 right-0 w-6 h-6 bg-blue-400 cursor-se-resize rounded-tl-lg touch-action-none"
            onMouseDown={handleResize}
            onTouchStart={handleResize}
            style={{ touchAction: 'none' }}
          />
        )}
      </div>
    </Draggable>
  );
};
