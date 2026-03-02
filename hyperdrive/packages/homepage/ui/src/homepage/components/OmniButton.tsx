import React, { useEffect, useState, useRef, useCallback } from 'react';
import { useNavigationStore } from '../stores/navigationStore';
import classNames from 'classnames';
import { usePersistenceStore } from '../stores/persistenceStore';
type DragStart = { x: number; y: number; buttonX: number; buttonY: number };

export const OmniButton: React.FC = () => {
  const { toggleRecentApps, isRecentAppsOpen, closeAllOverlays } = useNavigationStore();
  const { omnibuttonPosition, setOmnibuttonPosition } = usePersistenceStore();
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<DragStart | null>(null);
  const dragStartRef = useRef<DragStart | null>(null);
  const isDraggingRef = useRef(false);
  const dragThreshold = 5; // pixels - swipes smaller than this will be treated as taps
  const buttonRef = useRef<HTMLDivElement>(null);
  const mouseListenersActive = useRef(false);
  const isMobile = () => window.innerWidth < 768;

  const setDragging = (value: boolean) => {
    isDraggingRef.current = value;
    setIsDragging(value);
  };

  const setDragStartState = (value: DragStart | null) => {
    dragStartRef.current = value;
    setDragStart(value);
  };

  // Touch handlers for drag and tap
  const handleTouchStart = (e: React.TouchEvent) => {
    if (!isMobile()) return;
    console.log('omnibutton handleTouchStart', e);
    e.stopPropagation();
    const touch = e.touches[0];
    setDragStartState({
      x: touch.clientX,
      y: touch.clientY,
      buttonX: omnibuttonPosition.x,
      buttonY: omnibuttonPosition.y
    });
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    console.log('omnibutton handleTouchMove', e);
    const currentDragStart = dragStartRef.current;
    if (!currentDragStart) return;
    e.stopPropagation();

    const touch = e.touches[0];
    const deltaX = touch.clientX - currentDragStart.x;
    const deltaY = touch.clientY - currentDragStart.y;

    // Check if movement exceeds threshold to start dragging
    if (!isDraggingRef.current && (Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold)) {
      setDragging(true);
    }

    // Update position if dragging
    if (isDraggingRef.current || Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold) {
      const newX = Math.max(30, Math.min(window.innerWidth - 30, currentDragStart.buttonX + deltaX));
      const newY = Math.max(30, Math.min(window.innerHeight - 30, currentDragStart.buttonY + deltaY));
      setOmnibuttonPosition({ x: newX, y: newY });
    }
  };

  const handleTouchEnd = () => {
    if (!isMobile()) return;
    console.log('omnibutton handleTouchEnd');
    if (!isDraggingRef.current && dragStartRef.current) {
      // Tap - open recent apps
      if (!isRecentAppsOpen) toggleRecentApps();
      else closeAllOverlays();
    }
    setDragStartState(null);
    setDragging(false);
  };

  // Mouse handlers for desktop
  const handleMouseDown = (e: React.MouseEvent) => {
    if (isMobile()) return;
    console.log('omnibutton handleMouseDown', e);
    e.stopPropagation();
    if (!mouseListenersActive.current) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      mouseListenersActive.current = true;
    }
    setDragging(false);
    setDragStartState({
      x: e.clientX,
      y: e.clientY,
      buttonX: omnibuttonPosition.x,
      buttonY: omnibuttonPosition.y
    });
  };

  const handleMouseMove = useCallback((e: MouseEvent) => {
    if (isMobile()) return;
    console.log('omnibutton handleMouseMove', e);
    e.stopPropagation();
    const currentDragStart = dragStartRef.current;
    if (!currentDragStart) return;

    const deltaX = e.clientX - currentDragStart.x;
    const deltaY = e.clientY - currentDragStart.y;

    if (!isDraggingRef.current && (Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold)) {
      setDragging(true);
    }

    if (isDraggingRef.current || Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold) {
      const newX = Math.max(30, Math.min(window.innerWidth - 30, currentDragStart.buttonX + deltaX));
      const newY = Math.max(30, Math.min(window.innerHeight - 30, currentDragStart.buttonY + deltaY));
      setOmnibuttonPosition({ x: newX, y: newY });
    }
  }, [setOmnibuttonPosition]);

  const detachMouseListeners = () => {
    if (!mouseListenersActive.current) return;
    document.removeEventListener('mousemove', handleMouseMove);
    document.removeEventListener('mouseup', handleMouseUp);
    mouseListenersActive.current = false;
  };

  const handleMouseUp = useCallback(() => {
    if (isMobile()) return;
    console.log('omnibutton handleMouseUp');
    if (!isDraggingRef.current && dragStartRef.current) {
      if (!isRecentAppsOpen) toggleRecentApps();
      else closeAllOverlays();
    }
    setDragStartState(null);
    setDragging(false);
    detachMouseListeners();
  }, [isRecentAppsOpen, toggleRecentApps, closeAllOverlays]);

  useEffect(() => () => detachMouseListeners(), []);

  // Handle window resize to keep button in bounds
  useEffect(() => {
    const handleResize = () => {
      setOmnibuttonPosition({
        x: Math.max(30, Math.min(window.innerWidth - 30, omnibuttonPosition.x)),
        y: Math.max(30, Math.min(window.innerHeight - 30, omnibuttonPosition.y))
      });
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [omnibuttonPosition]);

  useEffect(() => {
    if (!isRecentAppsOpen) {
      setDragStartState(null);
      setDragging(false);
    }
  }, [isRecentAppsOpen]);

  return (
    <div
      ref={buttonRef}
      className={classNames(
        "fixed z-50 select-none touch-none",
        {
          "cursor-grabbing": isDragging,
          "cursor-pointer": !isDragging,
          "scale-110": isDragging
        }
      )}
      style={{
        left: omnibuttonPosition.x - 30,
        top: omnibuttonPosition.y - 30,
        transform: 'translate(0, 0)' // Prevent transform conflicts
      }}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      onMouseDown={handleMouseDown}
    >
      {/* Black rounded square background */}
      <div className="absolute inset-0 w-16 h-16 bg-black/40 dark:bg-white/10 backdrop-blur-sm rounded-2xl shadow-lg touch-none" />

      {/* White circle with icon */}
      <div className="relative w-16 h-16 flex items-center justify-center touch-none">
        <div className="w-10 h-10 bg-white/90 backdrop-blur-sm rounded-full shadow-md flex items-center justify-center touch-none" />
      </div>
    </div>
  );
};
