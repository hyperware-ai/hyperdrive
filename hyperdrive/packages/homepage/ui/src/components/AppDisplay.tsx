import { HomepageApp } from "../store/homepageStore";

interface AppDisplayProps {
  app?: HomepageApp;
}

const AppDisplay: React.FC<AppDisplayProps> = ({ app }) => {
  return (
    <a
      id={app?.package_name}
      href={app?.path || undefined}
      className="p-2 flex gap-2 items-center"
      title={app?.label}
      style={
        !app?.path
          ? {
            pointerEvents: "none",
            textDecoration: "none !important",
            filter: "grayscale(100%)",
          }
          : {
            touchAction: "manipulation",
            WebkitTapHighlightColor: "transparent",
          }
      }
    >
      {app?.base64_icon ? (
        <img className="w-8 h-8 object-cover rounded-lg" src={app.base64_icon} />
      ) : (
        <img className="w-8 h-8 object-cover rounded-lg" src="/h-green.svg" />
      )}
      <h6 className="text-white">{app?.label || app?.package_name}</h6>
    </a>
  );
};

export default AppDisplay;
