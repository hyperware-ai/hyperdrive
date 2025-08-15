import React, { useEffect, useState, useRef } from 'react';
import { useNavigationStore } from '../../../stores/navigationStore';
import classNames from 'classnames';
import { BsClock } from 'react-icons/bs';

export const GestureZone: React.FC = () => {
  const { toggleRecentApps, isRecentAppsOpen } = useNavigationStore();
  const [position, setPosition] = useState({ x: window.innerWidth - 80, y: window.innerHeight / 2 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState<{ x: number; y: number; buttonX: number; buttonY: number } | null>(null);
  const dragThreshold = 5; // pixels - swipes smaller than this will be treated as taps
  const buttonRef = useRef<HTMLDivElement>(null);
  const isMobile = window.innerWidth < 768;

  // Touch handlers for drag and tap
  const handleTouchStart = (e: React.TouchEvent) => {
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }
    const touch = e.touches[0];
    setDragStart({
      x: touch.clientX,
      y: touch.clientY,
      buttonX: position.x,
      buttonY: position.y
    });
  };

  const handleTouchMove = (e: React.TouchEvent) => {
    if (!dragStart) return;
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }

    const touch = e.touches[0];
    const deltaX = touch.clientX - dragStart.x;
    const deltaY = touch.clientY - dragStart.y;

    // Check if movement exceeds threshold to start dragging
    if (!isDragging && (Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold)) {
      setIsDragging(true);
    }

    // Update position if dragging
    if (isDragging || Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold) {
      const newX = Math.max(30, Math.min(window.innerWidth - 30, dragStart.buttonX + deltaX));
      const newY = Math.max(30, Math.min(window.innerHeight - 30, dragStart.buttonY + deltaY));
      setPosition({ x: newX, y: newY });
    }
  };

  const handleTouchEnd = () => {
    if (!isDragging && dragStart) {
      // Tap - open recent apps
      toggleRecentApps();
    }
    setDragStart(null);
    setIsDragging(false);
  };

  // Mouse handlers for desktop
  const handleMouseDown = (e: React.MouseEvent) => {
    if (isMobile) return;
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }
    setDragStart({
      x: e.clientX,
      y: e.clientY,
      buttonX: position.x,
      buttonY: position.y
    });
  };

  const handleMouseMove = (e: MouseEvent) => {
    if (isMobile) return;
    try { e.preventDefault(); } catch { }
    try { e.stopPropagation(); } catch { }
    if (!dragStart) return;

    const deltaX = e.clientX - dragStart.x;
    const deltaY = e.clientY - dragStart.y;

    if (!isDragging && (Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold)) {
      setIsDragging(true);
    }

    if (isDragging || Math.abs(deltaX) > dragThreshold || Math.abs(deltaY) > dragThreshold) {
      const newX = Math.max(30, Math.min(window.innerWidth - 30, dragStart.buttonX + deltaX));
      const newY = Math.max(30, Math.min(window.innerHeight - 30, dragStart.buttonY + deltaY));
      setPosition({ x: newX, y: newY });
    }
  };

  const handleMouseUp = () => {
    if (isMobile) return;
    if (!isDragging && dragStart) {
      toggleRecentApps();
    }
    setDragStart(null);
    setIsDragging(false);
  };

  // Mouse event listeners
  useEffect(() => {
    if (dragStart) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('mouseup', handleMouseUp);
      return () => {
        document.removeEventListener('mousemove', handleMouseMove);
        document.removeEventListener('mouseup', handleMouseUp);
      };
    }
  }, [dragStart, isDragging]);

  // Handle window resize to keep button in bounds
  useEffect(() => {
    const handleResize = () => {
      setPosition(prev => ({
        x: Math.max(30, Math.min(window.innerWidth - 30, prev.x)),
        y: Math.max(30, Math.min(window.innerHeight - 30, prev.y))
      }));
    };

    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  useEffect(() => {
    if (!isRecentAppsOpen) {
      setDragStart(null);
      setIsDragging(false);
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
        left: position.x - 30,
        top: position.y - 30,
        transform: 'translate(0, 0)' // Prevent transform conflicts
      }}
      onTouchStart={handleTouchStart}
      onTouchMove={handleTouchMove}
      onTouchEnd={handleTouchEnd}
      onMouseDown={handleMouseDown}
    >
      {/* Black rounded square background */}
      <div className="absolute inset-0 w-16 h-16 bg-black/40 backdrop-blur-sm rounded-2xl shadow-lg touch-none" />

      {/* White circle with icon */}
      <div className="relative w-16 h-16 flex items-center justify-center touch-none">
        <div className="w-10 h-10 bg-white/90 backdrop-blur-sm rounded-full shadow-md flex items-center justify-center touch-none" />
      </div>
    </div>
  );
};