import React from "react";
import { Tooltip } from "./Tooltip";

export const CacheSourceTooltip: React.FC = () => {
    return (
        <Tooltip
            text={`
        Cache sources are nodes that provide cached hypermap data to improve performance.
        These nodes store and serve frequently accessed blockchain data locally.
        If unchecked, the system will use default cache sources.
      `}
        />
    );
};