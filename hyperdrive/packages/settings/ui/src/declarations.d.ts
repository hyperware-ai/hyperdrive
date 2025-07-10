
declare interface ImportMeta {
    env: {
        BASE_URL: string;
        VITE_NODE_URL?: string;
        DEV: boolean;
    };
}
declare interface Window {
    our: {
        node: string;
        process: string;
    };
}