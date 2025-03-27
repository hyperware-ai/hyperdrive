import React, { useRef, useEffect } from "react";
import { Tooltip } from "./Tooltip";

export const DirectTooltip: React.FC = () => {
  const tooltipRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const positionTooltip = () => {
      const tooltips = document.querySelectorAll('.tooltip-text');

      tooltips.forEach(tooltip => {
        const tooltipElem = tooltip as HTMLElement;
        const parentRect = tooltipElem.parentElement?.getBoundingClientRect();

        if (parentRect) {
          const left = Math.min(
            parentRect.left,
            window.innerWidth - tooltipElem.offsetWidth - 10
          );

          tooltipElem.style.left = `${Math.max(10, left)}px`;
          tooltipElem.style.top = `${parentRect.bottom + 10}px`;
        }
      });
    };

    // Position on hover
    const tooltipElement = tooltipRef.current?.parentElement;
    if (tooltipElement) {
      tooltipElement.addEventListener('mouseenter', positionTooltip);
      window.addEventListener('resize', positionTooltip);

      return () => {
        tooltipElement.removeEventListener('mouseenter', positionTooltip);
        window.removeEventListener('resize', positionTooltip);
      };
    }
  }, []);

  return (
    <div ref={tooltipRef}>
      <Tooltip text={`A direct node publishes its own networking information on-chain: IP, port, so on. An indirect node relies on the service of routers, which are themselves direct nodes. Only register a direct node if you know what you're doing and have a public, static IP address.`}>
        <span>ⓘ</span>
      </Tooltip>
    </div>
  );
};
