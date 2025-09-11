import { encodePacked, keccak256, getAddress, encodeAbiParameters } from 'viem';
import { hyperhash } from './hyperhash';

const ERC6551_REGISTRY = '0x000000006551c19487814612e58FE06813775758' as const;

export function predictTBAAddress(
    hypermapAddr: `0x${string}`,
    label: string,
    chainId: number = 8453 // Base chain ID
): `0x${string}` {
    // Calculate the namehash for the label
    const namehash = hyperhash(label);

    // First compute the proxy address
    const proxyAddr = computeProxyAddress(hypermapAddr, hypermapAddr, namehash);

    // Implementation bytecode hash for ERC6551 v3
    const ACCOUNT_IMPLEMENTATION_BYTECODE_HASH = keccak256(encodePacked(['string'], ['erc6551:v3:account']));

    // Create the init code for ERC6551 account
    const initCode = encodePacked(
        ['bytes10', 'address', 'bytes32', 'uint256', 'address', 'uint256'],
        [
            '0x' + '00'.repeat(10), // 10 bytes of zeros for ERC6551 v3
            proxyAddr, // implementation (proxy)
            namehash, // salt
            BigInt(chainId), // chainId
            hypermapAddr, // tokenContract
            BigInt(namehash) // tokenId (using namehash as tokenId)
        ]
    );

    // Compute init code hash for ERC6551 account
    const initCodeHash = keccak256(
        encodePacked(
            ['bytes', 'bytes32'],
            [initCode, ACCOUNT_IMPLEMENTATION_BYTECODE_HASH]
        )
    );

    // Compute the TBA address using CREATE2
    const hash = keccak256(
        encodePacked(
            ['bytes1', 'address', 'bytes32'],
            ['0xff', ERC6551_REGISTRY, initCodeHash]
        )
    );

    return getAddress(`0x${hash.slice(-40)}`) as `0x${string}`;
}

function computeProxyAddress(
    deployer: `0x${string}`,
    hypermapAddr: `0x${string}`,
    salt: `0x${string}`
): `0x${string}` {
    // HyperAccountProxy creation code with constructor argument
    const PROXY_CREATION_CODE = '0x60a0604052348015600e575f5ffd5b5060405161051d38038061051d833981016040819052602b91603b565b6001600160a01b03166080526066565b5f60208284031215604a575f5ffd5b81516001600160a01b0381168114605f575f5ffd5b9392505050565b6080516104a061007d5f395f607a01526104a05ff3fe608060405260043610610021575f3560e01c8063d1f578941461003257610028565b3661002857005b610030610045565b005b610030610040366004610383565b610057565b610055610050610132565b610169565b565b7f7d0893b5fe6077fb4cf083ec3487b8eece7e03b4ab6e888f7a8a1758010f8c007f00000000000000000000000000000000000000000000000000000000000000006001600160a01b031633146100df57805460ff16156100bf576100ba610045565b6100df565b60405163572190d160e01b81523360048201526024015b60405180910390fd5b805460ff16156101015760405162dc149f60e41b815260040160405180910390fd5b5f61010a610132565b6001600160a01b03160361012d57805460ff1916600117815561012d8383610187565b505050565b5f6101647f360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc546001600160a01b031690565b905090565b365f5f375f5f365f845af43d5f5f3e808015610183573d5ff35b3d5ffd5b610190826101e0565b6040516001600160a01b038316907fbc7cd75a20ee27fd9adebab32041f755214dbc6bffa90cc0225b39da2e5c2d3b905f90a28051156101d45761012d8282610256565b6101dc6102c8565b5050565b806001600160a01b03163b5f0361021557604051634c9c8ce360e01b81526001600160a01b03821660048201526024016100d6565b7f360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc80546001600160a01b0319166001600160a01b0392909216919091179055565b60605f5f846001600160a01b0316846040516102729190610454565b5f60405180830381855af49150503d805f81146102aa576040519150601f19603f3d011682016040523d82523d5f602084013e6102af565b606091505b50915091506102bf8583836102e7565b95945050505050565b34156100555760405163b398979f60e01b815260040160405180910390fd5b6060826102fc576102f782610346565b61033f565b815115801561031357506001600160a01b0384163b155b1561033c57604051639996b31560e01b81526001600160a01b03851660048201526024016100d6565b50805b9392505050565b8051156103565780518082602001fd5b60405163d6bda27560e01b815260040160405180910390fd5b634e487b7160e01b5f52604160045260245ffd5b5f5f60408385031215610394575f5ffd5b82356001600160a01b03811681146103aa575f5ffd5b9150602083013567ffffffffffffffff8111156103c5575f5ffd5b8301601f810185136103d5575f5ffd5b803567ffffffffffffffff8111156103ef576103ef61036f565b604051601f8201601f19908116603f0116810167ffffffffffffffff8111828210171561041e5761041e61036f565b604052818152828201602001871015610435575f5ffd5b816020840160208301375f602083830101528093505050509250929050565b5f82518060208501845e5f92019182525091905056fea26469706673582212205c8437c90a52b26afb62a6e21b8baa0d106dcc547054521f0074dea229fd630f64736f6c634300081c0033';

    const proxyCreationCodeHash = keccak256(
        encodePacked(
            ['bytes', 'bytes'],
            [
                PROXY_CREATION_CODE,
                encodeAbiParameters(
                    [{ type: 'address' }],
                    [getAddress(hypermapAddr)]
                )
            ]
        )
    );

    const hash = keccak256(
        encodePacked(
            ['bytes1', 'address', 'bytes32', 'bytes32'],
            ['0xff', deployer, salt, proxyCreationCodeHash]
        )
    );
    return getAddress(`0x${hash.slice(-40)}`) as `0x${string}`;
}
