import { FaChevronLeft } from "react-icons/fa6";

export default function BackButton() {
    return (
        <button onClick={() => history.back()} className="icon absolute top-2 left-2">
            <FaChevronLeft />
        </button>
    )
}
