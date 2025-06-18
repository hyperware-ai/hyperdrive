import React, { useState, useEffect, useRef } from 'react';
import type { Position } from '../../../types/app.types';

interface DraggableProps {
  id: string;
  position: Position;
  onMove: (position: Position) => void;
  isEditMode: boolean;
  children: React.ReactNode;
  className?: string;
}

export const Draggable: React.FC<DraggableProps> = ({
  id,
  position,
  onMove,
  isEditMode,
  children,
  className = ''
}) => {
  const [isDragging, setIsDragging] = useState(false);
  const [dragOffset, setDragOffset] = useState({ x: 0, y: 0 });
  const elementRef = useRef<HTMLDivElement>(null);

  const handleStart = (clientX: number, clientY: number) => {
    if (!isEditMode) return;
    setIsDragging(true);
    setDragOffset({
      x: clientX - position.x,
      y: clientY - position.y,
    });
  };

  const handleMove = (clientX: number, clientY: number) => {
    if (!isDragging) return;

    // Get element dimensions
    const element = elementRef.current;
    if (!element) return;

    const rect = element.getBoundingClientRect();
    const elementWidth = rect.width;
    const elementHeight = rect.height;

    // Calculate new position with bounds checking
    // Reserve 120px at the bottom for the dock (dock height + padding)
    const dockHeight = 120;
    const newX = Math.max(0, Math.min(window.innerWidth - elementWidth, clientX - dragOffset.x));
    const newY = Math.max(0, Math.min(window.innerHeight - elementHeight - dockHeight, clientY - dragOffset.y));

    onMove({ x: newX, y: newY });
  };

  const handleEnd = () => {
    setIsDragging(false);
  };

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => handleMove(e.clientX, e.clientY);
    const handleTouchMove = (e: TouchEvent) => {
      const touch = e.touches[0];
      handleMove(touch.clientX, touch.clientY);
    };
    const handleMouseUp = () => handleEnd();
    const handleTouchEnd = () => handleEnd();

    if (isDragging) {
      document.addEventListener('mousemove', handleMouseMove);
      document.addEventListener('touchmove', handleTouchMove);
      document.addEventListener('mouseup', handleMouseUp);
      document.addEventListener('touchend', handleTouchEnd);
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('touchmove', handleTouchMove);
      document.removeEventListener('mouseup', handleMouseUp);
      document.removeEventListener('touchend', handleTouchEnd);
    };
  }, [isDragging, dragOffset, onMove]);

  const handleDragStart = (e: React.DragEvent) => {
    if (!isEditMode) return;
    e.dataTransfer.setData('appId', id);
    e.dataTransfer.effectAllowed = 'move';
  };

  return (
    <div
      ref={elementRef}
      className={`absolute ${isDragging ? 'z-50' : ''} ${className}`}
      style={{
        left: `${position.x}px`,
        top: `${position.y}px`,
        cursor: isEditMode ? 'move' : 'default',
        touchAction: isEditMode ? 'none' : 'auto',
      }}
      draggable={isEditMode}
      onDragStart={handleDragStart}
      onMouseDown={(e) => handleStart(e.clientX, e.clientY)}
      onTouchStart={(e) => {
        const touch = e.touches[0];
        handleStart(touch.clientX, touch.clientY);
      }}
    >
      {children}
    </div>
  );
};
