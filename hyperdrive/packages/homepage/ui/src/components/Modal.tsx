import { FaX } from "react-icons/fa6"
import { useEffect } from 'react';
import classNames from "classnames";

interface Props extends React.HTMLAttributes<HTMLDivElement> {
  title: string
  onClose: () => void
  outerClassName?: string
  innerClassName?: string
}

export const Modal: React.FC<Props> = ({ title, onClose, children, outerClassName, innerClassName }) => {
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [onClose]);

  return (
    <div
      className={classNames(`
        fixed top-0 left-0 w-screen h-screen
        flex flex-col items-center justify-center
        bg-black/10 backdrop-blur-sm
        z-50
      `, outerClassName)}
    >
      <div
        className={classNames(`
          flex flex-col items-center self-center gap-4
          bg-white dark:bg-black 
          rounded-xl p-4 
          w-screen md:w-fit
          overflow-y-auto
          max-h-[90vh]
        `, innerClassName)}
      >
        <div className="flex items-center justify-between gap-2">
          <h2>{title}</h2>
          <button
            className="icon"
            onClick={onClose}
          >
            <FaX />
          </button>
        </div>
        {children}
      </div>
    </div>
  )
}