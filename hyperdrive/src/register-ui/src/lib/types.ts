
export interface PageProps {
  networkingKey: string,
  setNetworkingKey: React.Dispatch<React.SetStateAction<string>>,
  ipAddress: number,
  setIpAddress: React.Dispatch<React.SetStateAction<number>>,
  ws_port: number,
  setWsPort: React.Dispatch<React.SetStateAction<number>>,
  tcp_port: number,
  setTcpPort: React.Dispatch<React.SetStateAction<number>>,
  routers: string[],
  setRouters: React.Dispatch<React.SetStateAction<string[]>>,
  direct: boolean,
  setDirect: React.Dispatch<React.SetStateAction<boolean>>,
  upgradable: boolean,
  setUpgradable: React.Dispatch<React.SetStateAction<boolean>>,
  hnsName: string,
  setHnsName: React.Dispatch<React.SetStateAction<string>>,
  key: string,
  keyFileName: string,
  setKeyFileName: React.Dispatch<React.SetStateAction<string>>,
  reset: boolean,
  setReset: React.Dispatch<React.SetStateAction<boolean>>,
  pw: string,
  setPw: React.Dispatch<React.SetStateAction<string>>,
  nodeChainId: string
}

export type NetworkingInfo = {
  name: string,
  networking_key: string,
  routing: {
    Both: {
      ip: string,
      ports: {
        ws?: number,
        tcp?: number
      },
      routers: string[]
    }
  },
}

export type UnencryptedIdentity = {
  name: string,
  allowed_routers: string[]
}

export type InfoResponse = {
  name?: string;
  allowed_routers?: string[];
  initial_cache_sources: string[];
  initial_base_l2_providers: string[];
}

export interface RpcProviderConfig {
  url: string;
  auth: {
    Basic?: string;
    Bearer?: string;
    Raw?: string;
  } | null;
}