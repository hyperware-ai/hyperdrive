import { ConnectButton } from '@rainbow-me/rainbowkit';
import { useAccount } from 'wagmi';
import EditNote from './components/EditNote';
import { useEffect, useMemo, useState } from 'react';
import { PiEye, PiEyeSlash } from 'react-icons/pi';
import classNames from 'classnames';

const APP_PATH = '/settings:settings:sys/ask';

interface Identity {
  name: string;
  networking_key: string;
  ws_routing?: string;
  routers?: string;
}

interface EthRpcSettings {
  public: boolean;
  allow: string[];
  deny: string[];
}

interface ProcessInfo {
  public: boolean;
  on_exit: string;
  wit_version?: string;
  wasm_bytes_handle?: string;
  capabilities: Array<{
    issuer: string;
    params: string;
  }>;
}

interface AppState {
  our_tba: string;
  our_owner: string;
  net_key: string;
  routers: string;
  ip: string;
  tcp_port: string;
  ws_port: string;
  identity: Identity;
  diagnostics: string;
  eth_rpc_providers: any[];
  eth_rpc_access_settings: EthRpcSettings;
  process_map: Record<string, ProcessInfo>;
  stylesheet: string;
}

function App() {
  const [appState, setAppState] = useState<Partial<AppState>>({});
  const [peerPkiResponse, setPeerPkiResponse] = useState('');
  const [peerPingResponse, setPeerPingResponse] = useState('');

  const [showNetworkDiagnostics, setShowNetworkDiagnostics] = useState(false);
  const [showNodeInfo, setShowNodeInfo] = useState(false);
  const [showProcesses, setShowProcesses] = useState(false);
  const [showEthRpcSettings, setShowEthRpcSettings] = useState(false);
  const [showEthRpcProviders, setShowEthRpcProviders] = useState(false);
  const [showIdOnchain, setShowIdOnchain] = useState(false);
  const [showHyperwareCss, setShowHyperwareCss] = useState(false);
  const [showPing, setShowPing] = useState(false);


  const { address } = useAccount();

  useEffect(() => {
    // Initial data fetch
    fetch(APP_PATH)
      .then(response => response.json())
      .then(data => setAppState(data));

    // WebSocket connection
    const wsProtocol = location.protocol === 'https:' ? 'wss://' : 'ws://';
    const ws = new WebSocket(wsProtocol + location.host + "/settings:settings:sys/");
    ws.onmessage = event => {
      const data = JSON.parse(event.data);
      setAppState(data);
    };
  }, []);

  const apiCall = async (body: any) => {
    return await fetch(APP_PATH, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
  };

  const handleShutdown = () => {
    apiCall("Shutdown");
    setTimeout(() => window.location.reload(), 1000);
  };

  const handleReset = () => {
    apiCall("Reset");
    setTimeout(() => window.location.reload(), 1000);
  };

  const handleSaveStylesheet = () => {
    const stylesheet = (document.getElementById('stylesheet-editor') as HTMLTextAreaElement).value;
    apiCall({ "SetStylesheet": stylesheet });
  };

  const handlePeerPki = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const response = await fetch(APP_PATH, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ "PeerId": formData.get('peer') }),
    });
    const data = await response.json();
    setPeerPkiResponse(data === null ? "no pki data for peer" : JSON.stringify(data, undefined, 2));
    e.currentTarget.reset();
  };

  const handlePeerPing = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const form = e.currentTarget;
    const response = await fetch(APP_PATH, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        "Hi": {
          node: formData.get('peer'),
          content: formData.get('content'),
          timeout: Number(formData.get('timeout')),
        }
      }),
    });
    form.reset();
    try {
      const data = await response.json();
      if (data === null) {
        setPeerPingResponse("ping successful!");
      } else if (data === "HiTimeout") {
        setPeerPingResponse("node timed out");
      } else if (data === "HiOffline") {
        setPeerPingResponse("node is offline");
      }
    } catch (err) {
      setPeerPingResponse("ping successful!");
    }
  };

  const handleAddEthProvider = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const form = e.currentTarget;
    const response = await apiCall({
      "EthConfig": {
        "AddProvider": {
          chain_id: Number(formData.get('chain-id')),
          node_or_rpc_url: { "RpcUrl": formData.get('rpc-url') as string }
        }
      }
    });
    try {
      const data = await response.json();
      console.log(data);
    } catch (err) {
      form.reset();
      // this is actually a success
    }

  };

  const handleRemoveEthProvider = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    const formData = new FormData(e.currentTarget);
    const form = e.currentTarget;
    const response = await apiCall({
      "EthConfig": {
        "RemoveProvider": [Number(formData.get('chain-id')), formData.get('rpc-url') as string]
      }
    });
    try {
      const data = await response.json();
      console.log(data);
    } catch (err) {
      form.reset();
      // this is actually a success
    }
  };

  const articleClass = "shadow-xl flex flex-col gap-2 items-stretch rounded-lg  self-stretch p-2 max-w-md";
  const h2Class = "text-lg font-bold flex items-center justify-between prose gap-2";
  const showHideButton = (show: boolean, setShow: (show: boolean) => void) => (
    <button
      className="clear thin text-xl"
      onClick={() => setShow(!show)}
    >
      {show ? <PiEyeSlash className="opacity-50" /> : <PiEye className="opacity-50" />}
    </button>
  );

  const NODE_BASE_URL = useMemo(() => {
    const subdomainDomain = window.location.host.indexOf('.') === -1 ? window.location.host : window.location.host.slice(window.location.host.indexOf('.') + 1);
    return subdomainDomain;
  }, []);


  return (
    <div className='max-w-screen grow self-stretch min-h-screen flex flex-col bg-black/5'>
      <div
        id="header"
        className="bg-neon flex flex-col gap-2 items-stretch p-4"
      >
        <div className="flex self-stretch items-center justify-between gap-4">
          <img
            src={`//${NODE_BASE_URL}/Logomark Iris.svg`}
            alt="back"
            className="h-8 cursor-pointer"
            data-base-url={import.meta.env.BASE_URL}
            onClick={() => {
              window.history.back();
            }}
          />
          <ConnectButton />
        </div>
        <h1 className="font-bold uppercase">system diagnostics & settings</h1>
        <div className="flex items-center justify-between gap-2">

          <span className="text-xs bg-stone text-neon font-bold self-start rounded px-2">{window?.our?.node}</span>
        </div>
      </div>
      <main className=" grid gap-4 p-4  self-stretch grid-cols-1 md:grid-cols-2 lg:grid-cols-3">
        <article
          id="net-diagnostics"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>networking diagnostics</span>
            {showHideButton(showNetworkDiagnostics, setShowNetworkDiagnostics)}
          </h2>
          <p
            id="diagnostics"
            className={classNames("break-all transition-all ", {
              "h-0 overflow-hidden invisible": !showNetworkDiagnostics,
              'h-auto': showNetworkDiagnostics,
            })}>
            {appState.diagnostics}
          </p>
        </article>

        <article
          id="node-info"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>node info</span>
            {showHideButton(showNodeInfo, setShowNodeInfo)}
          </h2>
          <div className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showNodeInfo,
            'h-auto': showNodeInfo,
          })}>
            <p id="node-name">{appState.identity?.name}</p>
            <p id="net-key" className="break-all">{appState.identity?.networking_key}</p>
            {appState.identity?.ws_routing && <p id="ip-ports" className="break-all">{appState.identity.ws_routing}</p>}
            {appState.identity?.routers && <p id="routers" className="break-all">{appState.identity.routers}</p>}
            <div className="flex gap-4">
              <button
                onClick={handleShutdown}
                id="shutdown"
                className="bg-red text-white"
              >
                Shutdown Node
              </button>
              <br />
              <button
                onClick={handleReset}
              >
                Reset HNS State
              </button>
            </div>
          </div>
        </article>

        <article
          id="pings"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>fetch PKI data</span>
            {showHideButton(showPing, setShowPing)}
          </h2>
          <div className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showPing,
            'h-auto': showPing,
          })}>
            <form id="get-peer-pki" className="flex flex-col items-stretch gap-2" onSubmit={handlePeerPki}>
              <input type="text" name="peer" placeholder="peer-name.os" />
              <button type="submit">get peer info</button>
            </form>
            <p id="peer-pki-response">{peerPkiResponse}</p>
            <h2 className={h2Class}>
              <span>ping a node</span>
            </h2>
            <form id="ping-peer" className="flex flex-col items-stretch gap-2" onSubmit={handlePeerPing}>
              <input type="text" name="peer" placeholder="peer-name.os" />
              <input type="text" name="content" placeholder="message" />
              <input type="number" name="timeout" placeholder="timeout (seconds)" />
              <button type="submit">ping</button>
            </form>
            <p id="peer-ping-response">{peerPingResponse}</p>
          </div>
        </article>

        <article id="eth-rpc-providers"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>ETH RPC providers</span>
            {showHideButton(showEthRpcProviders, setShowEthRpcProviders)}
          </h2>
          <div className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showEthRpcProviders,
            'h-auto': showEthRpcProviders,
          })}>
            <form id="add-eth-provider" className="flex flex-col items-stretch gap-2" onSubmit={handleAddEthProvider}>
              <input type="number" name="chain-id" placeholder="1" />
              <input type="text" name="rpc-url" placeholder="wss://rpc-url.com" />
              <button type="submit">add provider</button>
            </form>
            <form id="remove-eth-provider" className="flex flex-col items-stretch gap-2" onSubmit={handleRemoveEthProvider}>
              <input type="number" name="chain-id" placeholder="1" />
              <input type="text" name="rpc-url" placeholder="wss://rpc-url.com" />
              <button type="submit">remove provider</button>
            </form>
            <ul id="providers" className="">
              {appState.eth_rpc_providers?.map((provider, i) => (
                <li className="list-none break-all font-mono" key={i}>{JSON.stringify(provider, undefined, 2)}</li>
              ))}
            </ul>
          </div>
        </article>

        <article id="eth-rpc-settings"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>ETH RPC settings</span>
            {showHideButton(showEthRpcSettings, setShowEthRpcSettings)}
          </h2>
          <div className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showEthRpcSettings,
            'h-auto': showEthRpcSettings,
          })}>
            <p id="public">status: {appState.eth_rpc_access_settings?.public ? 'public' : 'private'}</p>
            {!appState.eth_rpc_access_settings?.public && (
              <article>
                <p>nodes allowed to connect:</p>
                <ul id="allowed-nodes">
                  {appState.eth_rpc_access_settings?.allow.length === 0 ? (
                    <li>(none)</li>
                  ) : (
                    appState.eth_rpc_access_settings?.allow.map((node, i) => (
                      <li key={i}>{node}</li>
                    ))
                  )}
                </ul>
              </article>
            )}
            <article>
              <p>nodes banned from connecting:</p>
              <ul id="denied-nodes">
                {appState.eth_rpc_access_settings?.deny.length === 0 ? (
                  <li>(none)</li>
                ) : (
                  appState.eth_rpc_access_settings?.deny.map((node, i) => (
                    <li key={i}>{node}</li>
                  ))
                )}
              </ul>
            </article>
          </div>
        </article>

        <article id="kernel"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>running processes</span>
            {showHideButton(showProcesses, setShowProcesses)}
          </h2>
          <ul id="process-map" className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showProcesses,
            'h-auto': showProcesses,
          })}>
            {Object.entries(appState.process_map || {}).map(([id, process]) => (
              <li
                className="list-none"
                key={id}
              >
                <button
                  onClick={(e) => {
                    const details = e.currentTarget.nextElementSibling as HTMLElement;
                    details.style.display = details.style.display === 'none' ? 'block' : 'none';
                  }}
                >
                  {id}
                </button>
                <div
                  style={{ display: 'none' }}
                >
                  <p>public: {String(process.public)}</p>
                  <p>on_exit: {process.on_exit}</p>
                  {process.wit_version && <p>wit_version: {process.wit_version}</p>}
                  {process.wasm_bytes_handle && <p>wasm_bytes_handle: {process.wasm_bytes_handle}</p>}
                  <ul>
                    {process.capabilities.map((cap, i) => (
                      <li key={i}>{cap.issuer}({JSON.stringify(JSON.parse(cap.params), null, 2)})</li>
                    ))}
                  </ul>
                </div>
              </li>
            ))}
          </ul>
        </article>

        <article id="id-onchain"
          className={articleClass}
        >
          <h2 className={h2Class}>
            <span>identity onchain</span>
            {showHideButton(showIdOnchain, setShowIdOnchain)}
          </h2>
          <div className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showIdOnchain,
            'h-auto': showIdOnchain,
          })}>
            <p>Only use this utility if you *really* know what you're doing. If edited incorrectly, your node may be unable to connect to the network and require re-registration.</p>
            <br />
            <p className="font-mono break-all">{appState.our_owner && address ? (address.toLowerCase() === appState.our_owner.toLowerCase() ? 'Connected as node owner.' : '**Not connected as node owner. Change wallet to edit node identity.**') : ''}</p>
            <p className="font-mono break-all">TBA: {appState.our_tba}</p>
            <p className="font-mono break-all">Owner: {appState.our_owner}</p>
            <br />
            <p className="font-mono break-all">Routers: {appState.routers || 'none currently, direct node'}</p>
            <EditNote label="~routers" tba={appState.our_tba || ''} field_placeholder="router names, separated by commas (no spaces!)" />
            <p className="font-mono break-all">IP: {appState.ip || 'none currently, indirect node'}</p>
            <EditNote label="~ip" tba={appState.our_tba || ''} field_placeholder="ip address encoded as hex" />
            <p className="font-mono break-all">TCP port: {appState.tcp_port || 'none currently, indirect node'}</p>
            <EditNote label="~tcp-port" tba={appState.our_tba || ''} field_placeholder="tcp port as a decimal number (e.g. 8080)" />
            <p className="font-mono break-all">WS port: {appState.ws_port || 'none currently, indirect node'}</p>
            <EditNote label="~ws-port" tba={appState.our_tba || ''} field_placeholder="ws port as a decimal number (e.g. 8080)" />
            <p>Add a brand new note to your node ID</p>
            <EditNote tba={appState.our_tba || ''} field_placeholder="note content" />
          </div>
        </article>

        <article id="hyperware-css" className={articleClass}>
          <h2 className={h2Class}>
            <span>stylesheet editor</span>
            {showHideButton(showHyperwareCss, setShowHyperwareCss)}
          </h2>
          <div className={classNames("flex flex-col items-stretch transition-all gap-2", {
            "h-0 overflow-hidden invisible": !showHyperwareCss,
            'h-auto': showHyperwareCss,
          })}>
            <textarea id="stylesheet-editor" defaultValue={appState.stylesheet} className="grow self-stretch min-h-64 font-mono" />
            <button id="save-stylesheet" onClick={handleSaveStylesheet}>update hyperware.css</button>
          </div>
        </article>
      </main>
    </div>
  );
}

export default App;