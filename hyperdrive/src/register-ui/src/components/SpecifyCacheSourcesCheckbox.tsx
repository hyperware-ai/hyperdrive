import { CacheSourceTooltip } from "./CacheSourceTooltip";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface SpecifyCacheSourcesProps {
    specifyCacheSources: boolean;
    setSpecifyCacheSources: (specifyCacheSources: boolean) => void;
}

export default function SpecifyCacheSourcesCheckbox({ specifyCacheSources, setSpecifyCacheSources }: SpecifyCacheSourcesProps) {
    return (
        <div className="flex gap-2 items-center">
            <button className="icon" onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setSpecifyCacheSources(!specifyCacheSources);
            }}>
                {specifyCacheSources ? <FaSquareCheck /> : <FaRegSquare />}
            </button>
            <div className="flex flex-col gap-1 min-w-0 wrap-anywhere">
                <span className="text-sm">Specify cache sources for hypermap data.</span>
                <span className="text-xs">If you are unsure, leave unchecked.</span>
            </div>
            <CacheSourceTooltip />
        </div>
    );
}