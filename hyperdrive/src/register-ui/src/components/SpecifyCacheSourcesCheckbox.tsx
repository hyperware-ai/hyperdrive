import { CacheSourceTooltip } from "./CacheSourceTooltip";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface SpecifyCacheSourcesCheckboxProps {
    specifyCacheSources: boolean;
    setSpecifyCacheSources: (specifyCacheSources: boolean) => void;
    initiallyChecked?: boolean;
}

export default function SpecifyCacheSourcesCheckbox({
                                                        specifyCacheSources,
                                                        setSpecifyCacheSources,
                                                        initiallyChecked = false
                                                    }: SpecifyCacheSourcesCheckboxProps) {
    return (
        <div className="flex gap-2 items-center">
            <button className="icon" onClick={(e) => {
                e.preventDefault();
                e.stopPropagation();
                setSpecifyCacheSources(!specifyCacheSources);
            }}>
                {specifyCacheSources ? <FaSquareCheck /> : <FaRegSquare />}
            </button>
            <div className="flex flex-col gap-1 flex-1 min-w-0 wrap-anywhere">
                <span className="text-sm">Specify cache sources.</span>
                <span className="text-xs">
                    If you are unsure, leave {initiallyChecked ? 'checked' : 'unchecked'}.
                </span>
            </div>
            <CacheSourceTooltip />
        </div>
    );
}