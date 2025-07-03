import React from 'react';
import { FaInfo } from 'react-icons/fa6';
import classNames from 'classnames';
interface TooltipProps {
  text?: string;
  children?: React.ReactNode;
  className?: string;
}

export const Tooltip: React.FC<TooltipProps> = ({ text, children, className }) => {
  return (
    <div className={classNames("tooltip", className)}>
      {children}
      {text && <div className="tooltip-text">{text}</div>}
      <FaInfo className="bg-iris/50 rounded-full p-1 w-4 h-4 text-white" />
    </div>
  );
};