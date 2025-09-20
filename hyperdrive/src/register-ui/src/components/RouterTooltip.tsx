import React from "react";
import { Tooltip } from "./Tooltip";

export const RouterTooltip: React.FC = () => {
    return (
        <Tooltip
            text={`
        For indirect nodes, you can specify which router nodes to use for networking.
        Routers are direct nodes that help relay traffic for indirect nodes.
        If unchecked, the system will automatically select appropriate routers.
      `}
        />
    );
};