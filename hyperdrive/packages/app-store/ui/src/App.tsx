import React from "react";
import { BrowserRouter as Router, Route, Routes, Navigate } from "react-router-dom";

import Header from "./components/Header";
import { APP_DETAILS_PATH, DOWNLOAD_PATH, MY_APPS_PATH, PUBLISH_PATH, STORE_PATH } from "./constants/path";

import StorePage from "./pages/StorePage";
import AppPage from "./pages/AppPage";
import DownloadPage from "./pages/DownloadPage";
import PublishPage from "./pages/PublishPage";
import MyAppsPage from "./pages/MyAppsPage";
import { ToastContainer } from "react-toastify";


//@ts-ignore
const BASE_URL = import.meta.env.BASE_URL;
//@ts-ignore
if (window.our) window.our.process = BASE_URL?.replace("/", "");

function App() {
  return (
    <div className="bg-white dark:bg-stone grow self-stretch min-h-screen px-4 pb-32 md:pb-0 md:px-0 overflow-y-auto">
      <Router basename={BASE_URL}>
        <Header />
        <Routes>
          <Route path={STORE_PATH} element={<StorePage />} />
          <Route path={MY_APPS_PATH} element={<MyAppsPage />} />
          <Route path={`${APP_DETAILS_PATH}/:id`} element={<AppPage />} />
          <Route path={PUBLISH_PATH} element={<PublishPage />} />
          <Route path={`${DOWNLOAD_PATH}/:id`} element={<DownloadPage />} />
        </Routes>
      </Router>
      <ToastContainer

      />
    </div >
  );
}

export default App;
