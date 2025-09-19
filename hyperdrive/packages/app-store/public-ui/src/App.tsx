import { BrowserRouter as Router, Route, Routes } from "react-router-dom";
import Home from "./components/Home";
import AppDetail from "./components/AppDetail";
import { APP_DETAILS_PATH, STORE_PATH } from "./constants/path";
import NavBar from "./components/NavBar";

//@ts-ignore
const BASE_URL = import.meta.env.BASE_URL;
//@ts-ignore
if (window.our) window.our.process = BASE_URL?.replace("/", "");

function App() {
  const getBasename = () => {
    const path = window.location.pathname;
    if (path.startsWith('/main:app-store:sys/public')) {
      return '/main:app-store:sys/public';
    }
    return '/';
  };

  return (
    <div className="bg-white dark:bg-stone grow self-stretch min-h-screen px-4 pb-32 md:pb-0 md:px-0 overflow-y-auto">
      <Router basename={getBasename()}>
        <NavBar />
        <Routes>
          <Route path={STORE_PATH} element={<Home />} />
          <Route path={`${APP_DETAILS_PATH}/:id`} element={<AppDetail />} />
        </Routes>
      </Router>
    </div >
  )
}

export default App
