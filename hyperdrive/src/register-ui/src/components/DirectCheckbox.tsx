import { DirectTooltip } from "./DirectTooltip";
import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface DNCBProps {
  direct: boolean;
  setDirect: (direct: boolean) => void;
}

export default function DirectNodeCheckbox({ direct, setDirect }: DNCBProps) {
  return (
    <div className="flex gap-2 items-center">
      <button className="icon" onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setDirect(!direct);
      }}>
        {direct ? <FaSquareCheck /> : <FaRegSquare />}
      </button>
      <div className="flex flex-col gap-1 min-w-0 wrap-anywhere">
        <span className="text-sm">Register as a direct node.</span>
        <span className="text-xs">If you are unsure, leave unchecked.</span>
      </div>
      <DirectTooltip />
    </div>
  );
}