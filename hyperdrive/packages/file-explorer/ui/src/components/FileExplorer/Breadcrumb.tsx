import React from 'react';
import './Breadcrumb.css';

interface BreadcrumbProps {
  currentPath: string;
  onNavigate: (path: string) => void;
}

const Breadcrumb: React.FC<BreadcrumbProps> = ({ currentPath, onNavigate }) => {
  const pathParts = currentPath.split('/').filter(Boolean);
  const scrollRef = React.useRef<HTMLDivElement>(null);
  
  // Scroll to the end when path changes
  React.useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollLeft = scrollRef.current.scrollWidth;
    }
  }, [currentPath]);
  
  return (
    <div className="breadcrumb-container">
      <div className="breadcrumb" ref={scrollRef}>
      <button 
        className="breadcrumb-item"
        onClick={() => onNavigate('/')}
      >
        Root
      </button>
      
      {pathParts.map((part, index) => {
        const path = '/' + pathParts.slice(0, index + 1).join('/');
        return (
          <React.Fragment key={path}>
            <span className="breadcrumb-separator">/</span>
            <button
              className="breadcrumb-item"
              onClick={() => onNavigate(path)}
            >
              {part}
            </button>
          </React.Fragment>
        );
      })}
      </div>
    </div>
  );
};

export default Breadcrumb;