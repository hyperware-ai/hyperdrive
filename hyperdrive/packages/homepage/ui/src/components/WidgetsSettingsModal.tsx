import useHomepageStore from "../store/homepageStore"
import { Modal } from "./Modal"
import usePersistentStore from "../store/persistentStore"
import { FaCheck, FaRegSquare } from "react-icons/fa6"

const WidgetsSettingsModal = () => {
  const { apps, setShowWidgetsSettings } = useHomepageStore()
  const { widgetSettings, toggleWidgetVisibility } = usePersistentStore()

  return <Modal
    title='Widget Settings'
    onClose={() => setShowWidgetsSettings(false)}
  >
    {apps.filter((app) => app.widget).map((app) => {
      return (
        <div className="
          rounded-full 
          border border-solid border-black/5 dark:border-white/5 
          p-2 
          flex items-center justify-between gap-2 self-stretch
        ">
          <h4>{app.label}</h4>
          <button
            className="icon"
            onClick={() => toggleWidgetVisibility(app.id)}
          >
            {widgetSettings[app.id]?.hide ? <FaRegSquare /> : <FaCheck />}
          </button>
        </div>
      );
    })}
    <button onClick={() => window.location.href = '/settings:settings:sys'}>
      System settings
    </button>
  </Modal>
}

export default WidgetsSettingsModal