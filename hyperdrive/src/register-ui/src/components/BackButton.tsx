import { FaChevronLeft } from "react-icons/fa6";
import classNames from "classnames";

export default function BackButton({ mode = "wide", className }: { mode?: "narrow" | "wide", className?: string }) {
    return (
        <button onClick={() => history.back()} className={classNames(
            {
                'icon absolute top-2 left-2': mode === "narrow",
                'clear': mode === 'wide',
            }, className)}>
            {mode === "narrow" ? <FaChevronLeft /> : <span>Back</span>}
        </button>
    )
}
