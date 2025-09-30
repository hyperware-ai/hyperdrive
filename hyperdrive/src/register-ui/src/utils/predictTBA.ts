import { encodePacked, keccak256, getAddress, encodeAbiParameters, type Address, type Hex } from 'viem';
import { hyperhash } from './hyperhash';
import { PROXY_CREATION_CODE, ERC6551_REGISTRY } from "../abis";

export function predictTBAAddress(
    hypermapAddr: Address,
    label: string,
    chainId: number = 8453 // Base chain ID
): { predictedAddress: Address, predictedTokenId: BigInt } {
    // Calculate the namehash for the label
    const namehash = hyperhash(label);
    // First compute the proxy address
    const proxyAddr = computeProxyAddress(hypermapAddr, hypermapAddr, namehash);
    console.log("proxyAddr", proxyAddr);
    const predictedTokenId = BigInt(namehash)
    const predictedAddress = computeAccount(proxyAddr, namehash, BigInt(chainId), hypermapAddr, predictedTokenId);
    return { predictedAddress, predictedTokenId };
}

function computeAccount(
    implementation: Address,
    salt: bigint | `0x${string}`,
    chainId: bigint,
    tokenContract: Address,
    tokenId: bigint
): Address {
    // ERC-1167 minimal proxy bytecode components
    const fullHeader = "0x3d60ad80600a3d3981f3363d3d373d3d3d363d73" as Hex;
    const footer = "0x5af43d82803e903d91602b57fd5bf3" as Hex;

    const bytecode = encodePacked(
        ["bytes", "address", "bytes"],
        [fullHeader, implementation, footer]
    );

    // Encode the constructor arguments (salt, chainId, tokenContract, tokenId)
    const constructorArgs = encodeAbiParameters(
        [
            { type: "bytes32" },
            { type: "uint256" },
            { type: "address" },
            { type: "uint256" },
        ],
        [salt as `0x${string}`, chainId, tokenContract, tokenId]
    );

    // Combine bytecode with constructor arguments to match the exact memory layout
    const initCode = encodePacked(
        ["bytes", "bytes"],
        [bytecode, constructorArgs]
    );

    // CREATE2 formula
    const create2Hash = keccak256(
        encodePacked(
            ["bytes1", "address", "bytes32", "bytes32"],
            ["0xff" as Hex, ERC6551_REGISTRY, salt as `0x${string}`, keccak256(initCode)]
        )
    );

    return getAddress(`0x${create2Hash.slice(-40)}`);

}


function computeProxyAddress(
    deployer: Address,
    hypermapAddr: Address,
    salt: string
): Address {

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
            ['0xff', deployer, salt as `0x${string}`, proxyCreationCodeHash]
        )
    );
    return getAddress(`0x${hash.slice(-40)}`) as Address;
}