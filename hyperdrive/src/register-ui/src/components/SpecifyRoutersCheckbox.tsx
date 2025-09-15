
import { RouterTooltip } from "./RouterTooltip";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface SpecifyRoutersProps {
    specifyRouters: boolean;
    setSpecifyRouters: (specifyRouters: boolean) => void;
}

export default function SpecifyRoutersCheckbox({ specifyRouters, setSpecifyRouters }: SpecifyRoutersProps) {
    return (
        <div className="flex gap-2 items-center">
            <button className="icon" onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setSpecifyRouters(!specifyRouters);
            }}>
                {specifyRouters ? <FaSquareCheck /> : <FaRegSquare />}
            </button>
            <div className="flex flex-col gap-1 min-w-0 wrap-anywhere">
                <span className="text-sm">Specify routers for an indirect node.</span>
                <span className="text-xs">If you are unsure, leave unchecked.</span>
            </div>
            <RouterTooltip />
        </div>
    );
}