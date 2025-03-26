import { DirectTooltip } from "./DirectTooltip";
import { FaCheck } from "react-icons/fa6";

interface DNCBProps {
  direct: boolean;
  setDirect: (direct: boolean) => void;
}

export default function DirectNodeCheckbox({ direct, setDirect }: DNCBProps) {
  return (
    <div className="flex items-center gap-2">
      <button className="checkbox-button" onClick={(e) => {
        e.preventDefault(); // Prevent form submission
        e.stopPropagation(); // Prevent event bubbling
        setDirect(!direct);
      }}>
        {direct ? <FaCheck /> : null}
      </button>
      <span className="checkbox-label">
        Register as a direct node. If you are unsure, leave unchecked.
      </span>
      <DirectTooltip />
    </div>
  );
}