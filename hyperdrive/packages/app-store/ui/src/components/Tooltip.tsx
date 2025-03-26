import React, { useState } from 'react';
import { FaInfo } from 'react-icons/fa6';
interface TooltipProps {
  content: React.ReactNode;
  children?: React.ReactNode;
}

export function Tooltip({ content, children }: TooltipProps) {
  const [isOpen, setIsOpen] = useState(false);
  return (
    <div className="flex flex-col items-start gap-2">
      {children}

      <button className="icon-button" onClick={() => setIsOpen(!isOpen)}>
        <FaInfo />
      </button>

      {isOpen && <div className="tooltip-content">{content}</div>}
    </div>
  );
}
