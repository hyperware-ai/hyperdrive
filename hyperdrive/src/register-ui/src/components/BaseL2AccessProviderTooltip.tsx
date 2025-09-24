import React from "react";
import { Tooltip } from "./Tooltip";

export const BaseL2AccessProviderTooltip: React.FC = () => {
    return (
        <Tooltip
            text={`
        Base L2 Access Providers are specialized nodes that provide access to Base Layer 2 blockchain data.
        These providers help your node interact with Base L2 for faster and cheaper transactions.
        If unchecked, the system will use default Base L2 access providers.
      `}
        />
    );
};