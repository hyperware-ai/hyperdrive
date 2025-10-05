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
    const addProvider = () => {
        onChange([...providers, { url: '', auth: null }]);
    };

    const removeProvider = (index: number) => {
        onChange(providers.filter((_, i) => i !== index));
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

    const hasEmptyRequired = providers.some(p => !p.url.trim());

    return (
        <div className="flex flex-col gap-3">
            {label && (
                <label className="text-sm font-medium">
                    {label} {providers.length > 0 && <span className="text-red-500">*</span>}
                </label>
            )}

            {providers.map((provider, index) => (
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
                                        'border-red-500 focus:border-red-500': !provider.url.trim()
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
                                    <input
                                        type="text"
                                        value={provider.auth.value}
                                        onChange={(e) => updateAuth(index, provider.auth!.type, e.target.value)}
                                        placeholder={
                                            provider.auth.type === 'Bearer' ? 'your-bearer-token' :
                                                provider.auth.type === 'Basic' ? 'username:password (base64 encoded)' :
                                                    'custom-header-value'
                                        }
                                        className={classNames("input text-sm", {
                                            'border-red-500 focus:border-red-500': !provider.auth.value.trim()
                                        })}
                                    />
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
                </div>
            ))}

            {/* Add Provider Button */}
            <button
                type="button"
                onClick={addProvider}
                className="px-3 py-2 text-sm border border-gray-300 dark:border-gray-600 rounded hover:bg-gray-100 dark:hover:bg-gray-800"
            >
                + Add RPC Provider
            </button>

            {/* Validation Message */}
            {providers.length > 0 && hasEmptyRequired && (
                <span className="text-xs text-red-500">
          All providers must have a valid WebSocket URL
        </span>
            )}

            {providers.length > 0 && !hasEmptyRequired && (
                <span className="text-xs text-gray-500">
          {providers.length} provider{providers.length !== 1 ? 's' : ''} configured
        </span>
            )}
        </div>
    );
}