import { FaSquareCheck, FaRegSquare } from "react-icons/fa6";

interface UpgradableCheckboxProps {
  upgradable: boolean;
  setUpgradable: (upgradable: boolean) => void;
}

export default function UpgradableCheckbox({ upgradable, setUpgradable }: UpgradableCheckboxProps) {
  return (
    <div className="flex gap-2 items-center">
      <button className="icon" onClick={(e) => {
        e.preventDefault();
        e.stopPropagation();
        setUpgradable(!upgradable);
      }}>
        {upgradable ? <FaSquareCheck /> : <FaRegSquare />}
      </button>
      <div className="flex flex-col gap-1 min-w-0 wrap-anywhere">
        <span className="text-sm">Upgradable</span>
        <span className="text-xs">Allows opeator to upgrade implementation</span>
      </div>
    </div>
  );
}