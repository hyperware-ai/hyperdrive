import { useState, useEffect } from "react";
import HyperwareClientApi from "@hyperware-ai/client-api";
import "./App.css";
import FileExplorer from "./components/FileExplorer/FileExplorer";
import { ThemeProvider } from "./contexts/ThemeContext";

const BASE_URL = import.meta.env.BASE_URL;
if (window.our) window.our.process = BASE_URL?.replace("/", "");

const PROXY_TARGET = `${(import.meta.env.VITE_NODE_URL || "http://localhost:8080")}${BASE_URL}`;

const WEBSOCKET_URL = import.meta.env.DEV
  ? `${PROXY_TARGET.replace('http', 'ws')}`
  : undefined;

function App() {
  const [nodeConnected, setNodeConnected] = useState(true);
  const [api, setApi] = useState<HyperwareClientApi | undefined>();

  useEffect(() => {
    // Connect to the Hyperdrive via websocket
    console.log('WEBSOCKET URL', WEBSOCKET_URL)
    if (window.our?.node && window.our?.process) {
      const api = new HyperwareClientApi({
        uri: WEBSOCKET_URL,
        nodeId: window.our.node,
        processId: window.our.process,
        onOpen: (_event, _api) => {
          console.log("Connected to Hyperware");
        },
        onMessage: (json, _api) => {
          console.log('WEBSOCKET MESSAGE', json)
          try {
            const data = JSON.parse(json);
            console.log("WebSocket received message", data);
          } catch (error) {
            console.error("Error parsing WebSocket message", error);
          }
        },
      });

      setApi(api);
    } else {
      setNodeConnected(false);
    }
  }, []);

  return (
    <ThemeProvider>
      <div style={{ width: "100%", height: "100vh", display: "flex", flexDirection: "column" }}>
        {!nodeConnected && (
          <div className="node-not-connected">
            <h2 style={{ color: "red" }}>Node not connected</h2>
            <h4>
              You need to start a node at {PROXY_TARGET} before you can use this UI
              in development.
            </h4>
          </div>
        )}
        {nodeConnected && <FileExplorer />}
      </div>
    </ThemeProvider>
  );
}

export default App;