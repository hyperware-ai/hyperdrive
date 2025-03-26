import { DirectTooltip } from "./DirectTooltip";
import { FaCheck, FaX } from "react-icons/fa6";

interface DNCBProps {
  direct: boolean;
  setDirect: (direct: boolean) => void;
}

export default function DirectNodeCheckbox({ direct, setDirect }: DNCBProps) {
  return (
    <div className="flex items-center gap-2">
      <button className="icon" onClick={() => setDirect(!direct)}>
        {direct ? <FaCheck /> : <FaX />}
      </button>
      <span className="checkbox-label">
        Register as a direct node. If you are unsure, leave unchecked.
      </span>
      <DirectTooltip />
    </div>
  );
}