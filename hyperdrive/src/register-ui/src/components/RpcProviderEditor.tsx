import { useState } from 'react';
import classNames from 'classnames';

export interface RpcProviderData {
    url: string;
    auth: {
        type: 'Basic' | 'Bearer' | 'Raw' | null;
        value: string;
    } | null;
}

interface RpcProviderEditorProps {
    providers: RpcProviderData[];
    onChange: (providers: RpcProviderData[]) => void;
    label?: string;
}

export function RpcProviderEditor({ providers, onChange, label }: RpcProviderEditorProps) {
    const [showAuthValues, setShowAuthValues] = useState<Record<number, boolean>>({});

    const addProvider = () => {
        onChange([...providers, { url: '', auth: null }]);
    };

    const removeProvider = (index: number) => {
        onChange(providers.filter((_, i) => i !== index));
        // Clean up the showAuthValues state for this index
        const newShowAuthValues = { ...showAuthValues };
        delete newShowAuthValues[index];
        setShowAuthValues(newShowAuthValues);
    };

    const updateProvider = (index: number, updates: Partial<RpcProviderData>) => {
        const newProviders = [...providers];
        newProviders[index] = { ...newProviders[index], ...updates };
        onChange(newProviders);
    };

    const updateAuth = (index: number, authType: 'Basic' | 'Bearer' | 'Raw' | null, authValue: string = '') => {
        const newProviders = [...providers];
        if (authType === null) {
            newProviders[index].auth = null;
        } else {
            newProviders[index].auth = { type: authType, value: authValue };
        }
        onChange(newProviders);
    };

    const toggleAuthVisibility = (index: number) => {
        setShowAuthValues(prev => ({
            ...prev,
            [index]: !prev[index]
        }));
    };

    // Validate individual provider
    const validateProvider = (provider: RpcProviderData): string | null => {
        if (!provider.url.trim()) {
            return 'WebSocket URL is required';
        }
        if (!provider.url.startsWith('wss://')) {
            return 'URL must be a secure WebSocket URL starting with wss://';
        }
        if (provider.auth && !provider.auth.value.trim()) {
            return 'Auth value is required when auth type is specified';
        }
        return null;
    };

    // Get validation errors for all providers
    const validationErrors = providers.map(validateProvider);
    const hasErrors = validationErrors.some(error => error !== null);

    return (
        <div className="flex flex-col gap-3">
            {label && (
                <label className="text-sm font-medium">
                    {label} {providers.length > 0 && <span className="text-red-500">*</span>}
                </label>
            )}

            {providers.map((provider, index) => {
                const error = validationErrors[index];
                return (
                    <div key={index} className="flex flex-col gap-2 p-3 border border-gray-300 dark:border-gray-600 rounded">
                        <div className="flex gap-2 items-start">
                            <div className="flex-1 flex flex-col gap-2">
                                {/* URL Field */}
                                <div className="flex flex-col gap-1">
                                    <label className="text-xs font-medium opacity-70">
                                        WebSocket URL <span className="text-red-500">*</span>
                                    </label>
                                    <input
                                        type="text"
                                        value={provider.url}
                                        onChange={(e) => updateProvider(index, { url: e.target.value })}
                                        placeholder="wss://base-mainnet.infura.io/ws/v3/YOUR-API-KEY"
                                        className={classNames("input text-sm", {
                                            'border-red-500 focus:border-red-500': error !== null
                                        })}
                                    />
                                </div>

                                {/* Auth Type Selector */}
                                <div className="flex flex-col gap-1">
                                    <label className="text-xs font-medium opacity-70">Authentication (Optional)</label>
                                    <select
                                        value={provider.auth?.type || 'none'}
                                        onChange={(e) => {
                                            const value = e.target.value;
                                            if (value === 'none') {
                                                updateAuth(index, null);
                                            } else {
                                                updateAuth(index, value as 'Basic' | 'Bearer' | 'Raw', provider.auth?.value || '');
                                            }
                                        }}
                                        className="input text-sm"
                                    >
                                        <option value="none">No Authentication</option>
                                        <option value="Bearer">Bearer Token</option>
                                        <option value="Basic">Basic Auth</option>
                                        <option value="Raw">Raw Header</option>
                                    </select>
                                </div>

                                {/* Auth Value Field (conditional) */}
                                {provider.auth && (
                                    <div className="flex flex-col gap-1">
                                        <label className="text-xs font-medium opacity-70">
                                            Auth Value <span className="text-red-500">*</span>
                                        </label>

                                        <div className="relative">
                                            <input
                                                type={showAuthValues[index] ? "text" : "password"}
                                                value={provider.auth.value}
                                                onChange={(e) => updateAuth(index, provider.auth!.type, e.target.value)}
                                                placeholder={
                                                    provider.auth.type === 'Bearer' ? 'your-bearer-token' :
                                                        provider.auth.type === 'Basic' ? 'user:pass (base64 encoded)' :
                                                            'custom-header-value'
                                                }
                                                className={classNames("input text-sm", {
                                                    'border-red-500 focus:border-red-500': provider.auth && !provider.auth.value.trim()
                                                })}
                                                style={{ paddingRight: '88px' }}
                                                autoComplete="off"
                                            />
                                            <button
                                                type="button"
                                                onClick={() => toggleAuthVisibility(index)}
                                                className="absolute top-1/2 -translate-y-1/2 text-gray-500 hover:text-gray-700 dark:text-gray-400 dark:hover:text-gray-200 flex items-center gap-1 py-2 pr-2 pl-2"
                                                style={{ right: '0px' }}
                                                title={showAuthValues[index] ? "Hide" : "Show"}
                                            >
                                                {showAuthValues[index] ? (
                                                    // Eye icon (visible/show state)
                                                    <span className="flex items-center gap-1">
                                                        <svg width="20" height="20" viewBox="0 0 576 512" fill="currentColor" className="inline-block">
                                                            <path d="M38.8 5.1C28.4-3.1 13.3-1.2 5.1 9.2S-1.2 34.7 9.2 42.9l592 464c10.4 8.2 25.5 6.3 33.7-4.1s6.3-25.5-4.1-33.7L525.6 386.7c39.6-40.6 66.4-86.1 79.9-118.4c3.3-7.9 3.3-16.7 0-24.6c-14.9-35.7-46.2-87.7-93-131.1C465.5 68.8 400.8 32 320 32c-68.2 0-125 26.3-169.3 60.8L38.8 5.1zM223.1 149.5C248.6 126.2 282.7 112 320 112c79.5 0 144 64.5 144 144c0 24.9-6.3 48.3-17.4 68.7L408 294.5c8.4-19.3 10.6-41.4 4.8-63.3c-11.1-41.5-47.8-69.4-88.6-71.1c-5.8-.2-9.2 6.1-7.4 11.7c2.1 6.4 3.3 13.2 3.3 20.3c0 10.2-2.4 19.8-6.6 28.3l-90.3-70.8zM373 389.9c-16.4 6.5-34.3 10.1-53 10.1c-79.5 0-144-64.5-144-144c0-6.9 .5-13.6 1.4-20.2L83.1 161.5C60.3 191.2 44 220.8 34.5 243.7c-3.3 7.9-3.3 16.7 0 24.6c14.9 35.7 46.2 87.7 93 131.1C174.5 443.2 239.2 480 320 480c47.8 0 89.9-12.9 126.2-32.5L373 389.9z"></path>
                                                        </svg>
                                                        <span className="text-xs">Hide</span>
                                                    </span>
                                                ) : (
                                                    // Eye-off icon (hidden state)
                                                    <span className="flex items-center gap-1">
                                                        <svg width="20" height="20" viewBox="0 0 640 512" fill="currentColor" className="inline-block">
                                                            <path d="M288 32c-80.8 0-145.5 36.8-192.6 80.6C48.6 156 17.3 208 2.5 243.7c-3.3 7.9-3.3 16.7 0 24.6C17.3 304 48.6 356 95.4 399.4C142.5 443.2 207.2 480 288 480s145.5-36.8 192.6-80.6c46.8-43.5 78.1-95.4 93-131.1c3.3-7.9 3.3-16.7 0-24.6c-14.9-35.7-46.2-87.7-93-131.1C433.5 68.8 368.8 32 288 32zM144 256a144 144 0 1 1 288 0 144 144 0 1 1 -288 0zm144-64c0 35.3-28.7 64-64 64c-7.1 0-13.9-1.2-20.3-3.3c-5.5-1.8-11.9 1.6-11.7 7.4c.3 6.9 1.3 13.8 3.2 20.7c13.7 51.2 66.4 81.6 117.6 67.9s81.6-66.4 67.9-117.6c-11.1-41.5-47.8-69.4-88.6-71.1c-5.8-.2-9.2 6.1-7.4 11.7c2.1 6.4 3.3 13.2 3.3 20.3z"></path>
                                                        </svg>
                                                        <span className="text-xs">Show</span>
                                                    </span>
                                                )}
                                            </button>
                                        </div>
                                    </div>
                                )}
                            </div>

                            {/* Remove Button */}
                            <button
                                type="button"
                                onClick={() => removeProvider(index)}
                                className="px-2 py-1 text-red-500 hover:text-red-700 text-sm"
                                title="Remove provider"
                            >
                                ✕
                            </button>
                        </div>

                        {/* Error message for this specific provider */}
                        {error && (
                            <span className="text-xs text-red-500">
                                {error}
                            </span>
                        )}
                    </div>
                );
            })}

            {/* Add Provider Button */}
            <button
                type="button"
                onClick={addProvider}
                className="px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
            >
                + Add RPC Provider
            </button>

            {/* Overall validation summary */}
            {providers.length > 0 && !hasErrors && (
                <span className="text-xs text-gray-500">
                    {providers.length} provider{providers.length !== 1 ? 's' : ''} to be added
                </span>
            )}
        </div>
    );
}