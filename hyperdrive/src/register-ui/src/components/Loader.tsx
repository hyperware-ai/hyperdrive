import classNames from "classnames"
import { FaCircleNotch } from "react-icons/fa6"

type LoaderProps = {
  msg: string
  className?: string
}

export default function Loader({ msg, className }: LoaderProps) {
  return (
    <div id="loading" className={classNames("flex flex-col gap-4 items-center text-center", className)}>
      <FaCircleNotch className="animate-spin text-4xl" />
      <h3>{msg}</h3>
    </div>
  )
}