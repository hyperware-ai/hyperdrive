import { RouterTooltip } from "./RouterTooltip";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface SRCBProps {
    specifyRouters: boolean;
    setSpecifyRouters: (specifyRouters: boolean) => void;
    initiallyChecked?: boolean;
}

export default function SpecifyRoutersCheckbox({
                                                   specifyRouters,
                                                   setSpecifyRouters,
                                                   initiallyChecked
                                               }: SRCBProps) {
    const getHelpText = () => {
        if (initiallyChecked === undefined) {
            return "If you are unsure, leave unchecked.";
        }
        return initiallyChecked
            ? "If you are unsure, leave checked."
            : "If you are unsure, leave unchecked.";
    };

    return (
        <div className="flex gap-2 items-center">
            <button
                className="icon"
                onClick={(e) => {
                    e.preventDefault();
                    e.stopPropagation();
                    setSpecifyRouters(!specifyRouters);
                }}
            >
                {specifyRouters ? <FaSquareCheck /> : <FaRegSquare />}
            </button>
            <div className="flex flex-col gap-1 min-w-0 wrap-anywhere">
                <span className="text-sm">Specify routers to register as an indirect node.</span>
                <span className="text-xs">{getHelpText()}</span>
            </div>
            <RouterTooltip />
        </div>
    );
}