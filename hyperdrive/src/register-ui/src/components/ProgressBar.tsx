import classNames from 'classnames';
import { useNavigate, useLocation } from 'react-router-dom';

const steps = [
  { path: '/', label: 'Home' },
  { path: '/commit-os-name', label: 'Choose Name' },
  { path: '/mint-os-name', label: 'Mint Name' },
  { path: '/set-password', label: 'Set Password' },
];

interface ProgressBarProps {
  hnsName: string;
}

const ProgressBar = ({ hnsName }: ProgressBarProps) => {
  const navigate = useNavigate();
  const location = useLocation();

  const currentStepIndex = steps.findIndex(step => step.path === location.pathname);

  const isStepAccessible = (index: number) => {
    // Home is always accessible
    if (index === 0) return true;

    if (hnsName && index <= 2) return true;

    // Otherwise only allow going back
    return index <= currentStepIndex;
  };

  const handleStepClick = (path: string, index: number) => {
    if (isStepAccessible(index)) {
      navigate(path);
    }
  };

  return (
    <div
      className="progress-container mt-3 p-3 max-w-2xl rounded-lg relative"
      style={{
        background: "linear-gradient(145deg, rgba(255, 255, 255, 0.05), rgba(255, 255, 255, 0.02))"
      }}
    >
      <div
        className="progress-bar flex justify-between items-center mx-auto relative px-4"
      >
        {steps.map((step, index) => {
          const accessible = isStepAccessible(index);
          const active = index <= currentStepIndex;
          const completed = index < currentStepIndex;
          return (
            <div
              key={step.path}
              className="step-wrapper flex items-center flex-1 relative"
            >
              <div
                style={{
                }}
                className={classNames("step flex flex-col items-center relative transition-all duration-300 z-20 p-2", {
                  active,
                  completed,
                  "cursor-pointer": accessible,
                  "cursor-not-allowed opacity-50 pointer-events-none": !accessible
                })}
                onClick={() => handleStepClick(step.path, index)}
              >
                <div
                  style={{
                  }}
                  className="
                    step-number 
                    shadow-sm dark:shadow-white/10
                    w-6 h-6 text-sm font-bold rounded-full
                    flex items-center justify-center
                    bg-iris dark:!bg-neon text-white dark:text-black
                    transition-all duration-300
                    relative
                  "
                >
                  {index}
                </div>
                <div
                  className={classNames(`
                    step-label
                    mt-2 
                    text-sm text-center
                    whitespace-nowrap
                    tracking-widest
                  `, {
                    "opacity-85": !active,
                    "font-bold": active
                  })}
                >
                  {step.label}
                </div>
              </div>
            </div>
          );
        })}
      </div>
      {hnsName && (
        <div
          className="selected-name text-center mt-3 font-bold text-black dark:text-white opacity-90 p-3 rounded-lg tracking-wide"
          style={{
            background: 'var(--primary-xlight)',
          }}
        >
          Selected name: <span
            className='font-bold ml-2'
          >
            {hnsName}
          </span>
        </div>
      )}
    </div>
  );
};

export default ProgressBar;
