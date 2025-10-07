import { BaseL2AccessProviderTooltip } from "./BaseL2AccessProviderTooltip";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface SpecifyBaseL2AccessProvidersProps {
    specifyBaseL2AccessProviders: boolean;
    setSpecifyBaseL2AccessProviders: (specifyBaseL2AccessProviders: boolean) => void;
    initiallyChecked?: boolean;
}

export default function SpecifyBaseL2AccessProvidersCheckbox({
                                                                 specifyBaseL2AccessProviders,
                                                                 setSpecifyBaseL2AccessProviders,
                                                                 initiallyChecked = false
                                                             }: SpecifyBaseL2AccessProvidersProps) {
    return (
        <div className="flex gap-2 items-center">
            <button className="icon" onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setSpecifyBaseL2AccessProviders(!specifyBaseL2AccessProviders);
            }}>
                {specifyBaseL2AccessProviders ? <FaSquareCheck /> : <FaRegSquare />}
            </button>
            <div className="flex flex-col gap-1 flex-1 min-w-0 wrap-anywhere">
                <span className="text-sm">Add Base L2 access providers.</span>
                <span className="text-xs">
                    If you are unsure, leave {initiallyChecked ? 'checked' : 'unchecked'}.
                </span>
            </div>
            <BaseL2AccessProviderTooltip />
        </div>
    );
}